//! The server of the editor: the routes on top of `freemapper_web::serve`.
//!
//! Routes below the token: `GET /` (the page), `GET /assets/*` (the page's own files), `POST /api/done` (ends
//! the server). The suite assets at `/suite/*` come from `freemapper-web`.

use std::sync::Arc;

use axum::Router;
use axum::extract::Path;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use freemapper_print::web::{self, LocalServer, ServeOptions, WebError};
use tokio::sync::Notify;

use crate::ops::OpError;

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

fn routes(prefix: &str, shutdown: Arc<Notify>) -> Router {
    Router::new()
        .route(&format!("{prefix}/"), get(index))
        .route(&format!("{prefix}/assets/{{*path}}"), get(asset))
        .route(
            &format!("{prefix}/api/done"),
            post(move || done(Arc::clone(&shutdown))),
        )
}

/// Starts the server on `port`.
pub(super) async fn start(port: u16) -> Result<LocalServer, WebError> {
    web::serve(routes, options(port)).await
}

/// Starts the server, calls `announce` with its address, and waits until `POST api/done` or Ctrl+C ends it.
pub(super) fn serve_until_done(port: u16, announce: impl FnOnce(&str)) -> Result<(), OpError> {
    let serve_error = |source: std::io::Error| OpError::Serve {
        source: anyhow::Error::new(source),
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(serve_error)?;
    runtime.block_on(async {
        let server = start(port).await.map_err(|e| OpError::Serve {
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
        let out = Shared::default();
        let opened = Arc::new(AtomicUsize::new(0));
        let (writer, counter) = (out.clone(), Arc::clone(&opened));
        let args = EditArgs {
            project: viktoriapark(),
            port: 0,
            no_browser,
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
