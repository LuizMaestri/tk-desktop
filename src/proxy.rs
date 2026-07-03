use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use crate::events;
use crate::frames;
use crate::tracker::Tracker;

/// Roda o servidor MCP `cmd` como filho, repassando stdio byte a byte
/// e medindo requisições/respostas. Devolve o exit code do filho.
pub fn run(server_name: &str, cmd: &[String]) -> i32 {
    let mut child = match Command::new(&cmd[0])
        .args(&cmd[1..])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("tk: falha ao iniciar {}: {e}", cmd[0]);
            return 1;
        }
    };

    let tracker = Arc::new(Mutex::new(Tracker::new(server_name.to_string())));

    let mut child_stdin = child.stdin.take().expect("stdin do filho");
    let child_stdout = child.stdout.take().expect("stdout do filho");

    // stdin nosso -> stdin do filho (requisições do cliente MCP).
    // Não fazemos join desta thread: se o filho morrer antes de o cliente
    // fechar o stdin, ela fica bloqueada em leitura e o exit do processo
    // a encerra junto.
    let req_tracker = Arc::clone(&tracker);
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        pump(stdin.lock(), &mut child_stdin, |line| {
            if let Ok(mut t) = req_tracker.lock() {
                t.observe(frames::inspect(line));
            }
        });
        // child_stdin é dropado aqui: EOF sinaliza o filho a encerrar.
    });

    // stdout do filho -> stdout nosso (respostas do servidor).
    let mut stdout = std::io::stdout().lock();
    pump(BufReader::new(child_stdout), &mut stdout, |line| {
        if let Ok(mut t) = tracker.lock() {
            if let Some(event) = t.observe(frames::inspect(line)) {
                events::append(&event);
            }
        }
    });

    match child.wait() {
        Ok(status) => status.code().unwrap_or(1),
        Err(_) => 1,
    }
}

/// Copia linhas de `from` para `to` sem modificar bytes, chamando
/// `observe` com cada linha completa. Termina em EOF ou erro de escrita.
fn pump<R: BufRead, W: Write>(mut from: R, to: &mut W, mut observe: impl FnMut(&[u8])) {
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match from.read_until(b'\n', &mut buf) {
            Ok(0) => break,
            Ok(_) => {
                observe(&buf);
                if to.write_all(&buf).is_err() {
                    break;
                }
                let _ = to.flush();
            }
            Err(_) => break,
        }
    }
}
