use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

fn tk() -> &'static str {
    env!("CARGO_BIN_EXE_tk")
}

fn fake() -> &'static str {
    env!("CARGO_BIN_EXE_fake-mcp-server")
}

#[test]
fn proxy_passes_bytes_and_records_events() {
    let logs = tempfile::tempdir().unwrap();
    let mut child = Command::new(tk())
        .args(["--name", "fake", "--", fake()])
        .env("TK_LOG_DIR", logs.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    let requests = [
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"echo","arguments":{"msg":"oi"}}}"#,
    ];
    let mut responses = Vec::new();
    for req in requests {
        writeln!(stdin, "{req}").unwrap();
        let mut line = String::new();
        stdout.read_line(&mut line).unwrap();
        responses.push(line);
    }
    drop(stdin);
    let status = child.wait().unwrap();
    assert!(status.success());

    // Respostas chegam intactas (passthrough) com os ids corretos.
    let r0: serde_json::Value = serde_json::from_str(&responses[0]).unwrap();
    assert_eq!(r0["id"], 1);
    let r1: serde_json::Value = serde_json::from_str(&responses[1]).unwrap();
    assert_eq!(r1["id"], 2);
    assert!(responses[1].contains("xxxx"));

    // Eventos gravados: initialize + echo, com resp_tokens >= 100 (payload de 400 chars).
    let log_file = std::fs::read_dir(logs.path())
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let content = std::fs::read_to_string(log_file).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 2);
    let e0: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(e0["server"], "fake");
    assert_eq!(e0["tool"], "initialize");
    let e1: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(e1["tool"], "echo");
    assert!(e1["resp_tokens"].as_u64().unwrap() >= 100);
}

#[test]
fn proxy_propagates_child_exit_code() {
    let logs = tempfile::tempdir().unwrap();
    let mut child = Command::new(tk())
        .args(["--", fake()])
        .env("TK_LOG_DIR", logs.path())
        .env("FAKE_EXIT_CODE", "7")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdin.take());
    let status = child.wait().unwrap();
    assert_eq!(status.code(), Some(7));
}
