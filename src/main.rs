mod estimate;
mod events;
mod frames;
mod proxy;
mod stats;
mod tracker;

use std::process::ExitCode;

const USAGE: &str = "\
uso:
  tk [--name <alias>] -- <comando do servidor MCP> [args...]
  tk stats [--since <7d|24h>]
  tk init [--file <config.json>]
  tk restore [--file <config.json>]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("stats") => stats::run(&args[1..]),
        _ => run_proxy(&args),
    }
}

fn run_proxy(args: &[String]) -> ExitCode {
    let mut name: Option<String> = None;
    let mut rest = args;
    if rest.first().map(String::as_str) == Some("--name") {
        if rest.len() < 2 {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
        name = Some(rest[1].clone());
        rest = &rest[2..];
    }
    let cmd: Vec<String> = match rest.first().map(String::as_str) {
        Some("--") => rest[1..].to_vec(),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    if cmd.is_empty() {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    let server = name.unwrap_or_else(|| basename(&cmd[0]));
    let code = proxy::run(&server, &cmd);
    ExitCode::from(code.clamp(0, 255) as u8)
}

fn basename(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}
