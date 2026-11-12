//! `freemapper-print mcp` spoken to over stdio, as an agent would: stdout carries JSON-RPC and nothing else.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

#[cfg(all(feature = "test-stub", debug_assertions))]
mod support;

const VIKTORIAPARK: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/berlin-viktoriapark-training"
);

#[test]
fn stdout_carries_only_json_rpc_and_eof_ends_the_server() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_freemapper-print"))
        .args(["mcp", "--lang", "en"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start the server");
    let messages = [
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "test", "version": "1" }
            }
        }),
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} }),
        json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": { "name": "check_project", "arguments": { "project": VIKTORIAPARK } }
        }),
        json!({
            "jsonrpc": "2.0", "id": 4, "method": "tools/call",
            "params": { "name": "project_info", "arguments": { "project": "relative/path" } }
        }),
    ];
    let mut stdin = child.stdin.take().unwrap();
    for message in &messages {
        writeln!(stdin, "{message}").unwrap();
    }
    stdin.flush().unwrap();
    // Wait for every answer, then close stdin: the server must end by itself.
    let stdout = BufReader::new(child.stdout.take().unwrap());
    let (lines, received) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in stdout.lines() {
            if lines.send(line.unwrap()).is_err() {
                break;
            }
        }
    });
    let mut out = Vec::new();
    let mut answered = 0;
    while answered < 4 {
        let line = received
            .recv_timeout(Duration::from_secs(120))
            .expect("an answer within two minutes");
        if line.contains("\"id\"") {
            answered += 1;
        }
        out.push(line);
    }
    drop(stdin);
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "the server did not end after EOF"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    reader.join().unwrap();
    out.extend(received.try_iter());
    let out = out.join("\n");
    let mut err = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut err)
        .unwrap();
    assert!(status.success(), "{status}: {err}");
    let mut answered = Vec::new();
    for line in out.lines() {
        let message: Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("not JSON on stdout ({e}): {line:?}"));
        assert_eq!(message["jsonrpc"], "2.0", "{line}");
        if let Some(id) = message["id"].as_u64() {
            answered.push(id);
        }
    }
    answered.sort();
    assert_eq!(answered, [1, 2, 3, 4], "{out}\n{err}");
    let answer = |id: u64| -> Value {
        out.lines()
            .map(|l| serde_json::from_str::<Value>(l).unwrap())
            .find(|m| m["id"] == id)
            .unwrap()
    };
    assert_eq!(
        answer(1)["result"]["serverInfo"]["name"],
        "freemapper-print"
    );
    assert_eq!(answer(2)["result"]["tools"].as_array().unwrap().len(), 13);
    let check = &answer(3)["result"];
    assert_ne!(check["isError"], true, "{check}");
    let text = check["content"][0]["text"].as_str().unwrap();
    let result: Value = serde_json::from_str(text).unwrap();
    assert_eq!(result["courses"][0]["course"], "Training");
    let refused = &answer(4)["result"];
    assert_eq!(refused["isError"], true, "{refused}");
    assert!(
        refused["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("relative path"),
        "{refused}"
    );
}

#[test]
fn mcp_help_follows_the_language() {
    let run = |lang: &str| {
        let o = Command::new(env!("CARGO_BIN_EXE_freemapper-print"))
            .args(["mcp", "--help", "--lang", lang])
            .output()
            .unwrap();
        String::from_utf8_lossy(&o.stdout).into_owned()
    };
    let en = run("en");
    assert!(
        en.contains("over MCP") && en.contains("must be absolute"),
        "{en}"
    );
    let de = run("de");
    assert!(
        de.contains("über MCP") && de.contains("müssen absolut sein"),
        "{de}"
    );
}

const LUEBECK: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/luebeck-competition-courses"
);

/// A server on stdio with the answers on stdout read line by line on a thread.
struct Server {
    child: std::process::Child,
    stdin: Option<std::process::ChildStdin>,
    lines: std::sync::mpsc::Receiver<String>,
    /// The id of the next request made by [`Server::call_all`].
    next: u64,
}

