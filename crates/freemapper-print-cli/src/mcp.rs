//! `freemapper-print mcp`: the MCP server on stdio, so an AI agent can read, check and (later) change and
//! export a project. The tools are thin wrappers over [`crate::ops`]; results are compact JSON, notes and
//! errors carry the same texts the CLI prints (ADR 0004). Nothing but JSON-RPC goes to stdout.

use std::path::{Path, PathBuf};

use fluent_bundle::FluentArgs;
use rmcp::handler::server::{router::tool::ToolRouter, wrapper::Parameters};
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig};
use rmcp::{ErrorData as McpError, ServerHandler, ServiceExt, tool, tool_handler, tool_router};
use serde::{Deserialize, Serialize};

use crate::ops;
use crate::ops::note::Note;
use crate::text::{Lang, Texts, detect_lang};
use crate::{op_error, printable};

/// The most notes `check_project` returns; `more` counts the rest.
const MAX_NOTES: usize = 200;

const INSTRUCTIONS: &str = "freemapper-print prints orienteering maps from OpenOrienteering Mapper \
files as PDFs at an exact scale, one sheet per course or per layout. A project is a folder with a \
freemapper-print.toml that names the map, the optional course file (Purple Pen or IOF XML) and the \
layouts. Start with project_info to see what a project holds, then check_project to see the notes \
of every sheet. Paths must be absolute because the server does not know your working folder. The \
messages in the results are German or English; pass lang to choose.";

/// The arguments of every read tool.
#[derive(Deserialize, rmcp::schemars::JsonSchema)]
struct ProjectArgs {
    /// Absolute path of freemapper-print.toml or of the folder that holds it.
    project: String,
    /// Language of the messages: "de" or "en". Default: the language the server started with.
    lang: Option<String>,
}

/// One sheet of `check_project`.
#[derive(Serialize)]
struct SheetOut {
    /// `drawn`, or `failed` when the sheet could not be drawn (its note says why).
    outcome: &'static str,
    course: Option<String>,
    layout: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    paper: Option<freemapper_print::print::Paper>,
    #[serde(skip_serializing_if = "Option::is_none")]
    print: Option<ops::PrintFacts>,
    needs_attention: bool,
    notes: Vec<Note>,
}

/// The result of `check_project`.
#[derive(Serialize)]
struct CheckOut {
    map_scale: u32,
    /// A note other than info, or a sheet that could not be drawn: the CLI's `check` exits with 1.
    needs_attention: bool,
    /// The notes of the course file as a whole; `None` without a course file.
    course_file: Option<Vec<Note>>,
    courses: Vec<SheetOut>,
    layouts: Vec<SheetOut>,
    /// How many notes were left out past [`MAX_NOTES`].
    #[serde(skip_serializing_if = "is_zero")]
    more: usize,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

/// The result of `list_courses`.
#[derive(Serialize)]
struct CoursesOut {
    courses_file: Option<PathBuf>,
    courses: Vec<ops::project::CourseInfo>,
}

/// Keeps at most a fixed number of notes over several lists and counts the rest.
struct NoteBudget {
    left: usize,
    cut: usize,
}

impl NoteBudget {
    fn new(max: usize) -> Self {
        NoteBudget { left: max, cut: 0 }
    }

    /// The first items of `items` that still fit.
    fn take<T>(&mut self, mut items: Vec<T>) -> Vec<T> {
        if items.len() > self.left {
            self.cut += items.len() - self.left;
            items.truncate(self.left);
        }
        self.left -= items.len();
        items
    }

