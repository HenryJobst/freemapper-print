//! `freemapper-print mcp` spoken to over stdio, as an agent would: stdout carries JSON-RPC and nothing else.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

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
    assert_eq!(answer(2)["result"]["tools"].as_array().unwrap().len(), 11);
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
}

impl Server {
    fn start(env: &[(&str, &str)]) -> Server {
        let mut command = Command::new(env!("CARGO_BIN_EXE_freemapper-print"));
        command
            .args(["mcp", "--lang", "en"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        for (key, value) in env {
            command.env(key, value);
        }
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