impl Server {
    fn start(env: &[(&str, &str)]) -> Server {
        let mut command = Command::new(env!("CARGO_BIN_EXE_freemapper-print"));
        command
            .env_remove("FREEMAPPER_PRINT_UPDATE")
            .env_remove("FREEMAPPER_PRINT_UPDATE_TEST_BASE");
        for (key, value) in env {
            command.env(key, value);
        }
        Server::start_command(command)
    }

    /// `command` with `mcp --lang en` added, without `CI` and the update switches of the environment.
    fn start_command(mut command: Command) -> Server {
        command
            .args(["mcp", "--lang", "en"])
            .env_remove("CI")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().expect("start the server");
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let (send, lines) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for line in stdout.lines() {
                if send.send(line.unwrap()).is_err() {
                    break;
                }
            }
        });
        let mut server = Server {
            stdin: child.stdin.take(),
            child,
            lines,
            next: 100,
        };
        server.send(json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "test", "version": "1" }
            }
        }));
        server.answer(1);
        server.send(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
        server
    }

    fn send(&mut self, message: Value) {
        let stdin = self.stdin.as_mut().unwrap();
        writeln!(stdin, "{message}").unwrap();
        stdin.flush().unwrap();
    }

    fn answer(&mut self, id: u64) -> Value {
        loop {
            let line = self
                .lines
                .recv_timeout(Duration::from_secs(120))
                .expect("an answer within two minutes");
            let message: Value = serde_json::from_str(&line).unwrap();
            if message["id"] == id {
                return message;
            }
        }
    }

    /// Calls a tool and returns whether it failed and the texts of its content blocks.
    fn call_all(&mut self, name: &str, arguments: Value) -> (bool, Vec<String>) {
        self.next += 1;
        let id = self.next;
        self.send(json!({
            "jsonrpc": "2.0", "id": id, "method": "tools/call",
            "params": { "name": name, "arguments": arguments }
        }));
        let answer = self.answer(id);
        let result = &answer["result"];
        let texts = result["content"]
            .as_array()
            .unwrap_or_else(|| panic!("no content: {answer}"))
            .iter()
            .map(|block| block["text"].as_str().unwrap().to_string())
            .collect();
        (result["isError"] == true, texts)
    }

    #[cfg(all(feature = "test-stub", debug_assertions))]
    /// Closes stdin, waits for the end of the server and returns its stderr and the stdout lines not read yet.
    fn close(mut self) -> (String, Vec<String>) {
        drop(self.stdin.take());
        let deadline = Instant::now() + Duration::from_secs(30);
        while self.child.try_wait().unwrap().is_none() {
            assert!(
                Instant::now() < deadline,
                "the server did not end after EOF"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        let mut stderr = String::new();
        self.child
            .stderr
            .take()
            .unwrap()
            .read_to_string(&mut stderr)
            .unwrap();
        let rest: Vec<String> = self.lines.try_iter().collect();
        (stderr, rest)
    }

    fn call_export(&mut self, id: u64, arguments: Value) {
        self.send(json!({
            "jsonrpc": "2.0", "id": id, "method": "tools/call",
            "params": { "name": "export_project", "arguments": arguments }
        }));
    }
}

/// The names in `dir`, sorted.
fn names_in(dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// The hostile case of ADR 0040: 1000 courses of 200 controls each on a copy of the Lübeck map, all on one A3
/// layout (the same file as the unit tests in `src/mcp.rs` build).
fn hostile_project() -> tempfile::TempDir {
    let from = std::path::Path::new(LUEBECK);
    let dir = tempfile::tempdir().unwrap();
    for name in ["luebeck.omap", "freemapper.toml"] {
        std::fs::copy(from.join(name), dir.path().join(name)).unwrap();
    }
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<CourseData \
         xmlns=\"http://www.orienteering.org/datastandard/3.0\" iofVersion=\"3.0\">\n\
         <RaceCourseData><Map><Scale>4000</Scale></Map>\n\
         <Control type=\"Start\"><Id>S1</Id><MapPosition x=\"-100\" y=\"0\" unit=\"mm\"/></Control>\n",
    );
    for i in 0..200 {
        let (x, y) = (-95 + (i % 20) * 10, -70 + (i / 20) * 15);
        xml += &format!(
            "<Control type=\"Control\"><Id>{}</Id><MapPosition x=\"{x}\" y=\"{y}\" unit=\"mm\"/></Control>\n",
            100 + i
        );
    }
    xml += "<Control type=\"Finish\"><Id>F1</Id><MapPosition x=\"100\" y=\"0\" unit=\"mm\"/></Control>\n";
    for c in 0..1000_u64 {
        xml += &format!(
            "<Course><Name>K{}</Name><CourseControl type=\"Start\"><Control>S1</Control></CourseControl>",
            c + 1
        );
        let mut order: Vec<usize> = (0..200).collect();
        let mut seed = c * 7919 + 1;
        for i in (1..200).rev() {
            seed = (seed * 1_103_515_245 + 12_345) % (1 << 31);
            order.swap(i, seed as usize % (i + 1));
        }
        for k in order {
            xml += &format!(
                "<CourseControl type=\"Control\"><Control>{}</Control></CourseControl>",
                100 + k
            );
        }
        xml += "<CourseControl type=\"Finish\"><Control>F1</Control></CourseControl></Course>\n";
    }
    xml += "</RaceCourseData></CourseData>\n";
    std::fs::write(dir.path().join("hostile.xml"), xml).unwrap();
    std::fs::write(
        dir.path().join("freemapper-print.toml"),
        "format = 1\n\n[event]\ntitle = \"Hostile\"\nlang = \"en\"\n\n[sources]\n\
         map = \"luebeck.omap\"\ncourses = \"hostile.xml\"\n\n[map]\nkind = \"sprint\"\n\n\
         [[layouts]]\nid = \"a3\"\npaper = \"A3\"\nmap_frame = { crop = true }\n",
    )
    .unwrap();
    dir
}

#[test]
fn closing_stdin_during_a_long_export_ends_the_server_after_the_sheet_in_hand() {
    let dir = hostile_project();
    let out = tempfile::tempdir().unwrap();
    let mut server = Server::start(&[]);
    server.call_export(2, json!({ "project": dir.path(), "out_dir": out.path() }));
    let deadline = Instant::now() + Duration::from_secs(300);
    while !names_in(out.path()).iter().any(|n| n.ends_with(".pdf")) {
        assert!(Instant::now() < deadline, "no PDF after 300 s");
        std::thread::sleep(Duration::from_millis(20));
    }
    // The client goes away in the middle of the export.
    drop(server.stdin.take());
    let closed = Instant::now();
    let status = loop {
        if let Some(status) = server.child.try_wait().unwrap() {
            break status;
        }
        assert!(
            closed.elapsed() < Duration::from_secs(60),
            "the server still runs 60 s after EOF"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    eprintln!("the server ended {:?} after EOF", closed.elapsed());
    assert!(status.success(), "{status}");
    // The export stopped long before the 1000th course, and every file it left is a whole PDF.
    let names = names_in(out.path());
    assert!((1..1000).contains(&names.len()), "{}", names.len());
    for name in &names {
        assert!(name.ends_with(".pdf"), "{name}");
        let pdf = std::fs::read(out.path().join(name)).unwrap();
        assert!(
            pdf.starts_with(b"%PDF") && pdf.trim_ascii_end().ends_with(b"%%EOF"),
            "{name}"
        );
    }
}

#[test]
fn a_full_disk_comes_back_as_a_tool_error_and_leaves_no_file() {
    let project = tempfile::tempdir().unwrap();
    for name in [
        "freemapper-print.toml",
        "berlin-viktoriapark.omap",
        "freemapper.toml",
        "training.ppen",
    ] {
        std::fs::copy(
            std::path::Path::new(VIKTORIAPARK).join(name),
            project.path().join(name),
        )
        .unwrap();
    }
    let out = tempfile::tempdir().unwrap();
    let mut server = Server::start(&[(freemapper_print::output::FAKE_FREE_SPACE_ENV, "1000")]);
    server.call_export(
        2,
        json!({ "project": project.path(), "out_dir": out.path() }),
    );
    let answer = server.answer(2);
    let result = &answer["result"];
    assert_eq!(result["isError"], true, "{answer}");
    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("not enough disk space") && text.contains("Training.pdf"),
        "{text}"
    );
    assert!(names_in(out.path()).is_empty(), "no PDF, no .part");
}

fn tools_of(server: &mut Server) -> Vec<Value> {
    server.send(json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} }));
    server.answer(2)["result"]["tools"]
        .as_array()
        .unwrap()
        .clone()
}