    /// The notes in words, as many as still fit.
    fn describe(&mut self, texts: &Texts, notices: &[ops::Notice]) -> Vec<Note> {
        let kept = self.take(notices.iter().collect());
        kept.into_iter().map(|n| n.describe(texts)).collect()
    }
}

fn sheet_out(texts: &Texts, budget: &mut NoteBudget, sheet: &ops::check::SheetCheck) -> SheetOut {
    let needs_attention = sheet.needs_attention();
    match sheet {
        ops::check::SheetCheck::Drawn(s) => SheetOut {
            outcome: "drawn",
            course: s.course.clone(),
            layout: s.layout.clone(),
            paper: Some(s.paper),
            print: Some(s.print),
            needs_attention,
            notes: budget.describe(texts, &s.notes),
        },
        ops::check::SheetCheck::Failed {
            course,
            layout,
            error,
        } => SheetOut {
            outcome: "failed",
            course: course.clone(),
            layout: layout.clone(),
            paper: None,
            print: None,
            needs_attention,
            notes: budget.describe(texts, std::slice::from_ref(error)),
        },
    }
}

fn check_out(texts: &Texts, result: &ops::check::CheckResult) -> CheckOut {
    let mut budget = NoteBudget::new(MAX_NOTES);
    let course_file = result
        .course_file
        .as_ref()
        .map(|notes| budget.describe(texts, notes));
    let courses: Vec<SheetOut> = result
        .courses
        .iter()
        .map(|s| sheet_out(texts, &mut budget, s))
        .collect();
    let layouts: Vec<SheetOut> = result
        .layouts
        .iter()
        .map(|s| sheet_out(texts, &mut budget, s))
        .collect();
    CheckOut {
        map_scale: result.map_scale,
        needs_attention: result.course_file_needs_attention()
            || courses.iter().chain(&layouts).any(|s| s.needs_attention),
        course_file,
        courses,
        layouts,
        more: budget.cut,
    }
}

/// A path from the agent: absolute, not empty, without NUL. Relative paths are refused because the server
/// does not know the agent's working folder (ADR 0004).
fn absolute_path(texts: &Texts, path: &str) -> Result<PathBuf, McpError> {
    if path.is_empty() {
        return Err(McpError::invalid_params(
            texts.plain("mcp-error-empty-path"),
            None,
        ));
    }
    if path.contains('\0') {
        return Err(McpError::invalid_params(
            texts.get(
                "mcp-error-nul-path",
                Some(&args!("path" => printable(path))),
            ),
            None,
        ));
    }
    let resolved = PathBuf::from(path);
    if !resolved.is_absolute() {
        return Err(McpError::invalid_params(
            texts.get(
                "mcp-error-relative-path",
                Some(&args!("path" => printable(path))),
            ),
            None,
        ));
    }
    Ok(resolved)
}

/// A tool result with one text block.
fn text(text: impl Into<String>) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
}

/// A tool error the agent can relay to the user.
fn failure(message: impl Into<String>) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::error(vec![ContentBlock::text(message)]))
}

/// An op error in words, with its causes, as the CLI prints it after "Error:".
fn op_message(texts: &Texts, err: ops::OpError) -> String {
    printable(&format!("{:#}", op_error(texts, err)))
}

/// Compact JSON of a result.
fn json(texts: &Texts, value: &impl Serialize) -> Result<String, String> {
    serde_json::to_string(value)
        .map_err(|e| texts.get("mcp-error-internal", Some(&args!("error" => e.to_string()))))
}

#[derive(Clone)]
pub struct PrintServer {
    tool_router: ToolRouter<Self>,
    /// The language of the messages when a call names none.
    lang: Lang,
}

impl PrintServer {
    /// The texts for a call: its `lang`, else the server's.
    fn texts(&self, lang: Option<&str>) -> Result<Texts, String> {
        let Some(given) = lang else {
            return Ok(Texts::new(&self.lang));
        };
        detect_lang(Some(given), &|_| None)
            .map(|lang| Texts::new(&lang))
            .map_err(|unknown| {
                let available: Vec<String> = unknown.available.iter().map(Lang::code).collect();
                Texts::new(&self.lang).get(
                    "error-unknown-lang",
                    Some(&args!(
                        "lang" => printable(&unknown.given),
                        "available" => available.join(", "),
                    )),
                )
            })
    }

    /// Checks the arguments, then runs `op` off the async runtime: the ops read files and draw sheets. A
    /// panic in `op` comes back as a tool error, and the server keeps serving.
    async fn run(
        &self,
        args: ProjectArgs,
        op: impl FnOnce(&Texts, &Path) -> Result<String, String> + Send + 'static,
    ) -> Result<CallToolResult, McpError> {
        let texts = match self.texts(args.lang.as_deref()) {
            Ok(texts) => texts,
            Err(message) => return failure(message),
        };
        let path = match absolute_path(&texts, &args.project) {
            Ok(path) => path,
            Err(e) => return failure(e.message),
        };
        let lang = texts.lang().clone();
        match tokio::task::spawn_blocking(move || op(&texts, &path)).await {
            Ok(Ok(json)) => text(json),
            Ok(Err(message)) => failure(message),
            Err(e) => failure(Texts::new(&lang).get(
                "mcp-error-internal",
                Some(&args!("error" => printable(&panic_text(e)))),
            )),
        }
    }
}

