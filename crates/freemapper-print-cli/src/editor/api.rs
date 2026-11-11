//! The read API of the editor: `GET api/project`, `GET api/revision`, `GET api/preview`, `POST api/hit`.
//!
//! Reads never name a file: the project file is the one `edit` was started with. Drawing a sheet takes seconds
//! on a large map, so it runs on a blocking task and the server stays responsive.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use axum::extract::{Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use freemapper_print::web::{refused, revision, texts_language};
use serde::Deserialize;

use super::preview::{self, Drawn, PreviewError};
use crate::ops::{self, OpError};
use crate::text::{Lang, TEXTS, Texts};

/// A sheet drawn for the project file's revision and its sources' signature, a course and a layout.
struct Cached {
    stamp: Stamp,
    course: Option<String>,
    layout: Option<String>,
    drawn: Arc<Drawn>,
}

/// What a sheet depends on: the project file's revision (core `revision` of its bytes) and the signature of the
/// map and course file it names (length and modification time), so a map saved again is noticed too.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Stamp {
    pub(super) revision: String,
    pub(super) sources: String,
}

/// Test hook: the next sheet drawn for `project` (a cache miss) tells `reached` and waits for `release`, so a
/// test can send a newer request while an older one is being drawn.
#[cfg(test)]
pub(super) type Gate = (
    PathBuf,
    std::sync::mpsc::Sender<()>,
    std::sync::mpsc::Receiver<()>,
);
#[cfg(test)]
pub(super) static GATE: Mutex<Option<Gate>> = Mutex::new(None);

/// What the handlers share.
pub(super) struct Api {
    /// The project file.
    pub(super) project: PathBuf,
    /// The language the person set, if any (see `editor::fixed_language`).
    pub(super) fixed: Option<String>,
    /// The highest `rev` a preview request has named; a request with a lower one is superseded.
    ///
    /// `rev` counts the requests of one page, and the page starts counting at the time it was opened (ms), so
    /// a page opened later, a reload or a second tab always outranks the requests of an earlier one. A page
    /// opened earlier then finds its own previews superseded, which is right: the person looks at the newer one.
    latest: AtomicU64,
    /// The last sheet drawn, for the click that follows its preview: the project file's revision, the course
    /// and the layout it was drawn for.
    last: Mutex<Option<Cached>>,
}

impl Api {
    pub(super) fn new(project: PathBuf, fixed: Option<String>) -> Self {
        Api {
            project,
            fixed,
            latest: AtomicU64::new(0),
            last: Mutex::new(None),
        }
    }

    /// The language of this request: the person's setting, else the browser's, else English.
    pub(super) fn lang(&self, headers: &HeaderMap) -> Lang {
        let accept = headers
            .get(header::ACCEPT_LANGUAGE)
            .and_then(|value| value.to_str().ok());
        let code = texts_language(self.fixed.as_deref(), accept);
        TEXTS
            .parse_lang_arg(code)
            .unwrap_or_else(|_| Lang::english())
    }

    /// The revision of the project file as it is on disk now.
    fn file_revision(&self) -> Result<String, OpError> {
        std::fs::read(&self.project)
            .map(|bytes| revision(&bytes))
            .map_err(|source| OpError::LoadProject {
                path: self.project.clone(),
                source: anyhow::Error::new(source),
            })
    }

    /// The signature of the map and the course file the project names: length and modification time of each.
    /// Best effort: a project that does not load has the signature `unreadable`, and the next read says why.
    fn sources(&self) -> String {
        let Ok(loaded) = ops::load_project(&self.project) else {
            return "unreadable".to_owned();
        };
        let mut files = vec![loaded.map_path()];
        files.extend(
            loaded
                .project
                .sources
                .courses
                .as_ref()
                .map(|p| loaded.resolve(p)),
        );
        files
            .iter()
            .map(|file| match std::fs::metadata(file) {
                Ok(meta) => {
                    let nanos = meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map_or(0, |d| d.as_nanos());
                    format!("{}:{nanos}", meta.len())
                }
                Err(_) => "missing".to_owned(),
            })
            .collect::<Vec<_>>()
            .join(",")
    }

