//! The server of the editor: the routes on top of `freemapper_web::serve`.
//!
//! Routes below the token: `GET /` (the page), `GET /assets/*` (the page's own files), `POST /api/done` (ends
//! the server), `GET /api/texts` (the page's words). The suite assets at `/suite/*` come from `freemapper-web`.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::Router;
use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use freemapper_print::web::{self, LocalServer, ServeOptions, WebError};
use std::path::PathBuf;
use tokio::sync::Notify;

use super::api::{self, Api, ExportJob};
use crate::ops::OpError;
use crate::text::TEXTS;

/// Only the page's own files, plus the suite assets of `freemapper-web`; nothing from the network.
pub(super) const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data:; \
connect-src 'self'; base-uri 'none'; form-action 'none'";

/// The page and its own assets, embedded.
#[derive(rust_embed::RustEmbed)]
#[folder = "src/editor/web"]
struct Assets;

fn options(port: u16) -> ServeOptions {
    ServeOptions {
        port,
        strict_post: true,
        ..ServeOptions::new(CSP)
    }
}

async fn index() -> Response {
    web::embedded::<Assets>("index.html")
}

async fn asset(Path(path): Path<String>) -> Response {
    if path == "index.html" {
        // The page has its own route; keep one address for it.
        return StatusCode::NOT_FOUND.into_response();
    }
    web::embedded::<Assets>(&path)
}

/// 200 and the end of the server; the page calls it from `Fertig`.
async fn done(done: Arc<Notify>, job: Arc<ExportJob>, _body: axum::body::Bytes) -> Response {
    // An export in progress stops after the sheet in hand; the server waits for it, so no PDF is left half written.
    job.closing.store(true, std::sync::atomic::Ordering::SeqCst);
    done.notify_one();
    ([(header::CONTENT_TYPE, "application/json")], "{}").into_response()
}

/// Every `editor-*` and `suite-*` message for the page, as `{ lang, texts }`. The language is the one the
/// person set (`--lang`, `FREEMAPPER_LANG`), else the browser's `Accept-Language`, else English (core ADR 0030).
async fn texts(fixed: Option<String>, headers: HeaderMap) -> Response {
    let accept = headers
        .get(header::ACCEPT_LANGUAGE)
        .and_then(|value| value.to_str().ok());
    let code = web::texts_language(fixed.as_deref(), accept);
    let lang = TEXTS
        .parse_lang_arg(code)
        .unwrap_or_else(|_| crate::text::Lang::english());
    let texts: BTreeMap<String, String> = TEXTS
        .message_ids("en")
        .into_iter()
        .filter(|id| id.starts_with("editor-") || id.starts_with("suite-"))
        .map(|id| {
            let message = TEXTS.text(&lang, &id, &[]);
            (id, message)
        })
        .collect();
    let body = serde_json::json!({ "lang": lang.code(), "texts": texts }).to_string();
    ([(header::CONTENT_TYPE, "application/json")], body).into_response()
}

fn routes(
    prefix: &str,
    shutdown: Arc<Notify>,
    fixed: Option<String>,
    project: PathBuf,
    job: Arc<ExportJob>,
) -> Router {
    let api = Arc::new(Api::new(project, fixed.clone(), Arc::clone(&job)));
    Router::new()
        .route(&format!("{prefix}/api/project"), get(api::get_project))
        .route(&format!("{prefix}/api/revision"), get(api::get_revision))
        .route(&format!("{prefix}/api/spec"), get(api::get_spec))
        .route(&format!("{prefix}/api/preview"), get(api::get_preview))
        .route(&format!("{prefix}/api/hit"), post(api::post_hit))
        .route(&format!("{prefix}/api/op/{{name}}"), post(api::post_op))
        .route(&format!("{prefix}/api/check"), get(api::get_check))
        .route(&format!("{prefix}/api/export"), post(api::post_export))
        .route(
            &format!("{prefix}/api/export/status"),
            get(api::get_export_status),
        )
        .route(
            &format!("{prefix}/api/export/cancel"),
            post(api::post_export_cancel),
        )
        .route(&format!("{prefix}/api/undo"), post(api::post_undo))
        .route(&format!("{prefix}/api/redo"), post(api::post_redo))
        .with_state(api)
        .route(&format!("{prefix}/"), get(index))
        .route(
            &format!("{prefix}/api/texts"),
            get(move |headers: HeaderMap| texts(fixed.clone(), headers)),
        )
        .route(&format!("{prefix}/assets/{{*path}}"), get(asset))
        .route(
            &format!("{prefix}/api/done"),
            post(move |body: axum::body::Bytes| {
                done(Arc::clone(&shutdown), Arc::clone(&job), body)
            }),
        )
}

/// Starts the server on `port`.
pub(super) async fn start(
    port: u16,
    project: PathBuf,
    fixed: Option<String>,
    job: Arc<ExportJob>,
) -> Result<LocalServer, WebError> {
    web::serve(
        move |prefix, shutdown| routes(prefix, shutdown, fixed, project, job),
        options(port),
    )
    .await
}