/// What a failed blocking task says: the panic message if there is one.
fn panic_text(err: tokio::task::JoinError) -> String {
    if !err.is_panic() {
        return err.to_string();
    }
    let panic = err.into_panic();
    panic
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_default()
}

#[tool_router]
impl PrintServer {
    pub fn new(lang: Lang) -> Self {
        Self {
            tool_router: Self::tool_router(),
            lang,
        }
    }

    #[tool(
        description = "What a project holds: title, map kind and scale, the map and course file it \
                       reads, the layouts with their elements, the courses with the layout and masks \
                       each prints with, the mask variants and the assignments."
    )]
    async fn project_info(
        &self,
        Parameters(args): Parameters<ProjectArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.run(args, |texts, path| {
            let info = ops::project::info(&ops::project::InfoArgs {
                project: path.to_path_buf(),
            })
            .map_err(|e| op_message(texts, e))?;
            json(texts, &info)
        })
        .await
    }

    #[tool(
        description = "Draw every sheet of a project in memory, as the check command does, and \
                       return the notes of each sheet. With a course file there is one sheet per \
                       course, then one per layout without a course. Each note has kind, severity \
                       (info, warning or error), course and message. needs_attention is true when a \
                       note is more than info or a sheet could not be drawn. At most 200 notes come \
                       back; more counts the rest."
    )]
    async fn check_project(
        &self,
        Parameters(args): Parameters<ProjectArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.run(args, |texts, path| {
            let result = ops::check::check(&ops::check::CheckArgs {
                project: path.to_path_buf(),
            })
            .map_err(|e| op_message(texts, e))?;
            json(texts, &check_out(texts, &result))
        })
        .await
    }

    #[tool(
        description = "The courses of the project's course file in file order, each with its number \
                       of controls, the layout it prints on and the masks its sheet applies. The list \
                       is empty when the project has no course file."
    )]
    async fn list_courses(
        &self,
        Parameters(args): Parameters<ProjectArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.run(args, |texts, path| {
            let info = ops::project::info(&ops::project::InfoArgs {
                project: path.to_path_buf(),
            })
            .map_err(|e| op_message(texts, e))?;
            json(
                texts,
                &CoursesOut {
                    courses_file: info.courses_file,
                    courses: info.courses,
                },
            )
        })
        .await
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for PrintServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                "freemapper-print",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(INSTRUCTIONS)
    }
}

/// Serves MCP on `transport` until the client disconnects.
async fn serve<T, E, A>(lang: Lang, transport: T) -> anyhow::Result<()>
where
    T: rmcp::transport::IntoTransport<rmcp::RoleServer, E, A>,
    E: std::error::Error + Send + Sync + 'static,
{
    let service = PrintServer::new(lang).serve(transport).await?;
    service.waiting().await?;
    Ok(())
}

