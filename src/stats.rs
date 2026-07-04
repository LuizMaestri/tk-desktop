use std::collections::BTreeMap;
use std::path::Path;
use std::process::ExitCode;

use chrono::{Duration, Utc};

use crate::events::log_dir;
use crate::tracker::Event;

pub fn run(args: &[String]) -> ExitCode {
    let since = match parse_since(args) {
        Ok(s) => s,
        Err(msg) => {
            eprintln!("tk stats: {msg}");
            return ExitCode::from(2);
        }
    };
    let Some(dir) = log_dir() else {
        eprintln!("tk stats: diretório de logs indisponível");
        return ExitCode::from(1);
    };
    let events = read_events(&dir, since);
    print!("{}", report(&events));
    ExitCode::SUCCESS
}

/// Extrai --since dos args; padrão 7d. Formatos: <N>d ou <N>h.
fn parse_since(args: &[String]) -> Result<Duration, String> {
    let spec = match args.iter().position(|a| a == "--since") {
        None => return Ok(Duration::days(7)),
        Some(i) => args
            .get(i + 1)
            .ok_or("--since exige um valor (ex.: 7d, 24h)")?,
    };
    if !spec.is_ascii() || spec.len() < 2 {
        return Err(format!("período inválido: {spec} (use ex.: 7d, 24h)"));
    }
    let (num, unit) = spec.split_at(spec.len() - 1);
    let n: i64 = num
        .parse()
        .map_err(|_| format!("período inválido: {spec}"))?;
    if n < 0 {
        return Err(format!("período inválido: {spec} (use ex.: 7d, 24h)"));
    }
    match unit {
        "d" => Ok(Duration::days(n)),
        "h" => Ok(Duration::hours(n)),
        _ => Err(format!("período inválido: {spec} (use ex.: 7d, 24h)")),
    }
}

fn read_events(dir: &Path, since: Duration) -> Vec<Event> {
    let cutoff = Utc::now() - since;
    let mut events = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return events;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        for line in content.lines() {
            if let Ok(event) = serde_json::from_str::<Event>(line) {
                if event.ts >= cutoff {
                    events.push(event);
                }
            }
        }
    }
    events
}

/// Relatório agregado por servidor+tool, maior consumidor primeiro.
fn report(events: &[Event]) -> String {
    if events.is_empty() {
        return "nenhum evento no período (os valores de tokens são estimados)\n".to_string();
    }
    // (server, tool) -> (chamadas, req, resp)
    let mut by_tool: BTreeMap<(String, String), (u64, u64, u64)> = BTreeMap::new();
    for e in events {
        let slot = by_tool
            .entry((e.server.clone(), e.tool.clone()))
            .or_default();
        slot.0 += 1;
        slot.1 += e.req_tokens;
        slot.2 += e.resp_tokens;
    }
    let mut rows: Vec<_> = by_tool.into_iter().collect();
    rows.sort_by_key(|(_, (_, req, resp))| std::cmp::Reverse(req + resp));

    let mut out = format!(
        "{:<17} {:<25} {:>8} {:>12} {:>13}\n",
        "servidor", "tool", "chamadas", "tokens req", "tokens resp"
    );
    let mut total = 0u64;
    for ((server, tool), (calls, req, resp)) in &rows {
        out.push_str(&format!(
            "{server:<17} {tool:<25} {calls:>8} {req:>12} {resp:>13}\n"
        ));
        total += req + resp;
    }
    out.push_str(&format!(
        "\ntotal estimado: {total} tokens (heurística ~4 chars/token)\n"
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tracker::Event;

    fn ev(server: &str, tool: &str, req: u64, resp: u64, age_hours: i64) -> Event {
        Event {
            ts: Utc::now() - Duration::hours(age_hours),
            server: server.into(),
            tool: tool.into(),
            req_tokens: req,
            resp_tokens: resp,
        }
    }

    #[test]
    fn since_defaults_to_seven_days() {
        assert_eq!(parse_since(&[]).unwrap(), Duration::days(7));
    }

    #[test]
    fn since_parses_days_and_hours() {
        let args = |s: &str| vec!["--since".to_string(), s.to_string()];
        assert_eq!(parse_since(&args("2d")).unwrap(), Duration::days(2));
        assert_eq!(parse_since(&args("24h")).unwrap(), Duration::hours(24));
        assert!(parse_since(&args("banana")).is_err());
        assert!(parse_since(&["--since".to_string()]).is_err());
    }

    #[test]
    fn since_rejects_negative_magnitude() {
        assert!(parse_since(&["--since".into(), "-5d".into()]).is_err());
    }

    #[test]
    fn read_events_filters_by_cutoff() {
        let dir = tempfile::tempdir().unwrap();
        let recent = ev("s", "t", 1, 1, 1);
        let old = ev("s", "t", 1, 1, 100);
        let lines = format!(
            "{}\n{}\n",
            serde_json::to_string(&recent).unwrap(),
            serde_json::to_string(&old).unwrap()
        );
        std::fs::write(dir.path().join("2026-07-03.jsonl"), lines).unwrap();
        let events = read_events(dir.path(), Duration::days(2));
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn report_aggregates_and_sorts_by_total() {
        let events = vec![
            ev("a", "small", 1, 1, 0),
            ev("a", "big", 10, 1000, 0),
            ev("a", "big", 10, 1000, 0),
        ];
        let out = report(&events);
        let big_pos = out.find("big").unwrap();
        let small_pos = out.find("small").unwrap();
        assert!(big_pos < small_pos, "maior ofensor primeiro:\n{out}");
        assert!(out.contains("total estimado: 2022 tokens"));
    }

    #[test]
    fn report_handles_empty() {
        assert!(report(&[]).contains("nenhum evento"));
    }
}
