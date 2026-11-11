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
use tokio::sync::Notify;

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

fn routes(prefix: &str, shutdown: Arc<Notify>, fixed: Option<String>) -> Router {
    Router::new()
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
pub(super) async fn start(port: u16, fixed: Option<String>) -> Result<LocalServer, WebError> {
    web::serve(
        move |prefix, shutdown| routes(prefix, shutdown, fixed),
        options(port),
    )
    .await
}

/// Starts the server, calls `announce` with its address, and waits until `POST api/done` or Ctrl+C ends it.
pub(super) fn serve_until_done(
    port: u16,
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
        let server = start(port, fixed).await.map_err(|e| OpError::Serve {
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
        let lang = lang.map(str::to_owned);
        let out = Shared::default();
        let opened = Arc::new(AtomicUsize::new(0));
        let (writer, counter) = (out.clone(), Arc::clone(&opened));
        let args = EditArgs {
            project: viktoriapark(),
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
        let mut stream = TcpStream::connect(host).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let body = if method == "POST" { "{}" } else { "" };
        write!(
            stream,
            "{method} {path} HTTP/1.1\r\nHost: {request_host}\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        let mut answer = String::new();
        stream.read_to_string(&mut answer).unwrap();
        let code = answer.split(' ').nth(1).unwrap().parse().unwrap();
        (code, answer)
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
        assert_eq!(post_done(&run, &origin).0, 200);
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
}