    /// The revision of the project file and the signature of its sources, as they are on disk now.
    fn stamp(&self) -> Result<Stamp, OpError> {
        Ok(Stamp {
            revision: self.file_revision()?,
            sources: self.sources(),
        })
    }

    /// The sheet for `course` and `layout`, drawn again only when the project file, its sources or the choice
    /// changed. A sheet whose request was superseded while it was drawn (`rev` below the newest) is answered but
    /// not kept, so the cache never falls back to an older choice.
    fn drawn(
        &self,
        course: Option<&str>,
        layout: Option<&str>,
        rev: Option<u64>,
    ) -> Result<Arc<Drawn>, OpError> {
        let stamp = self.stamp()?;
        let (course, layout) = (course.map(str::to_owned), layout.map(str::to_owned));
        if let Some(hit) = self.last.lock().unwrap().as_ref()
            && hit.stamp == stamp
            && hit.course == course
            && hit.layout == layout
        {
            return Ok(Arc::clone(&hit.drawn));
        }
        #[cfg(test)]
        {
            let gate = {
                let mut slot = GATE.lock().unwrap();
                match slot.as_ref() {
                    Some((project, ..)) if *project == self.project => slot.take(),
                    _ => None,
                }
            };
            if let Some((_, reached, release)) = gate {
                let _ = reached.send(());
                let _ = release.recv_timeout(std::time::Duration::from_secs(30));
            }
        }
        let drawn = Arc::new(preview::draw(
            &self.project,
            course.as_deref(),
            layout.as_deref(),
        )?);
        let stale = rev.is_some_and(|rev| self.latest.load(Ordering::SeqCst) > rev);
        if !stale {
            *self.last.lock().unwrap() = Some(Cached {
                stamp,
                course,
                layout,
                drawn: Arc::clone(&drawn),
            });
        }
        Ok(drawn)
    }
}

/// The sentence for the person for a failed read, in the request's language. The page never gets an absolute
/// path: the folder of the project goes, and any other path shrinks to its file name.
fn refusal(lang: &Lang, error: OpError, project: &std::path::Path) -> Response {
    let texts = Texts::new(lang);
    let message = format!("{:#}", crate::op_error(&texts, error));
    refused(&scrub(&message, project))
}

/// `message` without absolute paths (see [`refusal`]).
fn scrub(message: &str, project: &std::path::Path) -> String {
    let mut text = message.to_owned();
    if let Some(dir) = project.parent().and_then(|d| d.to_str())
        && !dir.is_empty()
    {
        for separator in ["/", "\\"] {
            text = text.replace(&format!("{dir}{separator}"), "");
        }
    }
    text.split_inclusive(char::is_whitespace)
        .map(|word| {
            let body = word.trim_start_matches(['"', '\'', '`', '(', '\u{201e}', '\u{201c}']);
            let lead = &word[..word.len() - body.len()];
            let absolute = body.starts_with('/')
                || body.starts_with("\\\\")
                || (body.len() > 2
                    && body.as_bytes()[0].is_ascii_alphabetic()
                    && body.as_bytes()[1] == b':'
                    && matches!(body.as_bytes()[2], b'/' | b'\\'));
            if !absolute {
                return word.to_owned();
            }
            // Keep what follows the last separator, and the closing punctuation with it.
            let name = body.rsplit(['/', '\\']).next().unwrap_or(body);
            format!("{lead}{name}")
        })
        .collect()
}

fn json(value: &serde_json::Value) -> Response {
    (
        [(header::CONTENT_TYPE, "application/json")],
        value.to_string(),
    )
        .into_response()
}