/// Starts the server, calls `announce` with its address, and waits until `POST api/done` or Ctrl+C ends it.
pub(super) fn serve_until_done(
    port: u16,
    project: PathBuf,
    fixed: Option<String>,
    announce: impl FnOnce(&str),
) -> Result<(), OpError> {
    let serve_error = |source: std::io::Error| OpError::Serve {
        source: anyhow::Error::new(source),
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(serve_error)?;
    runtime.block_on(async {
        let job = Arc::new(ExportJob::default());
        let server = start(port, project, fixed, Arc::clone(&job))
            .await
            .map_err(|e| OpError::Serve {
                source: anyhow::Error::new(e),
            })?;
        announce(&server.url);
        let stop = server.shutdown();
        let finished = server.finished();
        tokio::pin!(finished);
        let result = tokio::select! {
            result = &mut finished => result,
            _ = tokio::signal::ctrl_c() => {
                job.closing.store(true, std::sync::atomic::Ordering::SeqCst);
                stop.notify_one();
                finished.await
            }
        };
        result.map_err(serve_error)
    })
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use super::super::{EditArgs, run_with};
    use crate::ops::testing::viktoriapark;
    use crate::text::{Lang, Texts};

    /// A writer the test can read while `run_with` still runs.
    #[derive(Clone, Default)]
    struct Shared(Arc<Mutex<Vec<u8>>>);

    impl Write for Shared {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    struct Running {
        out: Shared,
        opened: Arc<AtomicUsize>,
        thread: std::thread::JoinHandle<Result<(), crate::ops::OpError>>,
        /// `127.0.0.1:port`
        host: String,
        /// `/token`
        prefix: String,
    }

    fn start(no_browser: bool) -> Running {
        start_with(no_browser, None)
    }

    fn start_with(no_browser: bool, lang: Option<&str>) -> Running {
        start_on(viktoriapark(), no_browser, lang)
    }

    fn start_on(project: std::path::PathBuf, no_browser: bool, lang: Option<&str>) -> Running {
        let lang = lang.map(str::to_owned);
        let out = Shared::default();
        let opened = Arc::new(AtomicUsize::new(0));
        let (writer, counter) = (out.clone(), Arc::clone(&opened));
        let args = EditArgs {
            project,
            port: 0,
            no_browser,
            lang,
        };
        let thread = std::thread::spawn(move || {
            let texts = Texts::new(&Lang::english());
            let mut writer = writer;
            run_with(&args, &texts, &mut writer, &|_| {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
        });
        let begin = Instant::now();
        let url = loop {
            let text = String::from_utf8(out.0.lock().unwrap().clone()).unwrap();
            if let Some(start) = text.find("http://127.0.0.1:") {
                let rest = &text[start..];
                let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
                break rest[..end].trim_end_matches('.').to_string();
            }
            assert!(begin.elapsed() < Duration::from_secs(10), "no address");
            std::thread::sleep(Duration::from_millis(20));
        };
        let rest = url.strip_prefix("http://").unwrap();
        let (host, path) = rest.split_once('/').unwrap();
        let prefix = format!("/{}", path.trim_end_matches('/'));
        Running {
            out,
            opened,
            thread,
            host: host.to_string(),
            prefix,
        }
    }

    /// Status line code and the whole answer, from a raw request.
    fn request(
        host: &str,
        request_host: &str,
        method: &str,
        path: &str,
        extra: &str,
    ) -> (u16, String) {
        request_with(host, request_host, method, path, extra, "{}")
    }

    /// One try of a raw request: the status line code and the whole answer.
    fn try_request(
        host: &str,
        request_host: &str,
        method: &str,
        path: &str,
        extra: &str,
        post_body: &str,
    ) -> std::io::Result<(u16, String)> {
        let mut stream = TcpStream::connect(host)?;
        // Long: a request that waits at a gate, or a sheet drawn on a loaded machine, can take its time.
        stream.set_read_timeout(Some(Duration::from_secs(90)))?;
        let body = if method == "POST" { post_body } else { "" };
        // One write: headers and body in one segment, so the server never answers a half-sent request.
        let request = format!(
            "{method} {path} HTTP/1.1\r\nHost: {request_host}\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(request.as_bytes())?;
        let mut answer = String::new();
        stream.read_to_string(&mut answer)?;
        let code = answer.split(' ').nth(1).unwrap().parse().unwrap();
        Ok((code, answer))
    }

    /// [`request`] with the body of a POST given. A GET that a loaded machine resets is tried again.
    fn request_with(
        host: &str,
        request_host: &str,
        method: &str,
        path: &str,
        extra: &str,
        post_body: &str,
    ) -> (u16, String) {
        let mut tries = if method == "GET" { 3 } else { 1 };
        loop {
            match try_request(host, request_host, method, path, extra, post_body) {
                Ok(answer) => return answer,
                Err(_) if tries > 1 => tries -= 1,
                Err(error) => panic!("{method} {path}: {error}"),
            }
        }
    }

    fn get(run: &Running, path: &str) -> (u16, String) {
        request(
            &run.host,
            &run.host,
            "GET",
            &format!("{}{path}", run.prefix),
            "",
        )
    }

    fn post_done(run: &Running, extra: &str) -> (u16, String) {
        request(
            &run.host,
            &run.host,
            "POST",
            &format!("{}/api/done", run.prefix),
            extra,
        )
    }

    fn finish(run: Running) {
        let origin = format!(
            "Origin: http://{}\r\nContent-Type: application/json\r\n",
            run.host
        );
        // Only this race is tolerated: the server shuts down as it answers `done`, so the answer can be lost to a
        // reset connection. Then the thread's end is the proof that `done` arrived. Every other request is read
        // in full by its handler and sent in one write, so a reset there is a real failure.
        let path = format!("{}/api/done", run.prefix);
        if let Ok((code, _)) = try_request(&run.host, &run.host, "POST", &path, &origin, "{}") {
            assert_eq!(code, 200);
        }
        // `done` must end the server; a hang here is a failed test, not a stuck run.
        let begin = Instant::now();
        while !run.thread.is_finished() {
            assert!(
                begin.elapsed() < Duration::from_secs(15),
                "the server did not end"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        run.thread.join().unwrap().unwrap();
    }

    #[test]
    fn the_printed_address_has_the_token_and_the_page_loads() {
        let run = start(true);
        let text = String::from_utf8(run.out.0.lock().unwrap().clone()).unwrap();
        let token = run.prefix.trim_start_matches('/');
        assert_eq!(token.len(), 32, "{text}");
        assert!(text.contains(token));
        let (code, answer) = get(&run, "/");
        assert_eq!(code, 200);
        assert!(answer.contains("suite/suite.css"));
        assert!(answer.contains("script-src 'self'"));
        assert!(answer.contains("form-action 'none'"));
        assert_eq!(get(&run, "/suite/suite.css").0, 200);
        assert_eq!(get(&run, "/suite/api.js").0, 200);
        assert_eq!(get(&run, "/assets/app.js").0, 200);
        assert_eq!(get(&run, "/assets/nothing.js").0, 404);
        finish(run);
    }

    #[test]
    fn a_wrong_token_is_404() {
        let run = start(true);
        let (code, _) = request(
            &run.host,
            &run.host,
            "GET",
            "/00000000000000000000000000000000/",
            "",
        );
        assert_eq!(code, 404);
        let (code, _) = request(&run.host, &run.host, "GET", "/", "");
        assert_eq!(code, 404);
        finish(run);
    }

    #[test]
    fn a_wrong_host_is_refused() {
        let run = start(true);
        let (code, _) = request(
            &run.host,
            "evil.example",
            "GET",
            &format!("{}/", run.prefix),
            "",
        );
        assert_eq!(code, 403);
        finish(run);
    }

    #[test]
    fn a_post_without_the_json_content_type_is_refused_and_the_server_goes_on() {
        let run = start(true);
        let origin = format!("Origin: http://{}\r\n", run.host);
        assert_eq!(post_done(&run, &origin).0, 403);
        let plain = format!("{origin}Content-Type: text/plain\r\n");
        assert_eq!(post_done(&run, &plain).0, 403);
        let no_origin = "Content-Type: application/json\r\n";
        assert_eq!(post_done(&run, no_origin).0, 403);
        // Still running.
        assert_eq!(get(&run, "/").0, 200);
        finish(run);
    }

    fn texts_with(run: &Running, accept: Option<&str>) -> serde_json::Value {
        let extra = accept.map_or(String::new(), |a| format!("Accept-Language: {a}\r\n"));
        let (code, answer) = request(
            &run.host,
            &run.host,
            "GET",
            &format!("{}/api/texts", run.prefix),
            &extra,
        );
        assert_eq!(code, 200);
        serde_json::from_str(answer.split("\r\n\r\n").nth(1).unwrap()).unwrap()
    }

    #[test]
    fn texts_come_as_lang_and_texts_in_the_language_order() {
        // The person's setting wins over the browser.
        let run = start_with(true, Some("de"));
        let answer = texts_with(&run, Some("en-US,en;q=0.9"));
        assert_eq!(answer["lang"], "de");
        assert_eq!(answer["texts"]["editor-menu-file"], "Datei");
        assert_eq!(answer["texts"]["suite-banner-dismiss"], "Schließen");
        finish(run);
        // Nothing set: the browser decides, the highest q-value first, else English.
        let run = start(true);
        assert_eq!(texts_with(&run, Some("de;q=0.5,en;q=0.8"))["lang"], "en");
        assert_eq!(texts_with(&run, Some("fr, de-AT;q=0.7"))["lang"], "de");
        let plain = texts_with(&run, Some("fr"));
        assert_eq!(plain["lang"], "en");
        assert_eq!(plain["texts"]["editor-menu-file"], "File");
        assert_eq!(texts_with(&run, None)["lang"], "en");
        finish(run);
        // A language the program has no texts in counts as unset.
        let run = start_with(true, Some("fr"));
        assert_eq!(texts_with(&run, Some("de"))["lang"], "de");
        finish(run);
    }

    #[test]
    fn the_language_the_person_set_is_the_argument_then_the_environment() {
        use super::super::fixed_language;
        let env = |name: &str| (name == "FREEMAPPER_LANG").then(|| "de".to_owned());
        assert_eq!(fixed_language(Some("en"), &env).as_deref(), Some("en"));
        assert_eq!(fixed_language(None, &env).as_deref(), Some("de"));
        let none = |_: &str| None;
        assert_eq!(fixed_language(None, &none), None);
        // LANG and LC_* do not fix the page's language.
        let locale = |name: &str| (name == "LANG").then(|| "de_DE.UTF-8".to_owned());
        assert_eq!(fixed_language(None, &locale), None);
    }

    #[test]
    fn a_post_without_origin_is_refused_even_with_a_revision() {
        let run = start(true);
        let extra = "Content-Type: application/json\r\nX-Freemapper-Revision: 0123456789abcdef\r\n";
        assert_eq!(post_done(&run, extra).0, 403);
        assert_eq!(get(&run, "/").0, 200);
        finish(run);
    }

    #[test]
    fn done_ends_run() {
        let run = start(true);
        assert!(!run.thread.is_finished());
        finish(run);
    }

    #[test]
    fn no_browser_opens_nothing_and_without_it_the_browser_opens_once() {
        let run = start(true);
        assert_eq!(run.opened.load(Ordering::SeqCst), 0);
        let opened = Arc::clone(&run.opened);
        finish(run);
        assert_eq!(opened.load(Ordering::SeqCst), 0);
        let run = start(false);
        assert_eq!(run.opened.load(Ordering::SeqCst), 1);
        finish(run);
    }

    #[test]
    fn a_missing_project_is_an_error_before_any_server_starts() {
        let args = EditArgs {
            project: "/nonexistent/freemapper-print-project".into(),
            port: 0,
            no_browser: true,
            lang: None,
        };
        let texts = Texts::new(&Lang::english());
        let mut out = Vec::new();
        let result = run_with(&args, &texts, &mut out, &|_| Ok(()));
        assert!(matches!(
            result,
            Err(crate::ops::OpError::LoadProject { .. })
        ));
        assert!(out.is_empty());
    }

    // ---- the read API ----

    use crate::ops::testing::{luebeck, luebeck_copy};

    /// The JSON body of an answer.
    fn body_json(answer: &str) -> serde_json::Value {
        serde_json::from_str(answer.split("\r\n\r\n").nth(1).unwrap()).unwrap()
    }

    fn post_json(run: &Running, path: &str, body: &str) -> (u16, String) {
        let extra = format!(
            "Origin: http://{}\r\nContent-Type: application/json\r\n",
            run.host
        );
        request_with(
            &run.host,
            &run.host,
            "POST",
            &format!("{}{path}", run.prefix),
            &extra,
            body,
        )
    }

    #[test]
    fn project_has_the_shape_of_the_project_info_and_the_files_revision() {
        let run = start(true);
        let (code, answer) = get(&run, "/api/project");
        assert_eq!(code, 200);
        let mut got = body_json(&answer);
        // The sources signature (length and time of the map and the course file) is checked on its own.
        let sources = got.as_object_mut().unwrap().remove("sources").unwrap();
        assert_eq!(sources.as_str().unwrap().split(',').count(), 2);
        let info = crate::ops::project::info(&crate::ops::project::InfoArgs {
            project: viktoriapark(),
        })
        .unwrap();
        let mut want = serde_json::to_value(&info).unwrap();
        let object = want.as_object_mut().unwrap();
        // The absolute path stays on the server.
        object.remove("path");
        let bytes = std::fs::read(viktoriapark().join("freemapper-print.toml")).unwrap();
        object.insert(
            "revision".into(),
            freemapper_print::web::revision(&bytes).into(),
        );
        object.insert("lang".into(), "en".into());
        object.insert("can_undo".into(), false.into());
        object.insert("can_redo".into(), false.into());
        assert_eq!(got, want);
        assert!(!answer.contains(env!("CARGO_MANIFEST_DIR")));
        let (_, revision) = get(&run, "/api/revision");
        assert_eq!(body_json(&revision)["revision"], want["revision"]);
        finish(run);
    }

    #[test]
    fn preview_is_the_renderers_svg_for_the_same_inputs() {
        let run = start(true);
        let (code, answer) = get(&run, "/api/preview?course=Training&rev=1");
        assert_eq!(code, 200);
        let got = body_json(&answer);
        let drawn = super::super::preview::draw(&viktoriapark(), Some("Training"), None).unwrap();
        assert_eq!(got["svg"], drawn.svg().unwrap());
        // The only style attribute the renderer writes is the blend of the overprint, on an element without a
        // class of its own: the page turns exactly that text into a class (`sheet.js`), because its CSP blocks
        // style attributes.
        let svg = got["svg"].as_str().unwrap();
        let blend = " style=\"mix-blend-mode:multiply\"";
        assert!(svg.contains(blend), "the course overprint multiplies");
        assert_eq!(svg.matches(" style=").count(), svg.matches(blend).count());
        for (at, _) in svg.match_indices(blend) {
            let tag = &svg[svg[..at].rfind('<').unwrap()..at];
            assert!(!tag.contains(" class="), "{tag}");
        }
        assert_eq!(
            got["page_mm"],
            serde_json::json!([drawn.page.page_mm.0, drawn.page.page_mm.1])
        );
        assert_eq!(got["course"], "Training");
        assert!(got["notes"].is_array());
        assert!(
            got["elements"]
                .as_array()
                .unwrap()
                .iter()
                .all(|e| e["rect_mm"].as_array().unwrap().len() == 4)
        );
        // An unknown course is refused with a sentence.
        let (code, answer) = get(&run, "/api/preview?course=Nope&rev=2");
        assert_eq!(code, 422);
        assert!(answer.contains("Nope"), "{answer}");
        finish(run);
    }

    #[test]
    fn a_project_without_courses_previews_the_layout() {
        let run = start_on(luebeck(), true, None);
        let (code, answer) = get(&run, "/api/preview");
        assert_eq!(code, 200);
        let got = body_json(&answer);
        assert!(got["course"].is_null());
        assert_eq!(got["layout"], "a4-4000");
        assert_eq!(got["page_mm"], serde_json::json!([297.0, 210.0]));
        let (_, answer) = get(&run, "/api/preview?layout=a3-3000");
        assert_eq!(
            body_json(&answer)["page_mm"],
            serde_json::json!([420.0, 297.0])
        );
        finish(run);
    }

    fn draws_of(project: &std::path::Path) -> usize {
        super::super::api::DRAWS
            .lock()
            .unwrap()
            .get(project)
            .copied()
            .unwrap_or(0)
    }

    #[test]
    fn a_newer_request_supersedes_an_older_one_of_the_same_page() {
        let dir = luebeck_copy();
        let run = start_on(dir.path().to_path_buf(), true, None);
        let project = dir.path().join("freemapper-print.toml");
        // Arrival: the newest number of a page wins, an older one is dropped at once.
        assert_eq!(
            get(&run, "/api/preview?layout=a4-4000&page=p1&rev=5").0,
            200
        );
        let (code, answer) = get(&run, "/api/preview?layout=a4-4000&page=p1&rev=4");
        assert_eq!(code, 409);
        assert_eq!(body_json(&answer)["error"], "superseded");
        // While waiting for its turn: the older request is held before it draws until the newer one is sent.
        let (reached_tx, reached_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        *super::super::api::GATE.lock().unwrap() = Some((project.clone(), reached_tx, release_rx));
        let drawn_before = draws_of(&project);
        let (host, prefix) = (run.host.clone(), run.prefix.clone());
        let older = std::thread::spawn(move || {
            request(
                &host,
                &host,
                "GET",
                &format!("{prefix}/api/preview?layout=a3-3000&page=p1&rev=10"),
                "",
            )
        });
        reached_rx.recv_timeout(Duration::from_secs(30)).unwrap();
        let (code, newer) = get(&run, "/api/preview?layout=a4-4000&page=p1&rev=11");
        assert_eq!(code, 200);
        assert_eq!(
            body_json(&newer)["page_mm"],
            serde_json::json!([297.0, 210.0])
        );
        let drawn_by_newer = draws_of(&project);
        release_tx.send(()).unwrap();
        let (code, answer) = older.join().unwrap();
        assert_eq!(code, 409, "{}", &answer[..answer.len().min(300)]);
        // The older request dropped out before it started: no sheet was drawn for it.
        assert_eq!(draws_of(&project), drawn_by_newer);
        assert!(drawn_by_newer >= drawn_before);
        // The superseded request did not replace the newer sheet in the cache: a click resolves on a4-4000 as before.
        let (code, _) = post_json(
            &run,
            "/api/hit",
            r#"{"layout":"a4-4000","x_mm":-9,"y_mm":-9}"#,
        );
        assert_eq!(code, 200);
        // Without a number nothing is superseded.
        assert_eq!(get(&run, "/api/preview?layout=a4-4000").0, 200);
        assert_eq!(get(&run, "/api/preview?rev=x").0, 400);
        assert_eq!(get(&run, "/api/preview?page=a%20b&rev=1").0, 400);
        finish(run);
    }

    #[test]
    fn two_pages_do_not_outdate_each_other() {
        // A second tab (or a reload) has an id of its own; neither page drops the other's requests, however its
        // numbers compare, and each gets its sheet.
        let run = start_on(luebeck(), true, None);
        assert_eq!(
            get(
                &run,
                "/api/preview?layout=a4-4000&page=tab1&rev=1700000050000"
            )
            .0,
            200
        );
        assert_eq!(
            get(&run, "/api/preview?layout=a3-3000&page=tab2&rev=3").0,
            200
        );
        assert_eq!(
            get(
                &run,
                "/api/preview?layout=a4-4000&page=tab1&rev=1700000050001"
            )
            .0,
            200
        );
        assert_eq!(
            get(&run, "/api/preview?layout=a3-3000&page=tab2&rev=4").0,
            200
        );
        // Within a page an older number is still dropped.
        assert_eq!(
            get(&run, "/api/preview?layout=a4-4000&page=tab2&rev=3").0,
            409
        );
        assert_eq!(
            get(
                &run,
                "/api/preview?layout=a4-4000&page=tab1&rev=1700000000000"
            )
            .0,
            409
        );
        finish(run);
    }

    #[test]
    fn errors_never_carry_an_absolute_path() {
        let dir = luebeck_copy();
        let run = start_on(dir.path().to_path_buf(), true, None);
        std::fs::remove_file(dir.path().join("luebeck.omap")).unwrap();
        for path in ["/api/project", "/api/preview?layout=a4-4000&rev=1"] {
            let (code, answer) = get(&run, path);
            assert_eq!(code, 422, "{path}");
            let body = answer.split("\r\n\r\n").nth(1).unwrap();
            assert!(!body.is_empty());
            assert!(!body.contains('/') && !body.contains(":\\"), "{body}");
        }
        let (code, answer) = post_json(
            &run,
            "/api/hit",
            r#"{"layout":"a4-4000","x_mm":1,"y_mm":1}"#,
        );
        assert_eq!(code, 422);
        assert!(!answer.split("\r\n\r\n").nth(1).unwrap().contains('/'));
        std::fs::remove_file(dir.path().join("freemapper-print.toml")).unwrap();
        for path in ["/api/revision", "/api/project"] {
            let (code, answer) = get(&run, path);
            assert_eq!(code, 422, "{path}");
            let body = answer.split("\r\n\r\n").nth(1).unwrap();
            assert!(!body.contains('/') && !body.contains(":\\"), "{body}");
        }
        finish(run);
    }

    #[test]
    fn a_map_saved_again_changes_the_sources_signature() {
        let dir = luebeck_copy();
        let run = start_on(dir.path().to_path_buf(), true, None);
        let (_, first) = get(&run, "/api/revision");
        let first = body_json(&first);
        let map = std::fs::File::options()
            .write(true)
            .open(dir.path().join("luebeck.omap"))
            .unwrap();
        map.set_modified(std::time::SystemTime::now() + Duration::from_secs(60))
            .unwrap();
        drop(map);
        let (_, second) = get(&run, "/api/revision");
        let second = body_json(&second);
        // The project file is the same, the map is not.
        assert_eq!(first["revision"], second["revision"]);
        assert_ne!(first["sources"], second["sources"]);
        // The sheet is drawn again for the new signature (the cache does not answer for the old one).
        assert_eq!(
            get(&run, "/api/preview?layout=a4-4000&rev=1700000000001").0,
            200
        );
        let (_, project) = get(&run, "/api/project");
        assert_eq!(body_json(&project)["sources"], second["sources"]);
        finish(run);
    }

    #[test]
    fn hit_finds_a_number_a_map_object_and_a_mask_through_the_route() {
        use super::super::preview::draw;
        // A number: Training of the Viktoriapark example.
        let run = start(true);
        let drawn = draw(&viktoriapark(), Some("Training"), None).unwrap();
        let (code, centre) = drawn
            .report
            .numbers
            .iter()
            .find(|(_, c)| drawn.pick(c.0, c.1).is_some_and(|p| p.kind == "number"))
            .map(|(code, c)| (code.clone(), *c))
            .unwrap();
        let body = format!(
            r#"{{"course":"Training","x_mm":{},"y_mm":{}}}"#,
            centre.0, centre.1
        );
        let (status, answer) = post_json(&run, "/api/hit", &body);
        assert_eq!(status, 200);
        let pick = body_json(&answer);
        assert_eq!(
            (pick["kind"].as_str(), pick["id"].as_str()),
            (Some("number"), Some(code.as_str()))
        );
        finish(run);
        // A map object and a mask: Fenster of the window example.
        let project = std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/examples/luebeck-training-window"
        ));
        let run = start_on(project.clone(), true, None);
        let drawn = draw(&project, Some("Fenster"), None).unwrap();
        let frame = drawn.page.clip;
        let mut wanted = std::collections::BTreeMap::new();
        for i in 0..80 {
            for j in 0..50 {
                let x = frame.min.0 + (frame.max.0 - frame.min.0) * f64::from(i) / 80.0;
                let y = frame.min.1 + (frame.max.1 - frame.min.1) * f64::from(j) / 50.0;
                if let Some(p) = drawn.pick(x, y) {
                    wanted.entry(p.kind).or_insert((x, y, p.id));
                }
            }
        }
        for kind in ["map_object", "mask"] {
            let (x, y, id) = wanted.get(kind).unwrap_or_else(|| panic!("no {kind}"));
            let body = format!(r#"{{"course":"Fenster","x_mm":{x},"y_mm":{y}}}"#);
            let (status, answer) = post_json(&run, "/api/hit", &body);
            assert_eq!(status, 200);
            let pick = body_json(&answer);
            assert_eq!(
                (pick["kind"].as_str(), pick["id"].as_str()),
                (Some(kind), Some(id.as_str()))
            );
        }
        finish(run);
    }

    #[test]
    fn hit_names_what_lies_at_a_point_and_needs_origin_and_json() {
        let run = start_on(luebeck(), true, None);
        let (_, answer) = get(&run, "/api/preview?layout=a4-4000");
        let got = body_json(&answer);
        let title = got["elements"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["id"] == "title")
            .unwrap();
        let r: Vec<f64> = title["rect_mm"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap())
            .collect();
        let body = format!(
            r#"{{"layout":"a4-4000","x_mm":{},"y_mm":{}}}"#,
            (r[0] + r[2]) / 2.0,
            (r[1] + r[3]) / 2.0
        );
        let (code, answer) = post_json(&run, "/api/hit", &body);
        assert_eq!(code, 200);
        let pick = body_json(&answer);
        assert_eq!(
            (pick["kind"].as_str(), pick["id"].as_str()),
            (Some("element"), Some("title"))
        );
        let (code, answer) = post_json(
            &run,
            "/api/hit",
            r#"{"layout":"a4-4000","x_mm":-9,"y_mm":-9}"#,
        );
        assert_eq!(code, 200);
        assert!(body_json(&answer)["kind"].is_null());
        assert_eq!(post_json(&run, "/api/hit", "not json").0, 400);
        let no_origin = request_with(
            &run.host,
            &run.host,
            "POST",
            &format!("{}/api/hit", run.prefix),
            "Content-Type: application/json\r\n",
            &body,
        );
        assert_eq!(no_origin.0, 403);
        finish(run);
    }

    /// The project file's revision as the server reports it now.
    fn revision_of(run: &Running) -> String {
        let (_, answer) = get(run, "/api/revision");
        body_json(&answer)["revision"].as_str().unwrap().to_owned()
    }

    /// `POST` with the revision header (`None`: without it).
    fn write(run: &Running, path: &str, revision: Option<&str>, body: &str) -> (u16, String) {
        let header = revision
            .map(|r| format!("X-Freemapper-Revision: {r}\r\n"))
            .unwrap_or_default();
        let extra = format!(
            "Origin: http://{}\r\nContent-Type: application/json\r\n{header}",
            run.host
        );
        request_with(
            &run.host,
            &run.host,
            "POST",
            &format!("{}{path}", run.prefix),
            &extra,
            body,
        )
    }

    /// A write that must succeed; the new revision.
    fn op(run: &Running, name: &str, body: &str) -> String {
        let revision = revision_of(run);
        let (code, answer) = write(run, &format!("/api/op/{name}"), Some(&revision), body);
        assert_eq!(code, 200, "{answer}");
        body_json(&answer)["revision"].as_str().unwrap().to_owned()
    }

    fn undo_or_redo(run: &Running, what: &str, revision: &str) -> (u16, String) {
        write(run, &format!("/api/{what}"), Some(revision), "{}")
    }

    #[test]
    fn an_edit_is_undone_to_the_exact_bytes_and_redone() {
        let dir = crate::ops::testing::luebeck_copy();
        let file = dir.path().join("freemapper-print.toml");
        let original = std::fs::read(&file).unwrap();
        let run = start_on(dir.path().to_path_buf(), true, Some("en"));
        let revision = op(&run, "set_layout", r#"{"id":"a4-4000","margin_mm":11.0}"#);
        let edited = std::fs::read(&file).unwrap();
        assert_ne!(edited, original);
        // Comments survive the edit.
        let text = String::from_utf8(edited.clone()).unwrap();
        assert!(text.contains("margin_mm = 11"), "{text}");
        assert_eq!(
            text.lines().filter(|l| l.starts_with('#')).count(),
            String::from_utf8(original.clone())
                .unwrap()
                .lines()
                .filter(|l| l.starts_with('#'))
                .count()
        );
        let (code, answer) = undo_or_redo(&run, "undo", &revision);
        assert_eq!(code, 200, "{answer}");
        let flags = body_json(&answer);
        assert_eq!(
            (flags["can_undo"].as_bool(), flags["can_redo"].as_bool()),
            (Some(false), Some(true))
        );
        assert_eq!(std::fs::read(&file).unwrap(), original);
        let reverted = flags["revision"].as_str().unwrap().to_owned();
        let (code, answer) = undo_or_redo(&run, "redo", &reverted);
        assert_eq!(code, 200, "{answer}");
        assert_eq!(std::fs::read(&file).unwrap(), edited);
        // Nothing left to redo.
        let again = body_json(&answer)["revision"].as_str().unwrap().to_owned();
        assert_eq!(undo_or_redo(&run, "redo", &again).0, 422);
        finish(run);
    }

    #[test]
    fn a_new_edit_clears_redo() {
        let dir = crate::ops::testing::luebeck_copy();
        let run = start_on(dir.path().to_path_buf(), true, Some("en"));
        let revision = op(&run, "set_layout", r#"{"id":"a4-4000","margin_mm":11.0}"#);
        let (_, answer) = undo_or_redo(&run, "undo", &revision);
        assert_eq!(body_json(&answer)["can_redo"], true);
        op(&run, "set_layout", r#"{"id":"a4-4000","margin_mm":12.0}"#);
        let (_, answer) = get(&run, "/api/revision");
        let flags = body_json(&answer);
        assert_eq!(
            (flags["can_undo"].as_bool(), flags["can_redo"].as_bool()),
            (Some(true), Some(false))
        );
        finish(run);
    }

    #[test]
    fn a_stale_revision_is_409_and_leaves_the_file_alone() {
        let dir = crate::ops::testing::luebeck_copy();
        let file = dir.path().join("freemapper-print.toml");
        let run = start_on(dir.path().to_path_buf(), true, Some("en"));
        let old = revision_of(&run);
        op(&run, "set_layout", r#"{"id":"a4-4000","margin_mm":11.0}"#);
        let bytes = std::fs::read(&file).unwrap();
        let (code, answer) = write(
            &run,
            "/api/op/set_layout",
            Some(&old),
            r#"{"id":"a4-4000","margin_mm":13.0}"#,
        );
        assert_eq!(code, 409);
        let body = body_json(&answer);
        assert_eq!(body["revision"], revision_of(&run));
        assert!(body["error"].as_str().unwrap().contains("changed"));
        assert_eq!(std::fs::read(&file).unwrap(), bytes);
        // Undo with the old revision is stale too.
        assert_eq!(undo_or_redo(&run, "undo", &old).0, 409);
        assert_eq!(std::fs::read(&file).unwrap(), bytes);
        finish(run);
    }

    #[test]
    fn writes_need_the_header_a_known_op_and_a_fitting_form() {
        let dir = crate::ops::testing::luebeck_copy();
        let run = start_on(dir.path().to_path_buf(), true, Some("en"));
        let revision = revision_of(&run);
        assert_eq!(
            write(&run, "/api/op/set_layout", None, r#"{"id":"a4-4000"}"#).0,
            400
        );
        assert_eq!(write(&run, "/api/undo", None, "{}").0, 400);
        assert_eq!(write(&run, "/api/op/nope", Some(&revision), "{}").0, 404);
        assert_eq!(
            write(
                &run,
                "/api/op/set_layout",
                Some(&revision),
                r#"{"id":"a4-4000","bogus":1}"#
            )
            .0,
            400
        );
        assert_eq!(
            write(&run, "/api/op/set_layout", Some(&revision), "nonsense").0,
            400
        );
        // A refused change: 422 with a sentence, file unchanged.
        let (code, answer) = write(
            &run,
            "/api/op/set_layout",
            Some(&revision),
            r#"{"id":"a4-4000","page_mm":[5,700]}"#,
        );
        assert_eq!(code, 422, "{answer}");
        assert_eq!(revision_of(&run), revision);
        let (code, answer) = write(
            &run,
            "/api/op/set_element",
            Some(&revision),
            r#"{"layout":"a4-4000","id":"x","kind":"logo","anchor":{"to":"page","corner":"top_left"},"file":"/etc/passwd"}"#,
        );
        assert_eq!(code, 422, "{answer}");
        assert!(!answer.contains("/etc/"), "{answer}");
        finish(run);
    }

    #[test]
    fn undo_refuses_when_the_file_changed_outside() {
        let dir = crate::ops::testing::luebeck_copy();
        let file = dir.path().join("freemapper-print.toml");
        let run = start_on(dir.path().to_path_buf(), true, Some("en"));
        let revision = op(&run, "set_layout", r#"{"id":"a4-4000","margin_mm":11.0}"#);
        let mut text = std::fs::read_to_string(&file).unwrap();
        text.push_str("\n# edited elsewhere\n");
        std::fs::write(&file, &text).unwrap();
        let (code, answer) = undo_or_redo(&run, "undo", &revision);
        assert_eq!(code, 422);
        assert!(answer.contains("outside the editor"), "{answer}");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), text);
        finish(run);
    }

    #[test]
    fn a_layout_form_sets_and_clears_the_free_page() {
        let dir = crate::ops::testing::luebeck_copy();
        let file = dir.path().join("freemapper-print.toml");
        let run = start_on(dir.path().to_path_buf(), true, Some("en"));
        op(
            &run,
            "set_layout",
            r#"{"id":"a4-4000","page_mm":[500.0,700.0]}"#,
        );
        assert!(
            std::fs::read_to_string(&file)
                .unwrap()
                .contains("page_mm = [500.0, 700.0]")
        );
        op(&run, "set_layout", r#"{"id":"a4-4000","page_mm":null}"#);
        assert!(!std::fs::read_to_string(&file).unwrap().contains("page_mm"));
        // Paper, orientation, scale, margin, border, north lines by their form values.
        op(
            &run,
            "set_layout",
            r#"{"id":"a4-4000","paper":"A3","orientation":"landscape","print_scale":5000,"margin_mm":9.0,"border_mm":0.3,"north_lines":"never"}"#,
        );
        let text = std::fs::read_to_string(&file).unwrap();
        for needle in ["\"A3\"", "landscape", "print_scale = 5000", "never"] {
            assert!(text.contains(needle), "{needle}: {text}");
        }
        finish(run);
    }

    #[test]
    fn moving_an_element_shifts_its_offset_for_every_corner_and_both_anchors() {
        use freemapper_print::project::{Anchor, AnchorTarget, Corner};
        let dir = crate::ops::testing::luebeck_copy();
        let run = start_on(dir.path().to_path_buf(), true, Some("en"));
        let corners = [
            ("top_left", Corner::TopLeft),
            ("top_right", Corner::TopRight),
            ("bottom_left", Corner::BottomLeft),
            ("bottom_right", Corner::BottomRight),
        ];
        let rect = |id: &str| -> Vec<f64> {
            let (_, answer) = get(&run, "/api/preview?layout=a4-4000");
            body_json(&answer)["elements"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["id"] == id)
                .unwrap_or_else(|| panic!("no {id}"))["rect_mm"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap())
                .collect()
        };
        for to in ["page", "map_frame"] {
            for (name, corner) in corners {
                let id = format!("m-{to}-{name}");
                op(
                    &run,
                    "set_element",
                    &format!(
                        r#"{{"layout":"a4-4000","id":"{id}","kind":"text","lines":["x"],"anchor":{{"to":"{to}","corner":"{name}","offset_mm":[20.0,30.0]}}}}"#
                    ),
                );
                let before = rect(&id);
                op(
                    &run,
                    "move_element",
                    &format!(r#"{{"layout":"a4-4000","id":"{id}","dx_mm":3.0,"dy_mm":2.0}}"#),
                );
                // The page's shift is the sheet's shift, whichever corner the element hangs from.
                let after = rect(&id);
                for i in 0..4 {
                    let want = if i % 2 == 0 { 3.0 } else { 2.0 };
                    assert!(
                        (after[i] - before[i] - want).abs() < 1e-6,
                        "{id}: {before:?} {after:?}"
                    );
                }
                let loaded =
                    crate::ops::load_project(&dir.path().join("freemapper-print.toml")).unwrap();
                let layout = loaded
                    .project
                    .layouts
                    .iter()
                    .find(|l| l.id == "a4-4000")
                    .unwrap();
                let anchor: Anchor = layout.elements.iter().find(|e| e.id == id).unwrap().anchor;
                let right = matches!(corner, Corner::TopRight | Corner::BottomRight);
                let bottom = matches!(corner, Corner::BottomLeft | Corner::BottomRight);
                assert_eq!(
                    anchor.to,
                    if to == "page" {
                        AnchorTarget::Page
                    } else {
                        AnchorTarget::MapFrame
                    }
                );
                assert_eq!(
                    anchor.offset_mm,
                    [
                        if right { 17.0 } else { 23.0 },
                        if bottom { 28.0 } else { 32.0 }
                    ],
                    "{id}"
                );
            }
        }
        // A shared element moves as a shared one; an unknown id is refused.
        let revision = revision_of(&run);
        let (code, _) = write(
            &run,
            "/api/op/move_element",
            Some(&revision),
            r#"{"layout":"a4-4000","id":"nothing","dx_mm":1,"dy_mm":1}"#,
        );
        assert_eq!(code, 422);
        finish(run);
    }

    #[test]
    fn an_edit_after_an_outside_change_keeps_that_change_through_both_undos() {
        let dir = crate::ops::testing::luebeck_copy();
        let file = dir.path().join("freemapper-print.toml");
        let run = start_on(dir.path().to_path_buf(), true, Some("en"));
        op(&run, "set_layout", r#"{"id":"a4-4000","margin_mm":11.0}"#);
        let mut outside = std::fs::read_to_string(&file).unwrap();
        outside.push_str("\n# outside change\n");
        std::fs::write(&file, &outside).unwrap();
        // The page reloads (reads the new revision) and edits again.
        op(&run, "set_layout", r#"{"id":"a4-4000","margin_mm":12.0}"#);
        let codes: Vec<u16> = (0..2)
            .map(|_| undo_or_redo(&run, "undo", &revision_of(&run)).0)
            .collect();
        // The second undo has nothing left: the older snapshot is gone, not written over the change.
        assert_eq!(codes, vec![200, 422]);
        assert!(
            std::fs::read_to_string(&file)
                .unwrap()
                .contains("# outside change")
        );
        finish(run);
    }

    #[test]
    fn logo_paths_outside_the_project_folder_are_refused() {
        let dir = crate::ops::testing::luebeck_copy();
        let run = start_on(dir.path().to_path_buf(), true, Some("en"));
        let revision = revision_of(&run);
        for file in ["../x.png", "a/../../x.png", "/abs/x.png"] {
            let body = format!(
                r#"{{"layout":"a4-4000","id":"l","kind":"logo","anchor":{{"to":"page","corner":"top_left"}},"file":{},"width_mm":20}}"#,
                serde_json::json!(file)
            );
            let (code, answer) = write(&run, "/api/op/set_element", Some(&revision), &body);
            assert_eq!(code, 422, "{file}: {answer}");
        }
        assert_eq!(revision_of(&run), revision);
        finish(run);
    }

    #[test]
    fn undo_after_the_project_file_vanished_is_refused_and_nothing_is_recreated() {
        let dir = crate::ops::testing::luebeck_copy();
        let file = dir.path().join("freemapper-print.toml");
        let run = start_on(dir.path().to_path_buf(), true, Some("en"));
        let revision = op(&run, "set_layout", r#"{"id":"a4-4000","margin_mm":11.0}"#);
        std::fs::remove_file(&file).unwrap();
        let (code, answer) = undo_or_redo(&run, "undo", &revision);
        assert_eq!(code, 422, "{answer}");
        assert!(!answer.contains(dir.path().to_str().unwrap()), "{answer}");
        assert!(!file.exists());
        finish(run);
    }

    #[test]
    fn two_writes_with_one_revision_give_one_200_and_one_409() {
        let dir = crate::ops::testing::luebeck_copy();
        let run = Arc::new(start_on(dir.path().to_path_buf(), true, Some("en")));
        let revision = revision_of(&run);
        let threads: Vec<_> = [11.0, 12.0]
            .into_iter()
            .map(|margin| {
                let (run, revision) = (Arc::clone(&run), revision.clone());
                std::thread::spawn(move || {
                    write(
                        &run,
                        "/api/op/set_layout",
                        Some(&revision),
                        &format!(r#"{{"id":"a4-4000","margin_mm":{margin}}}"#),
                    )
                    .0
                })
            })
            .collect();
        let mut codes: Vec<u16> = threads.into_iter().map(|t| t.join().unwrap()).collect();
        codes.sort_unstable();
        assert_eq!(codes, vec![200, 409]);
        finish(Arc::try_unwrap(run).ok().unwrap());
    }

    #[test]
    fn a_project_without_courses_takes_edits_and_undo() {
        let dir = crate::ops::testing::luebeck_copy();
        let file = dir.path().join("freemapper-print.toml");
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(
            !text.contains("courses ="),
            "the Luebeck layouts example has no course file"
        );
        let original = std::fs::read(&file).unwrap();
        let run = start_on(dir.path().to_path_buf(), true, Some("en"));
        let revision = op(&run, "set_layout", r#"{"id":"a4-4000","margin_mm":11.0}"#);
        assert_eq!(undo_or_redo(&run, "undo", &revision).0, 200);
        assert_eq!(std::fs::read(&file).unwrap(), original);
        finish(run);
    }

    /// The project file of a folder, as text.
    fn project_text(dir: &std::path::Path) -> String {
        std::fs::read_to_string(dir.join("freemapper-print.toml")).unwrap()
    }

    fn english() -> Texts {
        Texts::new(&Lang::english())
    }

    /// The error sentence of a refused write (422).
    fn refused_with(run: &Running, name: &str, body: &str) -> String {
        let revision = revision_of(run);
        let (code, answer) = write(run, &format!("/api/op/{name}"), Some(&revision), body);
        assert_eq!(code, 422, "{answer}");
        answer.split("\r\n\r\n").nth(1).unwrap().to_owned()
    }

    /// The Viktoriapark example with the number of control 34 put on its leg for the whole layout: a conflict
    /// with a proposal on the sheet of Training.
    fn conflicted_copy() -> tempfile::TempDir {
        let dir = crate::ops::testing::viktoriapark_copy();
        crate::ops::numbers::accept(&crate::ops::numbers::AcceptArgs {
            project: dir.path().to_path_buf(),
            course: "Training".into(),
            control: "34".into(),
            scope: crate::ops::numbers::AcceptScope::Layout,
            position: Some((0.0, 1.0)),
        })
        .unwrap();
        dir
    }

    #[test]
    fn a_window_mask_is_what_the_op_writes_and_an_id_is_never_replaced() {
        let by_page = crate::ops::testing::viktoriapark_copy();
        let by_op = crate::ops::testing::viktoriapark_copy();
        let run = start_on(by_page.path().to_path_buf(), true, Some("en"));
        let body = r#"{"course":"Training","controls":"34,35","radius":12.5,"radius_for":["35=20"],"gap":4,"no_start":true,"id":"w1","assign":true}"#;
        op(&run, "mask_window", body);
        crate::ops::masks::window(
            &crate::ops::masks::WindowArgs {
                project: by_op.path().to_path_buf(),
                course: "Training".into(),
                controls: Some("34,35".into()),
                radius_mm: Some(12.5),
                radius_for: vec!["35=20".into()],
                gap_mm: Some(4.0),
                no_start: true,
                no_finish: false,
                id: Some("w1".into()),
                assign: true,
            },
            &|r| crate::window_comment(&english(), r),
        )
        .unwrap();
        let text = project_text(by_page.path());
        assert_eq!(text, project_text(by_op.path()));
        assert!(text.contains("id = \"w1\""), "{text}");
        assert!(text.contains("at least 4 mm apart"), "{text}");
        // The same id again is refused with the sentence of the op, and the file stays as it is.
        let sentence = refused_with(&run, "mask_window", r#"{"course":"Training","id":"w1"}"#);
        assert!(sentence.contains("\"w1\" already"), "{sentence}");
        assert_eq!(project_text(by_page.path()), text);
        // Unknown keys and a missing course are a form error.
        let revision = revision_of(&run);
        let bad = write(
            &run,
            "/api/op/mask_window",
            Some(&revision),
            r#"{"controls":"1"}"#,
        );
        assert_eq!(bad.0, 400);
        finish(run);
    }

    #[test]
    fn a_corridor_from_a_typed_line_is_what_the_op_writes() {
        let by_page = crate::ops::testing::viktoriapark_copy();
        let by_op = crate::ops::testing::viktoriapark_copy();
        let run = start_on(by_page.path().to_path_buf(), true, Some("en"));
        let points = "10,-20;40,-25;70,-20";
        op(
            &run,
            "mask_corridor",
            &format!(r#"{{"points":"{points}","width":9,"course":"Training","assign":true}}"#),
        );
        crate::ops::masks::corridor(
            &crate::ops::masks::CorridorArgs {
                project: by_op.path().to_path_buf(),
                points: points.into(),
                width_mm: Some(9.0),
                id: None,
                assign: true,
                course: Some("Training".into()),
            },
            &|r| crate::corridor_comment(&english(), r),
        )
        .unwrap();
        assert_eq!(project_text(by_page.path()), project_text(by_op.path()));
        // The line needs two sources at most: text or a drawing, and assign needs a course.
        let revision = revision_of(&run);
        let both = write(
            &run,
            "/api/op/mask_corridor",
            Some(&revision),
            r#"{"points":"1,1;2,2","page_points_mm":[[1,1],[2,2]]}"#,
        );
        assert_eq!(both.0, 400);
        let sentence = refused_with(
            &run,
            "mask_corridor",
            r#"{"points":"1,1;2,2","assign":true}"#,
        );
        assert!(sentence.contains("assign needs a course"), "{sentence}");
        finish(run);
    }

    #[test]
    fn a_drawn_corridor_lies_where_the_pointer_was() {
        use super::super::preview::draw;
        let dir = crate::ops::testing::viktoriapark_copy();
        // The page point of the control "34" turns into the map point the window around it is centred on.
        let drawn = draw(dir.path(), Some("Training"), None).unwrap();
        let at = drawn
            .report
            .numbers
            .iter()
            .position(|(code, _)| code == "34")
            .unwrap();
        let centre = drawn.report.number_anchors[at].centre;
        let loaded = crate::ops::load_project(dir.path()).unwrap();
        let map = crate::ops::read_map(&loaded.map_path()).unwrap();
        let courses = crate::ops::read_courses(&loaded, &map).unwrap().unwrap();
        let course = &courses.event.courses[0];
        let (_, windows) = freemapper_print::mask_build::windows(
            "t",
            course,
            &courses.event,
            &freemapper_print::mask_build::WindowOptions {
                selection: freemapper_print::mask_build::Selection::Codes(vec!["34".into()]),
                start: false,
                finish: false,
                radius_mm: 10.0,
                per_control: Default::default(),
                gap_mm: freemapper_print::mask_build::DEFAULT_GAP_MM,
                map_mm_per_print_mm: 1.0,
            },
            None,
        )
        .unwrap();
        let wanted = windows[0].centre;
        let got = drawn.map_mm(centre.0, centre.1);
        assert!(
            (got[0] - wanted[0]).abs() < 0.01 && (got[1] - wanted[1]).abs() < 0.01,
            "{got:?} against {wanted:?}"
        );
        // Through the route: a corridor drawn on the sheet is the op with those map points.
        let by_op = crate::ops::testing::viktoriapark_copy();
        let run = start_on(dir.path().to_path_buf(), true, Some("en"));
        let drawn_points = [[centre.0, centre.1], [centre.0 + 20.0, centre.1 - 5.0]];
        let body = format!(
            r#"{{"page_points_mm":[[{},{}],[{},{}]],"course":"Training","layout":"{}","width":8}}"#,
            drawn_points[0][0],
            drawn_points[0][1],
            drawn_points[1][0],
            drawn_points[1][1],
            drawn.sheet.layout
        );
        op(&run, "mask_corridor", &body);
        let map_points: Vec<String> = drawn_points
            .iter()
            .map(|p| {
                let [x, y] = drawn.map_mm(p[0], p[1]);
                format!("{x},{y}")
            })
            .collect();
        crate::ops::masks::corridor(
            &crate::ops::masks::CorridorArgs {
                project: by_op.path().to_path_buf(),
                points: map_points.join(";"),
                width_mm: Some(8.0),
                id: None,
                assign: false,
                course: Some("Training".into()),
            },
            &|r| crate::corridor_comment(&english(), r),
        )
        .unwrap();
        assert_eq!(project_text(dir.path()), project_text(by_op.path()));
        finish(run);
    }

    /// A control whose number is in conflict on the sheet of Training, and the preview's list of such conflicts.
    #[test]
    fn the_preview_lists_the_numbers_in_conflict_and_their_centres() {
        let dir = conflicted_copy();
        let run = start_on(dir.path().to_path_buf(), true, Some("en"));
        let (code, answer) = get(&run, "/api/preview?course=Training");
        assert_eq!(code, 200);
        let preview = body_json(&answer);
        assert!(
            preview["numbers"].as_array().unwrap().len() > 3,
            "{preview}"
        );
        let conflicts = preview["conflicts"].as_array().unwrap();
        assert!(
            !conflicts.is_empty(),
            "the example has a number in conflict"
        );
        let first = &conflicts[0];
        assert_eq!(first["course"], "Training");
        assert!(
            first["message"]
                .as_str()
                .unwrap()
                .contains(first["control"].as_str().unwrap())
        );
        finish(run);
    }

    /// The anchor of control 34 on the sheet of Training in the Viktoriapark example.
    fn anchor_of_34(dir: &std::path::Path) -> freemapper_print::numbers::NumberAnchor {
        let drawn = super::super::preview::draw(dir, Some("Training"), None).unwrap();
        let at = drawn
            .report
            .numbers
            .iter()
            .position(|(code, _)| code == "34")
            .unwrap();
        drawn.report.number_anchors[at]
    }

    fn numbers_body(scope: &str, x: f64, y: f64) -> String {
        // The drop puts the centre of the number's box on the pointer; where the person grabbed the number does
        // not matter.
        format!(
            r#"{{"course":"Training","control":"34","scope":"{scope}","to_page_mm":[{x},{y}]}}"#
        )
    }

    fn stored_numbers(dir: &std::path::Path) -> Vec<freemapper_print::project::NumberCorrection> {
        freemapper_print::project::load(&dir.join("freemapper-print.toml"))
            .unwrap()
            .project
            .numbers
    }

    #[test]
    fn a_number_dropped_due_east_stores_direction_0_and_the_gap_for_the_sheet() {
        let dir = crate::ops::testing::viktoriapark_copy();
        let a = anchor_of_34(dir.path());
        let run = start_on(dir.path().to_path_buf(), true, Some("en"));
        // A clearance of 1.0 mm to the circle line, in print mm (the symbol factor `unit` turns it into page mm).
        let (x, y) = (a.centre.0 + a.outer_r + a.unit + a.size.0 / 2.0, a.centre.1);
        let body = numbers_body("sheet", x, y);
        let revision = op(&run, "numbers_accept", &body);
        let stored = stored_numbers(dir.path());
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].control, "34");
        assert_eq!(stored[0].course.as_deref(), Some("Training"));
        assert_eq!((stored[0].direction, stored[0].gap_mm), (0.0, Some(1.0)));
        // A second correction for the same control and sheet is refused with the op's sentence.
        let sentence = refused_with(&run, "numbers_accept", &body);
        assert!(
            sentence.contains("a correction for control 34"),
            "{sentence}"
        );
        // Undo takes it back.
        assert_eq!(undo_or_redo(&run, "undo", &revision).0, 200);
        assert!(!project_text(dir.path()).contains("[[numbers]]"));
        finish(run);
    }

    #[test]
    fn a_number_dropped_due_north_stores_direction_90_for_the_layout() {
        let dir = crate::ops::testing::viktoriapark_copy();
        let a = anchor_of_34(dir.path());
        let run = start_on(dir.path().to_path_buf(), true, Some("en"));
        // North is up on the page (y down) and 90 degrees, counter-clockwise from east.
        let (x, y) = (
            a.centre.0,
            a.centre.1 - (a.outer_r + 2.0 * a.unit + a.size.1 / 2.0),
        );
        op(&run, "numbers_accept", &numbers_body("layout_wide", x, y));
        let stored = stored_numbers(dir.path());
        assert!(stored[0].course.is_none() && stored[0].layout.is_some());
        assert_eq!((stored[0].direction, stored[0].gap_mm), (90.0, Some(2.0)));
        finish(run);
    }

    #[test]
    fn the_proposal_of_check_is_accepted_with_one_call() {
        let by_page = conflicted_copy();
        let by_op = conflicted_copy();
        let run = start_on(by_page.path().to_path_buf(), true, Some("en"));
        let (_, answer) = get(&run, "/api/preview?course=Training");
        let conflicts = body_json(&answer)["conflicts"].clone();
        let conflict = conflicts
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["searched"] == true)
            .expect("a conflict with a proposal");
        let control = conflict["control"].as_str().unwrap().to_owned();
        let revision = revision_of(&run);
        let (code, answer) = write(
            &run,
            "/api/op/numbers_accept",
            Some(&revision),
            &format!(r#"{{"course":"Training","control":"{control}","scope":"sheet"}}"#),
        );
        assert_eq!(code, 200, "{answer}");
        assert_eq!(body_json(&answer)["result"]["proposed"], true);
        crate::ops::numbers::accept(&crate::ops::numbers::AcceptArgs {
            project: by_op.path().to_path_buf(),
            course: "Training".into(),
            control,
            scope: crate::ops::numbers::AcceptScope::Sheet,
            position: None,
        })
        .unwrap();
        assert_eq!(project_text(by_page.path()), project_text(by_op.path()));
        finish(run);
    }

    #[test]
    fn absurd_values_are_refused_and_the_file_stays() {
        let dir = crate::ops::testing::viktoriapark_copy();
        let a = anchor_of_34(dir.path());
        let before = project_text(dir.path());
        let run = start_on(dir.path().to_path_buf(), true, Some("en"));
        for (name, body) in [
            // A drawn line far beyond any map.
            (
                "mask_corridor",
                r#"{"page_points_mm":[[1e308,1e308],[-1e308,5]],"course":"Training"}"#.to_owned(),
            ),
            (
                "mask_window",
                r#"{"course":"Training","radius":0}"#.to_owned(),
            ),
            (
                "mask_window",
                r#"{"course":"Training","radius":101}"#.to_owned(),
            ),
            // A gap out of range, and one that leaves 45 and 58 too close.
            (
                "mask_window",
                r#"{"course":"Training","gap":0.5}"#.to_owned(),
            ),
            (
                "mask_window",
                r#"{"course":"Training","gap":12}"#.to_owned(),
            ),
            // Dropped on its own circle: the gap would be zero or negative.
            (
                "numbers_accept",
                numbers_body("sheet", a.centre.0, a.centre.1),
            ),
        ] {
            refused_with(&run, name, &body);
            assert_eq!(project_text(dir.path()), before, "{name}");
        }
        finish(run);
    }

    #[test]
    fn a_number_drop_on_a_sheet_without_that_number_is_refused() {
        let dir = crate::ops::testing::viktoriapark_copy();
        let run = start_on(dir.path().to_path_buf(), true, Some("en"));
        let sentence = refused_with(
            &run,
            "numbers_accept",
            r#"{"course":"Training","control":"99","scope":"sheet","to_page_mm":[50,50]}"#,
        );
        assert!(sentence.contains("99"), "{sentence}");
        // A direction without a gap does not fit the op's form.
        let revision = revision_of(&run);
        let bad = write(
            &run,
            "/api/op/numbers_accept",
            Some(&revision),
            r#"{"course":"Training","control":"34","scope":"sheet","direction":10}"#,
        );
        assert_eq!(bad.0, 400);
        finish(run);
    }

    // ---- export, check and closing ----

    fn pdfs(dir: &std::path::Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    fn export_body(dir: &std::path::Path, rest: &str) -> String {
        format!(
            r#"{{"out_dir":{}{rest}}}"#,
            serde_json::to_string(&dir.display().to_string()).unwrap()
        )
    }

    #[test]
    fn an_export_through_the_page_equals_ops_export() {
        let project = crate::ops::testing::viktoriapark_copy();
        let (by_ops, by_page) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let expected = crate::ops::export::export(
            &crate::ops::export::ExportArgs {
                project: project.path().to_path_buf(),
                out_dir: Some(by_ops.path().to_path_buf()),
                ..Default::default()
            },
            &mut |_| std::ops::ControlFlow::Continue(()),
        )
        .unwrap();
        let run = start_on(project.path().to_path_buf(), true, Some("en"));
        let (code, answer) = post_json(&run, "/api/export", &export_body(by_page.path(), ""));
        assert_eq!(code, 200, "{answer}");
        let body = body_json(&answer);
        assert_eq!(body["stopped"], false);
        assert_eq!(body["files"][0]["path"], "Training.pdf");
        assert_eq!(body["files"][0]["bytes"], expected.files[0].bytes.unwrap());
        assert!(
            body["reminder"].as_str().unwrap().contains("100 %"),
            "{body}"
        );
        assert!(body["notes"][0]["severity"].is_string(), "{body}");
        assert_eq!(pdfs(by_ops.path()), pdfs(by_page.path()));
        let wrote = std::fs::read(by_page.path().join("Training.pdf")).unwrap();
        let reference = std::fs::read(by_ops.path().join("Training.pdf")).unwrap();
        assert!(wrote == reference, "the PDF differs from ops::export's");
        // The single file and a layout go through the same op.
        let single = tempfile::tempdir().unwrap();
        // The folder comes back as the person typed it, with its trailing slash.
        let typed = format!("{}/", single.path().display());
        let (code, answer) = post_json(
            &run,
            "/api/export",
            &export_body(std::path::Path::new(&typed), r#","single_file":true"#),
        );
        assert_eq!(code, 200, "{answer}");
        assert_eq!(body_json(&answer)["out_dir"], typed.as_str());
        assert_eq!(body_json(&answer)["files"][0]["pages"], 1);
        assert_eq!(pdfs(single.path()).len(), 1);
        finish(run);
    }

    #[test]
    fn an_export_refuses_a_missing_or_relative_folder_and_writes_nothing() {
        let run = start_on(viktoriapark(), true, Some("en"));
        let gone = std::env::temp_dir().join("freemapper-print-no-such-folder-9");
        let (code, answer) = post_json(&run, "/api/export", &export_body(&gone, ""));
        assert_eq!(code, 422, "{answer}");
        let body = answer.split("\r\n\r\n").nth(1).unwrap();
        assert!(
            body.contains("for the PDFs does not exist; create it first"),
            "{body}"
        );
        assert!(!gone.exists());
        for (dir, words) in [
            ("relative/out", "is not a full path"),
            ("", "Type the folder first"),
        ] {
            let (code, answer) =
                post_json(&run, "/api/export", &format!(r#"{{"out_dir":"{dir}"}}"#));
            assert_eq!(code, 422, "{answer}");
            let text = answer.split("\r\n\r\n").nth(1).unwrap();
            assert!(text.contains(words), "{text}");
            assert!(!text.contains("agent"), "{text}");
        }
        // Layouts do not go with courses; a form that does not fit is 400; a GET is no export.
        let here = tempfile::tempdir().unwrap();
        let (code, _) = post_json(
            &run,
            "/api/export",
            &export_body(here.path(), r#","layouts":["a4-4000"],"single_file":true"#),
        );
        assert_eq!(code, 422);
        assert_eq!(post_json(&run, "/api/export", "[]").0, 400);
        assert!(pdfs(here.path()).is_empty());
        // A write needs Origin like every other POST.
        let (code, _) = request_with(
            &run.host,
            &run.host,
            "POST",
            &format!("{}/api/export", run.prefix),
            "Content-Type: application/json\r\n",
            &export_body(here.path(), ""),
        );
        assert_eq!(code, 403);
        assert!(pdfs(here.path()).is_empty());
        finish(run);
    }

    #[test]
    fn a_cancelled_export_stops_after_the_sheet_in_hand() {
        let project = luebeck_copy();
        let out = tempfile::tempdir().unwrap();
        let run = start_on(project.path().to_path_buf(), true, Some("en"));
        let (reached_tx, reached_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let file = project.path().join("freemapper-print.toml");
        super::super::api::EXPORT_GATE
            .lock()
            .unwrap()
            .push((file, reached_tx, release_rx));
        let (host, prefix) = (run.host.clone(), run.prefix.clone());
        let body = export_body(out.path(), "");
        let export = std::thread::spawn(move || {
            let extra = format!("Origin: http://{host}\r\nContent-Type: application/json\r\n");
            request_with(
                &host,
                &host,
                "POST",
                &format!("{prefix}/api/export"),
                &extra,
                &body,
            )
        });
        // The first sheet is drawn and written; the export waits before it looks at the cancel flag.
        reached_rx.recv_timeout(Duration::from_secs(60)).unwrap();
        let status = body_json(&get(&run, "/api/export/status").1);
        assert_eq!(status["running"], true);
        assert_eq!(status["pages"], 1);
        // One export at a time.
        let (code, answer) = post_json(&run, "/api/export", &export_body(out.path(), ""));
        assert_eq!(code, 422, "{answer}");
        let (code, answer) = post_json(&run, "/api/export/cancel", "{}");
        assert_eq!(code, 200);
        assert_eq!(body_json(&answer)["running"], true);
        release_tx.send(()).unwrap();
        let (code, answer) = export.join().unwrap();
        assert_eq!(code, 200, "{answer}");
        let body = body_json(&answer);
        assert_eq!(body["stopped"], true, "{body}");
        assert_eq!(body["count"], 1);
        assert_eq!(pdfs(out.path()).len(), 1, "{:?}", pdfs(out.path()));
        let status = body_json(&get(&run, "/api/export/status").1);
        assert_eq!(status["running"], false);
        // Nothing runs now: cancelling does nothing, and the next export is not stopped by the old flag.
        let (_, answer) = post_json(&run, "/api/export/cancel", "{}");
        assert_eq!(body_json(&answer)["running"], false);
        let again = tempfile::tempdir().unwrap();
        let (code, answer) = post_json(&run, "/api/export", &export_body(again.path(), ""));
        assert_eq!(code, 200, "{answer}");
        assert!(pdfs(again.path()).len() > 1);
        finish(run);
    }

    #[test]
    fn check_returns_the_notes_with_severity_and_needs_attention() {
        let run = start_on(viktoriapark(), true, Some("en"));
        let (code, answer) = get(&run, "/api/check");
        assert_eq!(code, 200, "{answer}");
        let check = body_json(&answer);
        assert_eq!(check["needs_attention"], false);
        assert_eq!(check["courses"][0]["course"], "Training");
        let note = &check["courses"][0]["notes"][0];
        assert_eq!(note["severity"], "info");
        assert!(note["message"].is_string());
        assert!(!answer.contains(&viktoriapark().display().to_string()));
        finish(run);
        // A sheet that needs a look sets the flag.
        let project = luebeck_copy();
        let text = std::fs::read_to_string(project.path().join("freemapper-print.toml")).unwrap();
        std::fs::write(
            project.path().join("freemapper-print.toml"),
            text.replace("print_scale = 4000", "print_scale = 20000"),
        )
        .unwrap();
        let run = start_on(project.path().to_path_buf(), true, Some("en"));
        let (code, answer) = get(&run, "/api/check");
        assert_eq!(code, 200, "{answer}");
        let check = body_json(&answer);
        let severities: Vec<&str> = check["layouts"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|s| s["notes"].as_array().unwrap())
            .map(|n| n["severity"].as_str().unwrap())
            .collect();
        assert!(!severities.is_empty());
        assert_eq!(
            check["needs_attention"],
            severities.iter().any(|s| *s != "info")
        );
        finish(run);
    }

    #[test]
    fn ending_prints_the_project_file_the_person_gave() {
        let project = viktoriapark();
        let run = start_on(project.clone(), true, Some("en"));
        let out = run.out.clone();
        finish(run);
        let text = String::from_utf8(out.0.lock().unwrap().clone()).unwrap();
        let last = text.lines().last().unwrap();
        assert!(last.contains("The editor has ended"), "{text}");
        assert!(last.contains("freemapper-print.toml"), "{text}");
    }

    #[test]
    fn done_during_an_export_stops_it_after_the_sheet_in_hand_and_ends_the_server() {
        let project = luebeck_copy();
        let out = tempfile::tempdir().unwrap();
        let run = start_on(project.path().to_path_buf(), true, Some("en"));
        let (reached_tx, reached_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let file = project.path().join("freemapper-print.toml");
        super::super::api::EXPORT_GATE
            .lock()
            .unwrap()
            .push((file, reached_tx, release_rx));
        let (host, prefix) = (run.host.clone(), run.prefix.clone());
        let body = export_body(out.path(), "");
        let export = std::thread::spawn(move || {
            let extra = format!("Origin: http://{host}\r\nContent-Type: application/json\r\n");
            try_request(
                &host,
                &host,
                "POST",
                &format!("{prefix}/api/export"),
                &extra,
                &body,
            )
        });
        reached_rx.recv_timeout(Duration::from_secs(60)).unwrap();
        // `Fertig` while the first sheet is in hand. The answer may be lost to the shutdown.
        let extra = format!(
            "Origin: http://{}\r\nContent-Type: application/json\r\n",
            run.host
        );
        let path = format!("{}/api/done", run.prefix);
        if let Ok((code, _)) = try_request(&run.host, &run.host, "POST", &path, &extra, "{}") {
            assert_eq!(code, 200);
        }
        release_tx.send(()).unwrap();
        // The export ends after that sheet; its answer, if the connection still stands, says so.
        if let Ok((code, answer)) = export.join().unwrap() {
            assert_eq!(code, 200, "{answer}");
            assert_eq!(body_json(&answer)["stopped"], true, "{answer}");
        }
        let begin = Instant::now();
        while !run.thread.is_finished() {
            assert!(
                begin.elapsed() < Duration::from_secs(30),
                "the server did not end"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        run.thread.join().unwrap().unwrap();
        // Exactly one sheet, and it is a whole PDF.
        let names = pdfs(out.path());
        assert_eq!(names.len(), 1, "{names:?}");
        let bytes = std::fs::read(out.path().join(&names[0])).unwrap();
        assert!(bytes.starts_with(b"%PDF"));
        assert!(
            String::from_utf8_lossy(&bytes[bytes.len().saturating_sub(64)..]).contains("%%EOF")
        );
    }
}