/// `freemapper-print mcp`: serves on stdin and stdout until stdin closes.
pub fn run(texts: &Texts) -> anyhow::Result<u8> {
    let lang = texts.lang().clone();
    tokio::runtime::Runtime::new()?.block_on(serve(lang, rmcp::transport::stdio()))?;
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::testing::viktoriapark;
    use serde_json::{Value, json};
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream, ReadHalf, WriteHalf};

    /// An MCP client in the same process, over a pipe: the messages an agent sends, line by line.
    struct Client {
        lines: tokio::io::Lines<BufReader<ReadHalf<DuplexStream>>>,
        writer: WriteHalf<DuplexStream>,
        next_id: u64,
    }

    impl Client {
        async fn start() -> Client {
            let (ours, theirs) = tokio::io::duplex(1 << 16);
            tokio::spawn(serve(Lang::english(), theirs));
            let (reader, writer) = tokio::io::split(ours);
            let mut client = Client {
                lines: BufReader::new(reader).lines(),
                writer,
                next_id: 1,
            };
            let info = client
                .request(
                    "initialize",
                    json!({
                        "protocolVersion": "2025-06-18",
                        "capabilities": {},
                        "clientInfo": { "name": "test", "version": "1" }
                    }),
                )
                .await;
            assert_eq!(
                info["result"]["serverInfo"]["name"], "freemapper-print",
                "{info}"
            );
            client
                .send(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))
                .await;
            client
        }

        async fn send(&mut self, message: Value) {
            let line = format!("{message}\n");
            self.writer.write_all(line.as_bytes()).await.unwrap();
            self.writer.flush().await.unwrap();
        }

        /// The whole answer to a request.
        async fn request(&mut self, method: &str, params: Value) -> Value {
            let id = self.next_id;
            self.next_id += 1;
            self.send(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))
                .await;
            loop {
                let line = self
                    .lines
                    .next_line()
                    .await
                    .unwrap()
                    .expect("server closed");
                let message: Value = serde_json::from_str(&line).unwrap();
                if message["id"] == id {
                    return message;
                }
            }
        }

        /// Calls a tool; returns whether it failed and the text of its first content block.
        async fn call(&mut self, tool: &str, arguments: Value) -> (bool, String) {
            let answer = self
                .request(
                    "tools/call",
                    json!({ "name": tool, "arguments": arguments }),
                )
                .await;
            let result = &answer["result"];
            assert!(result.is_object(), "{answer}");
            let text = result["content"][0]["text"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            (result["isError"] == true, text)
        }
    }

    fn project() -> String {
        viktoriapark().to_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn lists_the_three_read_tools() {
        let mut client = Client::start().await;
        let answer = client.request("tools/list", json!({})).await;
        let tools = answer["result"]["tools"].as_array().unwrap();
        let mut names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        names.sort();
        assert_eq!(names, ["check_project", "list_courses", "project_info"]);
        for tool in tools {
            assert!(tool["description"].as_str().unwrap().len() > 40, "{tool}");
            let schema = &tool["inputSchema"];
            assert!(schema["properties"]["project"].is_object(), "{tool}");
            assert!(schema["properties"]["lang"].is_object(), "{tool}");
            assert_eq!(schema["required"], json!(["project"]), "{tool}");
        }
    }

    #[tokio::test]
    async fn project_info_describes_viktoriapark() {
        let mut client = Client::start().await;
        let (error, text) = client
            .call("project_info", json!({ "project": project() }))
            .await;
        assert!(!error, "{text}");
        assert!(!text.contains('\n'), "compact JSON: {text}");
        let info: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(info["title"], "Training Viktoriapark");
        assert_eq!(info["map_scale"], 4000);
        assert_eq!(info["courses"][0]["name"], "Training");
        assert_eq!(info["courses"][0]["controls"], 11);
        assert_eq!(info["courses"][0]["layout"], "a4-4000");
        assert_eq!(info["layouts"][0]["id"], "a4-4000");
        // The project file itself works as well as its folder.
        let file = viktoriapark().join("freemapper-print.toml");
        let (error, text) = client
            .call("project_info", json!({ "project": file }))
            .await;
        assert!(!error, "{text}");
    }

    #[tokio::test]
    async fn list_courses_names_the_course() {
        let mut client = Client::start().await;
        let (error, text) = client
            .call("list_courses", json!({ "project": project() }))
            .await;
        assert!(!error, "{text}");
        let list: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(list["courses_file"], "training.ppen");
        assert_eq!(
            list["courses"],
            json!([{ "name": "Training", "controls": 11, "layout": "a4-4000", "masks": [] }])
        );
    }

    #[tokio::test]
    async fn check_project_returns_notes_with_messages() {
        let mut client = Client::start().await;
        let (error, text) = client
            .call("check_project", json!({ "project": project() }))
            .await;
        assert!(!error, "{text}");
        let check: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(check["map_scale"], 4000);
        assert_eq!(check["needs_attention"], false);
        assert!(check.get("more").is_none(), "{check}");
        let course = &check["courses"][0];
        assert_eq!(course["outcome"], "drawn");
        assert_eq!(course["course"], "Training");
        assert_eq!(course["layout"], "a4-4000");
        let note = &course["notes"][0];
        let mut keys: Vec<&str> = note
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort();
        assert_eq!(keys, ["course", "kind", "message", "severity"]);
        assert_eq!(note["kind"], "map_cropped");
        assert_eq!(note["severity"], "info");
        assert_eq!(note["course"], "Training");
        assert!(!note["message"].as_str().unwrap().is_empty(), "{note}");
        assert_eq!(check["course_file"][0]["kind"], "fit_georef_skipped");
        assert_eq!(check["layouts"][0]["notes"][0]["course"], Value::Null);
        // The messages follow lang.
        let (_, en) = client
            .call(
                "check_project",
                json!({ "project": project(), "lang": "en" }),
            )
            .await;
        let (_, de) = client
            .call(
                "check_project",
                json!({ "project": project(), "lang": "de" }),
            )
            .await;
        let message = |text: &str| {
            serde_json::from_str::<Value>(text).unwrap()["courses"][0]["notes"][0]["message"]
                .as_str()
                .unwrap()
                .to_string()
        };
        assert_ne!(message(&en), message(&de));
    }

    #[tokio::test]
    async fn relative_and_unusable_paths_are_refused() {
        let mut client = Client::start().await;
        for tool in ["project_info", "check_project", "list_courses"] {
            let (error, text) = client
                .call(
                    tool,
                    json!({ "project": "examples/berlin-viktoriapark-training" }),
                )
                .await;
            assert!(error, "{tool}");
            assert!(
                text.contains("relative")
                    && text.contains("absolute")
                    && text.contains("working folder"),
                "{text}"
            );
        }
        let (error, text) = client.call("project_info", json!({ "project": "" })).await;
        assert!(error && text.contains("empty"), "{text}");
        let (error, text) = client
            .call("project_info", json!({ "project": "/tmp/a\u{0}b" }))
            .await;
        assert!(error && text.contains("NUL"), "{text}");
        let (error, text) = client
            .call(
                "project_info",
                json!({ "project": "relativ", "lang": "de" }),
            )
            .await;
        assert!(error && text.contains("relativer Pfad"), "{text}");
        let (error, text) = client
            .call(
                "project_info",
                json!({ "project": project(), "lang": "fr" }),
            )
            .await;
        assert!(
            error && text.contains("\"fr\"") && text.contains("de"),
            "{text}"
        );
    }

    #[tokio::test]
    async fn missing_or_broken_project_gives_a_clear_error() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing");
        let mut client = Client::start().await;
        let (error, text) = client
            .call("check_project", json!({ "project": missing }))
            .await;
        assert!(error, "{text}");
        assert!(
            text.contains("could not load the project") && text.contains("missing"),
            "{text}"
        );
        let broken = dir.path().join("broken");
        std::fs::create_dir(&broken).unwrap();
        std::fs::write(broken.join("freemapper-print.toml"), "this is [no toml").unwrap();
        let (error, text) = client
            .call("project_info", json!({ "project": broken }))
            .await;
        assert!(error, "{text}");
        assert!(text.contains("could not load the project"), "{text}");
        // The server still answers.
        let (error, text) = client
            .call("list_courses", json!({ "project": project() }))
            .await;
        assert!(!error, "{text}");
    }

    #[test]
    fn note_budget_keeps_the_first_and_counts_the_rest() {
        let mut budget = NoteBudget::new(5);
        assert_eq!(budget.take(vec![1, 2, 3]), [1, 2, 3]);
        assert_eq!(budget.take(vec![4, 5, 6, 7]), [4, 5]);
        assert_eq!(budget.take(vec![8]), Vec::<i32>::new());
        assert_eq!(budget.cut, 3);
    }

    #[test]
    fn absolute_path_refuses_relative_empty_and_nul() {
        let texts = Texts::new(&Lang::english());
        let absolute = std::env::temp_dir().join("project");
        assert_eq!(
            absolute_path(&texts, absolute.to_str().unwrap()).unwrap(),
            absolute
        );
        for (path, expected) in [
            ("project", "relative"),
            ("./project", "relative"),
            ("", "empty"),
            ("/a\u{0}b", "NUL"),
        ] {
            let err = absolute_path(&texts, path).unwrap_err();
            assert!(err.message.contains(expected), "{path:?}: {}", err.message);
        }
    }
}
