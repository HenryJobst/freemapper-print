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

/// A sheet drawn for the project file's revision, a course and a layout.
type Cached = (String, Option<String>, Option<String>, Arc<Drawn>);

/// What the handlers share.
pub(super) struct Api {
    /// The project file.
    pub(super) project: PathBuf,
    /// The language the person set, if any (see `editor::fixed_language`).
    pub(super) fixed: Option<String>,
    /// The highest `rev` a preview request has named; a request with a lower one is superseded.
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

    /// The sheet for `course` and `layout`, drawn again only when the project file or the choice changed.
    fn drawn(&self, course: Option<&str>, layout: Option<&str>) -> Result<Arc<Drawn>, OpError> {
        let rev = self.file_revision()?;
        let key = (rev, course.map(str::to_owned), layout.map(str::to_owned));
        if let Some((r, c, l, drawn)) = self.last.lock().unwrap().as_ref()
            && (r, c, l) == (&key.0, &key.1, &key.2)
        {
            return Ok(Arc::clone(drawn));
        }
        let drawn = Arc::new(preview::draw(&self.project, course, layout)?);
        *self.last.lock().unwrap() = Some((key.0, key.1, key.2, Arc::clone(&drawn)));
        Ok(drawn)
    }
}

/// The sentence for the person for a failed read, in the request's language.
fn refusal(lang: &Lang, error: OpError) -> Response {
    let texts = Texts::new(lang);
    let message = format!("{:#}", crate::op_error(&texts, error));
    refused(&message)
}

fn json(value: &serde_json::Value) -> Response {
    (
        [(header::CONTENT_TYPE, "application/json")],
        value.to_string(),
    )
        .into_response()
}

/// `GET api/revision`: `{ revision }` of the project file as it is now.
pub(super) async fn get_revision(State(api): State<Arc<Api>>, headers: HeaderMap) -> Response {
    let lang = api.lang(&headers);
    match tokio::task::spawn_blocking(move || api.file_revision()).await {
        Ok(Ok(rev)) => json(&serde_json::json!({ "revision": rev })),
        Ok(Err(error)) => refusal(&lang, error),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `GET api/project`: `{ lang, revision, ...ProjectInfo }`, without the file's path.
pub(super) async fn get_project(State(api): State<Arc<Api>>, headers: HeaderMap) -> Response {
    let lang = api.lang(&headers);
    let answer = tokio::task::spawn_blocking(move || -> Result<serde_json::Value, OpError> {
        let rev = api.file_revision()?;
        let info = ops::project::info(&ops::project::InfoArgs {
            project: api.project.clone(),
        })?;
        let mut value = serde_json::to_value(&info).expect("a project description serialises");
        let object = value
            .as_object_mut()
            .expect("a project description is an object");
        // The page shows what the person gave; the absolute path stays on the server.
        object.remove("path");
        object.insert("revision".into(), rev.into());
        Ok(value)
    })
    .await;
    match answer {
        Ok(Ok(mut value)) => {
            value["lang"] = lang.code().into();
            json(&value)
        }
        Ok(Err(error)) => refusal(&lang, error),
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
/// `rev` is a number the page counts up with each request. A request whose `rev` is lower than one already seen,
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
    let worker = Arc::clone(&api);
    let body_lang = lang.clone();
    let answer = tokio::task::spawn_blocking(move || {
        let drawn = worker.drawn(course.as_deref(), layout.as_deref())?;
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
        Ok(Err(error)) => refusal(&lang, error),
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
    let answer = tokio::task::spawn_blocking(move || {
        let drawn = api.drawn(course.as_deref(), layout.as_deref())?;
        Ok::<_, OpError>(drawn.pick(request.x_mm, request.y_mm))
    })
    .await;
    match answer {
        Ok(Ok(Some(pick))) => json(&serde_json::json!({ "kind": pick.kind, "id": pick.id })),
        Ok(Ok(None)) => json(&serde_json::json!({ "kind": null, "id": null })),
        Ok(Err(error)) => refusal(&lang, error),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
