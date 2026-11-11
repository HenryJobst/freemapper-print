//! The API of the editor: `GET api/project`, `GET api/revision`, `GET api/preview`, `POST api/hit`,
//! `GET api/check` and `GET api/export/status` read; `POST api/op/{name}`, `POST api/undo` and `POST api/redo`
//! write the project file; `POST api/export` writes PDFs and `POST api/export/cancel` stops it.
//!
//! Neither names a file: the project file is the one `edit` was started with. A write names the revision the page
//! last saw in `X-Freemapper-Revision`; a changed file answers 409, a refused change 422 (core ADR 0030). Drawing a sheet takes seconds
//! on a large map, so it runs on a blocking task and the server stays responsive.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use axum::extract::{Path as UrlPath, Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use fluent_bundle::FluentArgs;
use freemapper_print::web::{REVISION_HEADER, refused, revision, stale, texts_language};
use serde::Deserialize;

use super::edits::{self, EditError};
use super::preview::{self, Drawn, PreviewError};
use super::state::{History, Refusal};
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

/// The export in progress, shared by the handlers and the server's end: one export at a time, a flag that stops
/// it after the sheet in hand, and the progress the page shows.
#[derive(Default)]
pub(super) struct ExportJob {
    running: AtomicBool,
    /// The person pressed Stop; reset when the next export starts.
    cancel: AtomicBool,
    /// The server is ending (`Fertig`, Ctrl+C): the export stops after the sheet in hand, whatever the page says.
    pub(super) closing: AtomicBool,
    /// Pages drawn so far.
    pages: AtomicUsize,
    /// The course (else the layout) of the page drawn last.
    last: Mutex<Option<String>>,
}

/// Clears `running` when the export ends, whatever way.
struct Running<'a>(&'a ExportJob);

impl Drop for Running<'_> {
    fn drop(&mut self) {
        self.0.running.store(false, Ordering::SeqCst);
    }
}

/// Test hook: the first page an export of `project` reports tells `reached` and waits for `release`, before the
/// cancel flag is looked at.
#[cfg(test)]
pub(super) static EXPORT_GATE: Mutex<Option<Gate>> = Mutex::new(None);

/// What the handlers share.
pub(super) struct Api {
    /// The export in progress.
    pub(super) export: Arc<ExportJob>,
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
    /// Undo and redo; held for the whole of a write, so two writes never interleave.
    history: Mutex<History>,
}