#[test]
fn tools_list_has_the_two_update_tools() {
    let mut server = Server::start(&[]);
    let tools = tools_of(&mut server);
    let tool = |name: &str| {
        tools
            .iter()
            .find(|t| t["name"] == name)
            .unwrap_or_else(|| panic!("no tool {name}"))
    };
    let check = tool("check_update");
    assert!(
        check["inputSchema"]["properties"]["lang"].is_object(),
        "{check}"
    );
    assert!(
        check["inputSchema"]["properties"]["confirm"].is_null(),
        "{check}"
    );
    let install = tool("install_update");
    assert_eq!(
        install["inputSchema"]["properties"]["confirm"]["type"], "boolean",
        "{install}"
    );
    assert!(
        install["inputSchema"]["properties"]["lang"].is_object(),
        "{install}"
    );
    assert!(
        install["inputSchema"]["required"]
            .as_array()
            .is_none_or(|r| r.is_empty()),
        "confirm is optional: {install}"
    );
    // The tools reject fields they do not know.
    let (error, texts) = server.call_all("check_update", json!({ "confirm": true }));
    assert!(error, "{texts:?}");
}

/// A source build says how to update, in both tools, as `{"message"}` and not as an error.
#[test]
fn mcp_update_tools_in_a_source_build_say_how_to_update() {
    let mut server = Server::start(&[]);
    for (tool, arguments) in [
        ("check_update", json!({})),
        ("install_update", json!({})),
        ("install_update", json!({ "confirm": true })),
    ] {
        let (error, texts) = server.call_all(tool, arguments);
        assert!(!error, "{tool}: {texts:?}");
        assert_eq!(texts.len(), 1, "{texts:?}");
        let result: Value = serde_json::from_str(&texts[0]).unwrap();
        assert_eq!(result.as_object().unwrap().len(), 1, "{result}");
        let message = result["message"].as_str().unwrap();
        assert!(message.contains("built from source"), "{message}");
        assert!(
            message.contains("https://github.com/HenryJobst/freemapper-print/releases/latest")
                && message.contains("git pull and cargo build --release --locked"),
            "{message}"
        );
    }
    let (error, texts) = server.call_all("check_update", json!({ "lang": "de" }));
    assert!(!error, "{texts:?}");
    assert!(texts[0].contains("aus dem Quellcode gebaut"), "{texts:?}");
}

