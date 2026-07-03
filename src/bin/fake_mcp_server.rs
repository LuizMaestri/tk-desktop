use std::io::{BufRead, Write};

// Servidor MCP mínimo para testes de integração: responde initialize,
// tools/list e tools/call com payloads fixos; sai no EOF do stdin.
// FAKE_EXIT_CODE define o exit code final (padrão 0).
fn main() {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let value: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let id = value.get("id").cloned().unwrap_or(serde_json::Value::Null);
        if id.is_null() {
            continue; // notificação: sem resposta
        }
        let method = value.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let result = match method {
            "initialize" => serde_json::json!({"capabilities": {}, "serverInfo": {"name": "fake"}}),
            "tools/list" => serde_json::json!({"tools": [{"name": "echo"}]}),
            "tools/call" => serde_json::json!({"content": [{"type": "text", "text": "x".repeat(400)}]}),
            _ => serde_json::json!({}),
        };
        let resp = serde_json::json!({"jsonrpc": "2.0", "id": id, "result": result});
        writeln!(stdout, "{resp}").unwrap();
        stdout.flush().unwrap();
    }
    let code = std::env::var("FAKE_EXIT_CODE").ok().and_then(|c| c.parse().ok()).unwrap_or(0);
    std::process::exit(code);
}
