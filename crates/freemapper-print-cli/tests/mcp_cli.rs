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
    assert_eq!(answer(2)["result"]["tools"].as_array().unwrap().len(), 3);
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