#[cfg(all(feature = "test-stub", debug_assertions))]
mod update {
    use std::path::Path;
    use std::process::Command;
    use std::time::{Duration, Instant};

    use freemapper_update::{CheckRecord, RecordResult, load_record, save_record};
    use serde_json::{Value, json};

    use super::support::{MANIFEST_PATH, UpdateStub, copied_program};
    use super::{Server, VIKTORIAPARK};

    const NOTES: &str = "https://github.com/HenryJobst/freemapper-print/releases/tag/v99.0.0";

    /// `program` as a server talking to `stub`, with settings, record and home inside `dir`, so no test touches
    /// a shared folder or the home of the person running it.
    fn checking(program: &Path, stub: &UpdateStub, dir: &Path) -> Command {
        let mut command = Command::new(program);
        command
            .env_remove("FREEMAPPER_PRINT_UPDATE")
            .env("HOME", dir.join("home"))
            .env("USERPROFILE", dir.join("home"))
            .env("FREEMAPPER_PRINT_CONFIG_DIR", dir.join("config"))
            .env("FREEMAPPER_PRINT_CACHE_DIR", dir.join("cache"))
            .env("FREEMAPPER_PRINT_UPDATE_TEST_BASE", &stub.base);
        command
    }

    fn program() -> &'static Path {
        Path::new(env!("CARGO_BIN_EXE_freemapper-print"))
    }

    fn note() -> String {
        format!(
            "Note for the person: freemapper-print 99.0.0 is available (this is {}). Ask them before calling install_update.",
            env!("CARGO_PKG_VERSION")
        )
    }

    fn wait_for_record(cache: &Path) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while load_record(&cache.join("update-check.json")).is_none() {
            assert!(Instant::now() < deadline, "no check record");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// A record of a check made now that found release 99.0.0: a server started with it knows the note at once.
    fn record_99(cache: &Path) {
        std::fs::create_dir_all(cache).unwrap();
        let record = CheckRecord {
            at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            by: env!("CARGO_PKG_VERSION").parse().unwrap(),
            result: RecordResult::Available {
                version: "99.0.0".parse().unwrap(),
                notes: NOTES.into(),
            },
        };
        save_record(&cache.join("update-check.json"), &record).unwrap();
    }

    /// A tool that answers without the network.
    fn cheap_call(server: &mut Server) -> (bool, Vec<String>) {
        server.call_all("project_info", json!({ "project": VIKTORIAPARK }))
    }

    fn json_of(text: &str) -> Value {
        serde_json::from_str(text).unwrap_or_else(|e| panic!("not JSON ({e}): {text}"))
    }

    #[test]
    fn check_update_reports_available_with_stub() {
        let dir = tempfile::tempdir().unwrap();
        let stub = UpdateStub::with_fixture();
        let mut command = checking(program(), &stub, dir.path());
        // Mode `off`: the server does not check by itself, so only the tool asks.
        command.env("FREEMAPPER_PRINT_UPDATE", "off");
        let mut server = Server::start_command(command);
        let (error, texts) = server.call_all("check_update", json!({}));
        assert!(!error, "{texts:?}");
        assert_eq!(texts.len(), 1, "{texts:?}");
        let result = json_of(&texts[0]);
        let current = env!("CARGO_PKG_VERSION");
        assert_eq!(result["current"], current);
        let available = &result["available"];
        assert_eq!(available["version"], "99.0.0", "{result}");
        assert_eq!(available["released"], "2026-11-20", "{result}");
        assert_eq!(available["notes"], NOTES, "{result}");
        assert!(available["size_bytes"].as_u64().unwrap() > 0, "{result}");
        assert_eq!(available["manual"], false, "{result}");
        let message = result["message"].as_str().unwrap();
        assert!(
            message.contains(&format!(
                "freemapper-print 99.0.0 is available (you have {current})"
            )),
            "{message}"
        );
        assert_eq!(
            stub.paths(),
            [
                MANIFEST_PATH.to_string(),
                format!("{MANIFEST_PATH}.minisig")
            ]
        );
        // The tool checks; it neither records nor installs.
        assert!(!dir.path().join("cache/update-check.json").exists());
        assert!(!stub.asked_for_an_asset());
    }

    #[test]
    fn check_update_answers_in_the_language_of_the_call() {
        let dir = tempfile::tempdir().unwrap();
        let stub = UpdateStub::with_fixture();
        let mut command = checking(program(), &stub, dir.path());
        command.env("FREEMAPPER_PRINT_UPDATE", "off");
        let mut server = Server::start_command(command);
        let (_, texts) = server.call_all("check_update", json!({ "lang": "de" }));
        let result = json_of(&texts[0]);
        assert!(
            result["message"]
                .as_str()
                .unwrap()
                .contains("ist verfügbar"),
            "{result}"
        );
    }

    #[test]
    fn install_update_needs_confirm_true() {
        let dir = tempfile::tempdir().unwrap();
        let stub = UpdateStub::with_fixture();
        let mut command = checking(program(), &stub, dir.path());
        command.env("FREEMAPPER_PRINT_UPDATE", "off");
        let mut server = Server::start_command(command);
        for arguments in [json!({}), json!({ "confirm": false })] {
            let (error, texts) = server.call_all("install_update", arguments.clone());
            assert!(error, "{arguments}: {texts:?}");
            assert_eq!(
                texts,
                [
                    "Nothing was installed. Ask the person first, then call install_update with confirm: true once they agree."
                ],
                "{arguments}"
            );
        }
        assert!(stub.paths().is_empty(), "{:?}", stub.paths());
    }

    #[test]
    fn first_tool_result_carries_the_note_once() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        let stub = UpdateStub::with_fixture();
        let mut server = Server::start_command(checking(program(), &stub, dir.path()));
        wait_for_record(&cache);
        // The tool's own result first, then the note, and only on the first result that follows the news.
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            let (error, texts) = cheap_call(&mut server);
            assert!(!error, "{texts:?}");
            assert!(texts[0].contains("\"title\""), "{texts:?}");
            if texts.len() == 2 {
                assert_eq!(texts[1], note());
                break;
            }
            assert!(Instant::now() < deadline, "no note");
            std::thread::sleep(Duration::from_millis(20));
        }
        for _ in 0..2 {
            let (_, texts) = cheap_call(&mut server);
            assert_eq!(texts.len(), 1, "{texts:?}");
        }
        let (stderr, rest) = server.close();
        assert_eq!((stderr.as_str(), rest.len()), ("", 0), "{rest:?}");
        assert_eq!(stub.paths().len(), 2);

        // A server started later the same day does not ask again; the record tells it, at once.
        let mut server = Server::start_command(checking(program(), &stub, dir.path()));
        let (_, texts) = cheap_call(&mut server);
        assert_eq!(texts.get(1), Some(&note()), "{texts:?}");
        let (_, texts) = cheap_call(&mut server);
        assert_eq!(texts.len(), 1, "{texts:?}");
        assert_eq!(stub.paths().len(), 2);

        // check_update gives the news itself; no note follows it.
        let mut server = Server::start_command(checking(program(), &stub, dir.path()));
        let (error, texts) = server.call_all("check_update", json!({}));
        assert!(!error, "{texts:?}");
        assert_eq!(texts.len(), 1, "{texts:?}");
        let (_, texts) = cheap_call(&mut server);
        assert_eq!(texts.len(), 1, "{texts:?}");
    }

    /// An update tool that fails gives no news, so the note stays for the next result.
    #[test]
    fn a_failed_update_tool_keeps_the_note() {
        let dir = tempfile::tempdir().unwrap();
        record_99(&dir.path().join("cache"));
        let stub = UpdateStub::with_fixture();
        let mut server = Server::start_command(checking(program(), &stub, dir.path()));
        let (error, texts) = server.call_all("install_update", json!({}));
        assert!(error, "{texts:?}");
        assert_eq!(texts.len(), 1, "{texts:?}");
        let (_, texts) = cheap_call(&mut server);
        assert_eq!(texts.get(1), Some(&note()), "{texts:?}");
        assert!(stub.paths().is_empty(), "{:?}", stub.paths());
    }

    /// The server asks and notes, but never installs, also in mode `install`.
    #[test]
    fn mcp_never_installs_in_install_mode() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        let folder = dir.path().join("bin");
        std::fs::create_dir(&folder).unwrap();
        let copy = copied_program(&folder);
        let stub = UpdateStub::with_fixture();
        let mut command = checking(&copy, &stub, dir.path());
        command.env("FREEMAPPER_PRINT_UPDATE", "install");
        let mut server = Server::start_command(command);
        wait_for_record(&cache);
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            let (_, texts) = cheap_call(&mut server);
            if texts.len() == 2 {
                assert_eq!(texts[1], note());
                break;
            }
            assert!(Instant::now() < deadline, "no note");
            std::thread::sleep(Duration::from_millis(20));
        }
        let (stderr, rest) = server.close();
        assert_eq!((stderr.as_str(), rest.len()), ("", 0), "{rest:?}");
        assert!(!stub.asked_for_an_asset(), "{:?}", stub.paths());
        assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 1);
    }

    /// With an update known the server's stdout is still only JSON-RPC, and nothing goes to stderr.
    #[test]
    fn stdout_stays_json_rpc_with_an_update_known() {
        let dir = tempfile::tempdir().unwrap();
        record_99(&dir.path().join("cache"));
        let stub = UpdateStub::with_fixture();
        let mut command = checking(program(), &stub, dir.path());
        command.env("FREEMAPPER_PRINT_UPDATE", "install");
        let mut server = Server::start_command(command);
        let (_, texts) = cheap_call(&mut server);
        assert_eq!(texts.get(1), Some(&note()), "{texts:?}");
        let (stderr, rest) = server.close();
        assert_eq!(stderr, "");
        for line in rest {
            let message = json_of(&line);
            assert_eq!(message["jsonrpc"], "2.0", "{line}");
        }
        assert!(stub.paths().is_empty(), "{:?}", stub.paths());
    }

    /// A release that the self-update cannot install gets a note that says so, with the link.
    #[test]
    fn a_release_that_needs_a_manual_install_gets_its_own_note() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/update");
        let stub = UpdateStub::serving(
            std::fs::read(fixtures.join("manifest-99-manual.json")).unwrap(),
            std::fs::read(fixtures.join("manifest-99-manual.json.minisig")).unwrap(),
            Vec::new(),
        );
        let mut server = Server::start_command(checking(program(), &stub, dir.path()));
        wait_for_record(&cache);
        let deadline = Instant::now() + Duration::from_secs(20);
        let note = loop {
            let (_, texts) = cheap_call(&mut server);
            if let Some(note) = texts.get(1) {
                break note.clone();
            }
            assert!(Instant::now() < deadline, "no note");
            std::thread::sleep(Duration::from_millis(20));
        };
        assert!(
            note.starts_with("Note for the person: freemapper-print 99.0.0 is available")
                && note.contains("install_update cannot install it"),
            "{note}"
        );
        let (_, texts) = server.call_all("check_update", json!({}));
        let result = json_of(&texts[0]);
        assert_eq!(result["available"]["manual"], true, "{result}");
        assert!(result["available"]["released"].is_null(), "{result}");
        assert!(result["available"]["size_bytes"].is_null(), "{result}");
    }

    /// The whole install over a copy of the program, through the tool.
    #[test]
    #[cfg(unix)]
    fn install_update_with_confirm_installs_and_asks_for_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("bin");
        let work = dir.path().join("work");
        std::fs::create_dir(&folder).unwrap();
        std::fs::create_dir(&work).unwrap();
        let program = copied_program(&folder);
        let Some(stub) = super::support::installable_release(&work) else {
            eprintln!(
                "SKIPPED install_update_with_confirm_installs_and_asks_for_a_restart: no minisign"
            );
            return;
        };
        let mut command = checking(&program, &stub, dir.path());
        command.env("FREEMAPPER_PRINT_UPDATE", "off");
        let mut server = Server::start_command(command);
        let (error, texts) = server.call_all("install_update", json!({ "confirm": true }));
        assert!(!error, "{texts:?}");
        assert_eq!(texts.len(), 1, "{texts:?}");
        let result = json_of(&texts[0]);
        let current = env!("CARGO_PKG_VERSION");
        assert_eq!(result["from"], current, "{result}");
        assert_eq!(result["installed"], "99.0.0", "{result}");
        assert_eq!(result["notes"], NOTES, "{result}");
        let message = result["message"].as_str().unwrap();
        assert!(
            message.contains(&format!(
                "freemapper-print updated from {current} to 99.0.0"
            )),
            "{message}"
        );
        assert!(
            message.ends_with(
                "Restart the freemapper-print MCP server now; it still runs the old version."
            ),
            "{message}"
        );
        let output = Command::new(&program).arg("--version").output().unwrap();
        assert_eq!(
            String::from_utf8(output.stdout).unwrap().trim(),
            "freemapper-print 99.0.0"
        );
    }
}
