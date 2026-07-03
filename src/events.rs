use std::fs;
use std::io::Write;
use std::path::PathBuf;

use crate::tracker::Event;

/// Diretório dos logs: TK_LOG_DIR (override p/ testes) ou dir de dados do SO
/// (%LOCALAPPDATA% no Windows, ~/.local/share no Linux, Application Support no macOS).
pub fn log_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("TK_LOG_DIR") {
        return Some(PathBuf::from(dir));
    }
    Some(dirs::data_local_dir()?.join("token-killer").join("logs"))
}

/// Grava um evento no JSONL diário. Falhas são silenciosas por design:
/// o pior caso do tk é "não mediu", nunca "quebrou o servidor".
pub fn append(event: &Event) {
    let _ = try_append(event);
}

fn try_append(event: &Event) -> Option<()> {
    let dir = log_dir()?;
    fs::create_dir_all(&dir).ok()?;
    let path = dir.join(format!("{}.jsonl", event.ts.format("%Y-%m-%d")));
    let mut file = fs::OpenOptions::new().create(true).append(true).open(path).ok()?;
    let line = serde_json::to_string(event).ok()?;
    writeln!(file, "{line}").ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tracker::Event;

    // Único teste que toca TK_LOG_DIR (env é global ao processo;
    // os demais módulos recebem o diretório por parâmetro).
    #[test]
    fn appends_events_as_daily_jsonl() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("TK_LOG_DIR", dir.path());
        let event = Event {
            ts: chrono::Utc::now(),
            server: "srv".into(),
            tool: "echo".into(),
            req_tokens: 10,
            resp_tokens: 100,
        };
        append(&event);
        append(&event);
        std::env::remove_var("TK_LOG_DIR");

        let file = dir.path().join(format!("{}.jsonl", event.ts.format("%Y-%m-%d")));
        let content = std::fs::read_to_string(file).unwrap();
        let lines: Vec<_> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        let parsed: Event = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(parsed.tool, "echo");
        assert_eq!(parsed.resp_tokens, 100);
    }
}