/// `GET api/revision`: `{ revision, sources }`: the revision of the project file and the signature of the map and
/// course file it names, as they are now. The page reads the project again when either changes.
pub(super) async fn get_revision(State(api): State<Arc<Api>>, headers: HeaderMap) -> Response {
    let lang = api.lang(&headers);
    let project = api.project.clone();
    match tokio::task::spawn_blocking(move || api.stamp()).await {
        Ok(Ok(stamp)) => {
            json(&serde_json::json!({ "revision": stamp.revision, "sources": stamp.sources }))
        }
        Ok(Err(error)) => refusal(&lang, error, &project),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `GET api/project`: `{ lang, revision, ...ProjectInfo }`, without the file's path.
pub(super) async fn get_project(State(api): State<Arc<Api>>, headers: HeaderMap) -> Response {
    let lang = api.lang(&headers);
    let project = api.project.clone();
    let answer = tokio::task::spawn_blocking(move || -> Result<serde_json::Value, OpError> {
        let stamp = api.stamp()?;
        let info = ops::project::info(&ops::project::InfoArgs {
            project: api.project.clone(),
        })?;
        let mut value = serde_json::to_value(&info).expect("a project description serialises");
        let object = value
            .as_object_mut()
            .expect("a project description is an object");
        // The page shows what the person gave; the absolute path stays on the server.
        object.remove("path");
        object.insert("revision".into(), stamp.revision.into());
        object.insert("sources".into(), stamp.sources.into());
        Ok(value)
    })
    .await;
    match answer {
        Ok(Ok(mut value)) => {
            value["lang"] = lang.code().into();
            json(&value)
        }
        Ok(Err(error)) => refusal(&lang, error, &project),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// The query string as `name -> value`, percent-decoded; `+` is a space.
fn query(request_query: Option<&str>) -> HashMap<String, String> {
    request_query
        .unwrap_or_default()
        .split('&')
        .filter(|pair| !pair.is_empty())
        .filter_map(|pair| {
            let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
            Some((decode(name)?, decode(value)?))
        })
        .collect()
}

fn decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                let hex = text.get(i + 1..i + 3)?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            byte => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

/// 409 `{ "error": "superseded" }`: a newer preview request has come in; the page drops this answer.
fn superseded() -> Response {
    (
        StatusCode::CONFLICT,
        [(header::CONTENT_TYPE, "application/json")],
        r#"{"error":"superseded"}"#,
    )
        .into_response()
}

fn bad_request(what: &str) -> Response {
    (StatusCode::BAD_REQUEST, what.to_owned()).into_response()
}

/// An empty value is no choice.
fn choice(params: &HashMap<String, String>, name: &str) -> Option<String> {
    params.get(name).filter(|v| !v.is_empty()).cloned()
}

/// What `preview` answers.
fn preview_body(drawn: &Drawn, lang: &Lang) -> Result<serde_json::Value, PreviewError> {
    let svg = drawn.svg().map_err(PreviewError::Svg)?;
    let texts = Texts::new(lang);
    let notes: Vec<_> = drawn
        .sheet
        .notes
        .iter()
        .map(|notice| notice.describe(&texts))
        .collect();
    Ok(serde_json::json!({
        "svg": svg,
        "page_mm": [drawn.page.page_mm.0, drawn.page.page_mm.1],
        "course": drawn.sheet.course,
        "layout": drawn.sheet.layout,
        "print_scale": drawn.sheet.print.print_scale,
        "map_scale": drawn.sheet.map_scale,
        "notes": notes,
        "elements": drawn.elements(),
    }))
}

/// `GET api/preview?course=&layout=&rev=`: `{ svg, page_mm, notes, elements, ... }`.
///
/// `rev` is a number the page counts up with each request, starting at the time it was opened (see `Api`). A request whose `rev` is lower than one already seen,
/// when it arrives or when its sheet is drawn, answers 409 `superseded`: the page shows the newest answer only.
pub(super) async fn get_preview(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    request: Request,
) -> Response {
    let params = query(request.uri().query());
    let lang = api.lang(&headers);
    let rev = match params.get("rev").filter(|v| !v.is_empty()) {
        None => None,
        Some(text) => match text.parse::<u64>() {
            Ok(rev) => Some(rev),
            Err(_) => return bad_request("rev"),
        },
    };
    if let Some(rev) = rev
        && api.latest.fetch_max(rev, Ordering::SeqCst) > rev
    {
        return superseded();
    }
    let (course, layout) = (choice(&params, "course"), choice(&params, "layout"));
    let project = api.project.clone();
    let worker = Arc::clone(&api);
    let body_lang = lang.clone();
    let answer = tokio::task::spawn_blocking(move || {
        let drawn = worker.drawn(course.as_deref(), layout.as_deref(), rev)?;
        preview_body(&drawn, &body_lang).map_err(|e| match e {
            PreviewError::Op(error) => error,
            PreviewError::Svg(error) => OpError::Serve {
                source: anyhow::Error::new(error),
            },
        })
    })
    .await;
    if let Some(rev) = rev
        && api.latest.load(Ordering::SeqCst) > rev
    {
        return superseded();
    }
    match answer {
        Ok(Ok(body)) => json(&body),
        Ok(Err(error)) => refusal(&lang, error, &project),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// The body of `POST api/hit`.
#[derive(Deserialize)]
struct HitRequest {
    #[serde(default)]
    course: Option<String>,
    #[serde(default)]
    layout: Option<String>,
    x_mm: f64,
    y_mm: f64,
}

/// `POST api/hit { course, layout, x_mm, y_mm }`: `{ kind, id }` of what lies at the point of the page (mm, y
/// down), or `{ kind: null, id: null }` where nothing does.
pub(super) async fn post_hit(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let lang = api.lang(&headers);
    let Ok(request) = serde_json::from_slice::<HitRequest>(&body) else {
        return bad_request("json");
    };
    let course = request.course.filter(|v| !v.is_empty());
    let layout = request.layout.filter(|v| !v.is_empty());
    let project = api.project.clone();
    let answer = tokio::task::spawn_blocking(move || {
        let drawn = api.drawn(course.as_deref(), layout.as_deref(), None)?;
        Ok::<_, OpError>(drawn.pick(request.x_mm, request.y_mm))
    })
    .await;
    match answer {
        Ok(Ok(Some(pick))) => json(&serde_json::json!({ "kind": pick.kind, "id": pick.id })),
        Ok(Ok(None)) => json(&serde_json::json!({ "kind": null, "id": null })),
        Ok(Err(error)) => refusal(&lang, error, &project),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_shrink_to_file_names() {
        let project = std::path::Path::new("/home/ann/event/freemapper-print.toml");
        let text = scrub(
            "cannot read /home/ann/event/luebeck.omap: no such file; also \"/etc/x/y.ppen\" and C:\\Users\\a\\b.omap, D/H 12 stays",
            project,
        );
        assert_eq!(
            text,
            "cannot read luebeck.omap: no such file; also \"y.ppen\" and b.omap, D/H 12 stays"
        );
    }

    #[test]
    fn the_query_is_percent_decoded() {
        let q = query(Some("course=D%2FH%2012&layout=a4+x&rev=3&empty=&flag"));
        assert_eq!(q["course"], "D/H 12");
        assert_eq!(q["layout"], "a4 x");
        assert_eq!(q["rev"], "3");
        assert_eq!(q["empty"], "");
        assert_eq!(q["flag"], "");
        let umlaut = query(Some("course=L%C3%BCbeck"));
        assert_eq!(umlaut["course"], "Lübeck");
        // A broken escape drops the pair, never the request.
        assert!(!query(Some("a=%zz&b=1")).contains_key("a"));
        assert_eq!(query(Some("a=%zz&b=1"))["b"], "1");
        assert!(query(None).is_empty());
    }
}