impl Api {
    pub(super) fn new(project: PathBuf, fixed: Option<String>, export: Arc<ExportJob>) -> Self {
        Api {
            export,
            project,
            fixed,
            latest: AtomicU64::new(0),
            last: Mutex::new(None),
            history: Mutex::new(History::default()),
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

    fn history_flags(&self) -> (bool, bool) {
        let history = self.history.lock().unwrap_or_else(|e| e.into_inner());
        (history.can_undo(), history.can_redo())
    }

    /// The project file's text as it is on disk now.
    fn read_text(&self) -> Result<String, OpError> {
        std::fs::read_to_string(&self.project).map_err(|source| OpError::LoadProject {
            path: self.project.clone(),
            source: anyhow::Error::new(source),
        })
    }

    /// Runs one edit under the revision guard. The history stays locked throughout.
    fn edit(&self, run: edits::Run, args: serde_json::Value, claimed: &str, lang: &Lang) -> Done {
        let mut history = self.history.lock().unwrap_or_else(|e| e.into_inner());
        let before = match self.read_text() {
            Ok(text) => text,
            Err(error) => return Done::Refused(sentence(lang, error, &self.project)),
        };
        let current = revision(before.as_bytes());
        if current != claimed {
            return Done::Stale(current);
        }
        let result = match run(&self.project, args, lang) {
            Ok(result) => result,
            Err(EditError::Form) => return Done::Form,
            Err(EditError::Op(error)) => {
                return Done::Refused(sentence(lang, error, &self.project));
            }
            Err(EditError::Say(id, value)) => {
                let texts = Texts::new(lang);
                return Done::Refused(
                    texts.get(id, Some(&args!("id" => value.clone(), "path" => value))),
                );
            }
        };
        let after = match self.read_text() {
            Ok(text) => text,
            Err(error) => return Done::Refused(sentence(lang, error, &self.project)),
        };
        if after != before {
            history.record(before, &after);
        }
        Done::Changed {
            revision: revision(after.as_bytes()),
            result,
            can_undo: history.can_undo(),
            can_redo: history.can_redo(),
        }
    }

    /// Undo or redo: the file as an earlier edit left it, if nobody else changed it since.
    fn step(&self, undo: bool, claimed: &str, lang: &Lang) -> Done {
        let texts = Texts::new(lang);
        let mut history = self.history.lock().unwrap_or_else(|e| e.into_inner());
        let current = match self.read_text() {
            Ok(text) => text,
            Err(error) => return Done::Refused(sentence(lang, error, &self.project)),
        };
        let target = if undo {
            history.undo_text(&current)
        } else {
            history.redo_text(&current)
        };
        let target = match target {
            Ok(text) => text.to_owned(),
            Err(Refusal::ChangedOutside) => {
                // No snapshot is safe over somebody else's change.
                history.forget();
                return Done::Refused(texts.get("edit-changed-outside", None));
            }
            Err(Refusal::Nothing) => {
                let id = if undo {
                    "edit-nothing-undo"
                } else {
                    "edit-nothing-redo"
                };
                return Done::Refused(texts.get(id, None));
            }
        };
        let now = revision(current.as_bytes());
        if now != claimed {
            return Done::Stale(now);
        }
        // The last look before the write: whoever wrote in the meantime is not overwritten.
        match self.read_text() {
            Ok(again) if again == current => {}
            Ok(_) => {
                history.forget();
                return Done::Refused(texts.get("edit-changed-outside", None));
            }
            Err(error) => return Done::Refused(sentence(lang, error, &self.project)),
        }
        if let Err(source) = freemapper_print::project::replace_text(&self.project, &target) {
            let error = OpError::WriteFile {
                path: self.project.clone(),
                source: anyhow::Error::new(source),
            };
            return Done::Refused(sentence(lang, error, &self.project));
        }
        if undo {
            history.commit_undo(current, &target);
        } else {
            history.commit_redo(current, &target);
        }
        Done::Changed {
            revision: revision(target.as_bytes()),
            result: serde_json::Value::Null,
            can_undo: history.can_undo(),
            can_redo: history.can_redo(),
        }
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
    refused(&sentence(lang, error, project))
}

/// [`refusal`]'s sentence.
fn sentence(lang: &Lang, error: OpError, project: &std::path::Path) -> String {
    let texts = Texts::new(lang);
    let message = format!("{:#}", crate::op_error(&texts, error));
    scrub(&message, project)
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

/// `GET api/revision`: `{ revision, sources, can_undo, can_redo }`: the revision of the project file and the signature of the map and
/// course file it names, as they are now. The page reads the project again when either changes.
pub(super) async fn get_revision(State(api): State<Arc<Api>>, headers: HeaderMap) -> Response {
    let lang = api.lang(&headers);
    let project = api.project.clone();
    match tokio::task::spawn_blocking(move || api.stamp().map(|stamp| (stamp, api.history_flags())))
        .await
    {
        Ok(Ok((stamp, (can_undo, can_redo)))) => json(&serde_json::json!({
            "revision": stamp.revision,
            "sources": stamp.sources,
            "can_undo": can_undo,
            "can_redo": can_redo,
        })),
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
        let (can_undo, can_redo) = api.history_flags();
        object.insert("can_undo".into(), can_undo.into());
        object.insert("can_redo".into(), can_redo.into());
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

/// `GET api/spec`: `{ revision, layouts, shared }`, the layouts and shared elements as the project file has them,
/// for the page's forms. Like the rest it names no file.
pub(super) async fn get_spec(State(api): State<Arc<Api>>, headers: HeaderMap) -> Response {
    let lang = api.lang(&headers);
    let project = api.project.clone();
    let answer = tokio::task::spawn_blocking(move || -> Result<serde_json::Value, OpError> {
        let text = api.read_text()?;
        let loaded = ops::load_project(&api.project)?;
        Ok(serde_json::json!({
            "revision": revision(text.as_bytes()),
            "layouts": loaded.project.layouts,
            "shared": loaded.project.shared,
        }))
    })
    .await;
    match answer {
        Ok(Ok(value)) => json(&value),
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
    // The control numbers in conflict, for the panel that offers the proposal: the sentence is the note's own.
    let conflicts: Vec<_> = drawn
        .sheet
        .notes
        .iter()
        .filter_map(|notice| match &notice.value {
            crate::ops::note::NoteValue::Sheet(
                freemapper_print::sheet::SheetNote::NumberConflict(c),
            ) => Some(serde_json::json!({
                "control": c.control,
                "course": c.course,
                "message": notice.describe(&texts).message,
                "searched": c.searched,
                "free": c.free,
            })),
            _ => None,
        })
        .collect();
    let numbers: Vec<_> = drawn
        .report
        .numbers
        .iter()
        .map(|(control, (x, y))| serde_json::json!({ "control": control, "centre_mm": [x, y] }))
        .collect();
    Ok(serde_json::json!({
        "conflicts": conflicts,
        "numbers": numbers,
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

/// What a write came to.
enum Done {
    Changed {
        revision: String,
        result: serde_json::Value,
        can_undo: bool,
        can_redo: bool,
    },
    /// The file is not the one the page named; its revision now.
    Stale(String),
    /// The change was refused with this sentence; the file is as it was.
    Refused(String),
    /// The arguments do not have the op's form.
    Form,
}

/// The revision the page names; `None` when the header is missing (400).
fn claimed(headers: &HeaderMap) -> Option<String> {
    headers
        .get(REVISION_HEADER)
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn answer(done: Done, lang: &Lang) -> Response {
    match done {
        Done::Changed {
            revision,
            result,
            can_undo,
            can_redo,
        } => json(&serde_json::json!({
            "revision": revision,
            "can_undo": can_undo,
            "can_redo": can_redo,
            "result": result,
        })),
        Done::Stale(current) => stale(&current, &Texts::new(lang).get("edit-stale", None)),
        Done::Refused(sentence) => refused(&sentence),
        Done::Form => bad_request("form"),
    }
}

/// `POST api/op/{name}` with `X-Freemapper-Revision` and the op's arguments as JSON: 200 `{ revision, can_undo,
/// can_redo, result }`, 400 for a missing revision or a form that does not fit, 404 for a name that is no op, 409
/// when the file is no longer the one the page saw, 422 for a change that is refused (the file stays as it was).
pub(super) async fn post_op(
    State(api): State<Arc<Api>>,
    UrlPath(name): UrlPath<String>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let Some(run) = edits::find(&name) else {
        return (StatusCode::NOT_FOUND, "op").into_response();
    };
    let Some(claimed) = claimed(&headers) else {
        return bad_request("revision");
    };
    let Ok(args) = serde_json::from_slice::<serde_json::Value>(&body) else {
        return bad_request("json");
    };
    let lang = api.lang(&headers);
    let for_answer = lang.clone();
    match tokio::task::spawn_blocking(move || api.edit(run, args, &claimed, &lang)).await {
        Ok(done) => answer(done, &for_answer),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn step(api: Arc<Api>, headers: HeaderMap, undo: bool) -> Response {
    let Some(claimed) = claimed(&headers) else {
        return bad_request("revision");
    };
    let lang = api.lang(&headers);
    let for_answer = lang.clone();
    match tokio::task::spawn_blocking(move || api.step(undo, &claimed, &lang)).await {
        Ok(done) => answer(done, &for_answer),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `POST api/undo` with `X-Freemapper-Revision`: puts the file back as it was before the last edit; the answer is
/// that of `post_op` with a `null` result. 422 when there is nothing to undo or the file changed outside.
pub(super) async fn post_undo(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    _body: axum::body::Bytes,
) -> Response {
    step(api, headers, true).await
}

/// `POST api/redo`: the edit undo took back.
pub(super) async fn post_redo(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    _body: axum::body::Bytes,
) -> Response {
    step(api, headers, false).await
}

/// The body of `POST api/export`.
#[derive(Deserialize)]
struct ExportRequest {
    out_dir: String,
    #[serde(default)]
    courses: Vec<String>,
    #[serde(default)]
    layouts: Vec<String>,
    #[serde(default)]
    single_file: bool,
}

/// `value` with every string scrubbed of absolute paths (see [`scrub`]), except under the key `keep`.
fn scrub_value(value: &mut serde_json::Value, project: &std::path::Path, keep: &str) {
    match value {
        serde_json::Value::String(text) => *text = scrub(text, project),
        serde_json::Value::Array(items) => {
            items.iter_mut().for_each(|v| scrub_value(v, project, keep));
        }
        serde_json::Value::Object(map) => {
            for (key, v) in map.iter_mut() {
                if key != keep {
                    scrub_value(v, project, keep);
                }
            }
        }
        _ => {}
    }
}

/// The export's answer: what `export_project` returns, with each file by its name (the folder is the one the person
/// typed).
fn export_answer(
    texts: &Texts,
    result: &ops::export::ExportResult,
    project: &std::path::Path,
) -> serde_json::Value {
    let out = crate::mcp::export_out(texts, result, crate::mcp::MAX_ENTRIES);
    let mut value = serde_json::to_value(out).expect("an export result serialises");
    if let Some(files) = value["files"].as_array_mut() {
        for file in files {
            let name = file["path"]
                .as_str()
                .and_then(|p| std::path::Path::new(p).file_name())
                .map(|n| n.to_string_lossy().into_owned());
            if let Some(name) = name {
                file["path"] = name.into();
            }
        }
    }
    scrub_value(&mut value, project, "out_dir");
    value["stopped"] = false.into();
    value
}

/// `POST api/export { out_dir, courses?, layouts?, single_file? }`: writes the PDFs as `export_project` does and
/// answers `{ out_dir, files, notes, reminder, stopped, ... }`. `out_dir` is the one path the person types; it must
/// be absolute, exist and not lead out of the project through a link. 422 for a refusal (nothing written for it).
/// Stopped by `POST api/export/cancel`, it answers 200 with `stopped: true` and a sentence that counts the PDFs
/// written. One export runs at a time.
pub(super) async fn post_export(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let lang = api.lang(&headers);
    let texts = Texts::new(&lang);
    let Ok(request) = serde_json::from_slice::<ExportRequest>(&body) else {
        return bad_request("form");
    };
    if !request.layouts.is_empty() && (!request.courses.is_empty() || request.single_file) {
        return refused(&texts.plain("mcp-error-export-layouts"));
    }
    let out_dir = match crate::mcp::agent_path(&texts, &request.out_dir) {
        Ok(dir) => dir,
        Err(message) => return refused(&message),
    };
    let project = api.project.clone();
    if !out_dir.is_dir() {
        let error = OpError::OutDirMissing { path: out_dir };
        return refusal(&lang, error, &project);
    }
    let job = Arc::clone(&api.export);
    if job.running.swap(true, Ordering::SeqCst) {
        return refused(&texts.plain("editor-export-running"));
    }
    job.cancel.store(false, Ordering::SeqCst);
    job.pages.store(0, Ordering::SeqCst);
    *job.last.lock().unwrap_or_else(|e| e.into_inner()) = None;
    let for_answer = lang.clone();
    let typed = request.out_dir.clone();
    let worker_project = project.clone();
    let answer = tokio::task::spawn_blocking(move || {
        let _running = Running(&job);
        #[cfg(test)]
        let mut gated = false;
        let result = ops::export::export(
            &ops::export::ExportArgs {
                project: worker_project.clone(),
                layouts: request.layouts,
                courses: request.courses,
                single_file: request.single_file,
                out_dir: Some(out_dir),
            },
            &mut |progress| {
                if let ops::export::ExportProgress::Page { page, .. } = &progress {
                    job.pages.fetch_add(1, Ordering::SeqCst);
                    let label = page
                        .sheet
                        .course
                        .clone()
                        .unwrap_or_else(|| page.sheet.layout.clone());
                    *job.last.lock().unwrap_or_else(|e| e.into_inner()) = Some(label);
                    #[cfg(test)]
                    if !gated {
                        gated = true;
                        let gate = {
                            let mut slot = EXPORT_GATE.lock().unwrap();
                            match slot.as_ref() {
                                Some((p, ..)) if *p == worker_project => slot.take(),
                                _ => None,
                            }
                        };
                        if let Some((_, reached, release)) = gate {
                            let _ = reached.send(());
                            let _ = release.recv_timeout(std::time::Duration::from_secs(30));
                        }
                    }
                }
                if job.cancel.load(Ordering::SeqCst) || job.closing.load(Ordering::SeqCst) {
                    std::ops::ControlFlow::Break(())
                } else {
                    std::ops::ControlFlow::Continue(())
                }
            },
        );
        (result, worker_project)
    })
    .await;
    match answer {
        Ok((Ok(result), project)) => {
            let mut value = export_answer(&Texts::new(&for_answer), &result, &project);
            // The folder as the person typed it.
            value["out_dir"] = typed.into();
            json(&value)
        }
        Ok((Err(OpError::ExportStopped { written }), project)) => json(&serde_json::json!({
            "stopped": true,
            "count": written.len(),
            "message": sentence(&for_answer, OpError::ExportStopped { written }, &project),
        })),
        Ok((Err(error), project)) => refusal(&for_answer, error, &project),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `GET api/export/status`: `{ running, pages, last }`, the progress the page shows in its status bar.
pub(super) async fn get_export_status(State(api): State<Arc<Api>>) -> Response {
    let job = &api.export;
    let last = job.last.lock().unwrap_or_else(|e| e.into_inner()).clone();
    json(&serde_json::json!({
        "running": job.running.load(Ordering::SeqCst),
        "pages": job.pages.load(Ordering::SeqCst),
        "last": last,
    }))
}

/// `POST api/export/cancel`: the running export stops after the sheet in hand; `{ running }` says whether there was one.
pub(super) async fn post_export_cancel(
    State(api): State<Arc<Api>>,
    _body: axum::body::Bytes,
) -> Response {
    let running = api.export.running.load(Ordering::SeqCst);
    if running {
        api.export.cancel.store(true, Ordering::SeqCst);
    }
    json(&serde_json::json!({ "running": running }))
}

/// `GET api/check`: what `check_project` returns (`map_scale`, `needs_attention`, `course_file`, `courses`,
/// `layouts`, each note with `kind`, `severity`, `course` and `message`), for the page's notices.
pub(super) async fn get_check(State(api): State<Arc<Api>>, headers: HeaderMap) -> Response {
    let lang = api.lang(&headers);
    let project = api.project.clone();
    let for_check = lang.clone();
    let answer = tokio::task::spawn_blocking(move || {
        let result = ops::check::check(&ops::check::CheckArgs {
            project: api.project.clone(),
        })?;
        let out = crate::mcp::check_out(&Texts::new(&for_check), &result, crate::mcp::MAX_ENTRIES);
        Ok::<_, OpError>(serde_json::to_value(out).expect("a check result serialises"))
    })
    .await;
    match answer {
        Ok(Ok(mut value)) => {
            scrub_value(&mut value, &project, "");
            json(&value)
        }
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
