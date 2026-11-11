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

use super::api::{self, Api};
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
async fn done(done: Arc<Notify>) -> Response {
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

fn routes(prefix: &str, shutdown: Arc<Notify>, fixed: Option<String>, project: PathBuf) -> Router {
    let api = Arc::new(Api::new(project, fixed.clone()));
    Router::new()
        .route(&format!("{prefix}/api/project"), get(api::get_project))
        .route(&format!("{prefix}/api/revision"), get(api::get_revision))
        .route(&format!("{prefix}/api/preview"), get(api::get_preview))
        .route(&format!("{prefix}/api/hit"), post(api::post_hit))
        .with_state(api)
        .route(&format!("{prefix}/"), get(index))
        .route(
            &format!("{prefix}/api/texts"),
            get(move |headers: HeaderMap| texts(fixed.clone(), headers)),
        )
        .route(&format!("{prefix}/assets/{{*path}}"), get(asset))
        .route(
            &format!("{prefix}/api/done"),
            post(move || done(Arc::clone(&shutdown))),
        )
}

/// Starts the server on `port`.
pub(super) async fn start(
    port: u16,
    project: PathBuf,
    fixed: Option<String>,
) -> Result<LocalServer, WebError> {
    web::serve(
        move |prefix, shutdown| routes(prefix, shutdown, fixed, project),
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
        let server = start(port, project, fixed)
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
        stream.set_read_timeout(Some(Duration::from_secs(10)))?;
        let body = if method == "POST" { post_body } else { "" };
        write!(
            stream,
            "{method} {path} HTTP/1.1\r\nHost: {request_host}\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )?;
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
        // The server shuts down as it answers; on a loaded machine the answer can be lost to a reset
        // connection. Then the thread's end is the proof that `done` arrived.
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

    #[test]
    fn a_newer_request_supersedes_an_older_one() {
        let dir = luebeck_copy();
        let run = start_on(dir.path().to_path_buf(), true, None);
        // Arrival: the newest number wins, an older one is dropped at once.
        assert_eq!(get(&run, "/api/preview?layout=a4-4000&rev=5").0, 200);
        let (code, answer) = get(&run, "/api/preview?layout=a4-4000&rev=4");
        assert_eq!(code, 409);
        assert_eq!(body_json(&answer)["error"], "superseded");
        // While drawing: the older request waits inside the draw until the newer one has been sent.
        let (reached_tx, reached_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let project = dir.path().join("freemapper-print.toml");
        *super::super::api::GATE.lock().unwrap() = Some((project, reached_tx, release_rx));
        let (host, prefix) = (run.host.clone(), run.prefix.clone());
        let older = std::thread::spawn(move || {
            request(
                &host,
                &host,
                "GET",
                &format!("{prefix}/api/preview?layout=a3-3000&rev=10"),
                "",
            )
        });
        reached_rx.recv_timeout(Duration::from_secs(30)).unwrap();
        let (code, newer) = get(&run, "/api/preview?layout=a4-4000&rev=11");
        assert_eq!(code, 200);
        assert_eq!(
            body_json(&newer)["page_mm"],
            serde_json::json!([297.0, 210.0])
        );
        release_tx.send(()).unwrap();
        let (code, answer) = older.join().unwrap();
        assert_eq!(code, 409, "{}", &answer[..answer.len().min(300)]);
        // The superseded sheet did not replace the newer one in the cache: a click resolves on a4-4000 as before.
        let (code, _) = post_json(
            &run,
            "/api/hit",
            r#"{"layout":"a4-4000","x_mm":-9,"y_mm":-9}"#,
        );
        assert_eq!(code, 200);
        // Without a number nothing is superseded.
        assert_eq!(get(&run, "/api/preview?layout=a4-4000").0, 200);
        assert_eq!(get(&run, "/api/preview?rev=x").0, 400);
        finish(run);
    }

    #[test]
    fn a_page_opened_later_outranks_an_earlier_one() {
        // `rev` starts at the page's opening time (ms), so a reload or a second tab is never dropped by the
        // counter of the page before it; the page before it is the one that finds itself superseded.
        let run = start_on(luebeck(), true, None);
        assert_eq!(
            get(&run, "/api/preview?layout=a4-4000&rev=1700000000007").0,
            200
        );
        assert_eq!(
            get(&run, "/api/preview?layout=a4-4000&rev=1700000050000").0,
            200
        );
        assert_eq!(
            get(&run, "/api/preview?layout=a4-4000&rev=1700000000008").0,
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
            "/../../examples/luebeck-training-window"
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
}
