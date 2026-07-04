use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde_json::Value;

pub fn init(args: &[String]) -> ExitCode {
    let Some(path) = target_file(args) else {
        eprintln!("tk init: config não encontrada; use --file <config.json>");
        return ExitCode::from(1);
    };
    match wrap_config(&path, &tk_exe()) {
        Ok(n) => {
            println!(
                "tk init: {n} servidor(es) envelopado(s) em {}",
                path.display()
            );
            println!("backup: {}", backup_path(&path).display());
            ExitCode::SUCCESS
        }
        Err(msg) => {
            eprintln!("tk init: {msg}");
            ExitCode::from(1)
        }
    }
}

pub fn restore(args: &[String]) -> ExitCode {
    let Some(path) = target_file(args) else {
        eprintln!("tk restore: config não encontrada; use --file <config.json>");
        return ExitCode::from(1);
    };
    match restore_config(&path) {
        Ok(()) => {
            println!("tk restore: {} restaurado", path.display());
            ExitCode::SUCCESS
        }
        Err(msg) => {
            eprintln!("tk restore: {msg}");
            ExitCode::from(1)
        }
    }
}

/// --file <path> ou a config do Claude Desktop do SO.
fn target_file(args: &[String]) -> Option<PathBuf> {
    if let Some(i) = args.iter().position(|a| a == "--file") {
        return args.get(i + 1).map(PathBuf::from);
    }
    // Windows: %APPDATA%\Claude; macOS: ~/Library/Application Support/Claude;
    // Linux: ~/.config/Claude
    let path = dirs::config_dir()?
        .join("Claude")
        .join("claude_desktop_config.json");
    path.exists().then_some(path)
}

fn tk_exe() -> String {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "tk".to_string())
}

fn backup_path(path: &Path) -> PathBuf {
    let mut p = path.as_os_str().to_owned();
    p.push(".tk-backup");
    PathBuf::from(p)
}

/// Reescreve cada servidor em mcpServers para rodar via tk.
/// Devolve quantos foram envelopados. Cria backup antes de gravar.
fn wrap_config(path: &Path, tk_exe: &str) -> Result<usize, String> {
    let backup = backup_path(path);
    if backup.exists() {
        return Err(format!(
            "backup já existe ({}); rode `tk restore` antes de um novo init",
            backup.display()
        ));
    }
    let content = std::fs::read_to_string(path).map_err(|e| format!("lendo config: {e}"))?;
    let mut root: Value =
        serde_json::from_str(&content).map_err(|e| format!("JSON inválido: {e}"))?;
    let servers = root
        .get_mut("mcpServers")
        .and_then(|s| s.as_object_mut())
        .ok_or("config não tem mcpServers")?;

    let mut wrapped = 0;
    for (name, server) in servers.iter_mut() {
        let Some(obj) = server.as_object_mut() else {
            continue;
        };
        let Some(command) = obj
            .get("command")
            .and_then(|c| c.as_str())
            .map(String::from)
        else {
            continue; // servidores remotos (url) ficam fora do escopo da v1
        };
        if is_tk(&command) {
            continue; // já envelopado
        }
        let old_args: Vec<Value> = obj
            .get("args")
            .and_then(|a| a.as_array())
            .cloned()
            .unwrap_or_default();
        let mut new_args = vec![
            Value::from("--name"),
            Value::from(name.as_str()),
            Value::from("--"),
            Value::from(command),
        ];
        new_args.extend(old_args);
        obj.insert("command".into(), Value::from(tk_exe));
        obj.insert("args".into(), Value::Array(new_args));
        wrapped += 1;
    }
    if wrapped == 0 {
        return Err("nenhum servidor para envelopar".to_string());
    }
    std::fs::copy(path, &backup).map_err(|e| format!("criando backup: {e}"))?;
    let out = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?;
    std::fs::write(path, out).map_err(|e| format!("gravando config: {e}"))?;
    Ok(wrapped)
}

fn restore_config(path: &Path) -> Result<(), String> {
    let backup = backup_path(path);
    if !backup.exists() {
        return Err(format!("backup não existe: {}", backup.display()));
    }
    std::fs::copy(&backup, path).map_err(|e| format!("restaurando: {e}"))?;
    std::fs::remove_file(&backup).map_err(|e| format!("removendo backup: {e}"))?;
    Ok(())
}

/// O comando já é o tk? (basename sem extensão == "tk")
fn is_tk(command: &str) -> bool {
    Path::new(command)
        .file_stem()
        .map(|s| s.eq_ignore_ascii_case("tk"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG: &str = r#"{
  "mcpServers": {
    "notion": {"command": "npx", "args": ["-y", "@notionhq/notion-mcp-server"]},
    "local": {"command": "C:\\tools\\server.exe"}
  }
}"#;

    fn write_config(dir: &Path) -> PathBuf {
        let path = dir.join("claude_desktop_config.json");
        std::fs::write(&path, CONFIG).unwrap();
        path
    }

    #[test]
    fn wraps_servers_and_creates_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_config(dir.path());
        let wrapped = wrap_config(&path, "C:\\bin\\tk.exe").unwrap();
        assert_eq!(wrapped, 2);

        let root: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let notion = &root["mcpServers"]["notion"];
        assert_eq!(notion["command"], "C:\\bin\\tk.exe");
        let args: Vec<&str> = notion["args"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(
            args,
            [
                "--name",
                "notion",
                "--",
                "npx",
                "-y",
                "@notionhq/notion-mcp-server"
            ]
        );

        assert_eq!(std::fs::read_to_string(backup_path(&path)).unwrap(), CONFIG);
    }

    #[test]
    fn refuses_second_init_while_backup_exists() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_config(dir.path());
        wrap_config(&path, "tk").unwrap();
        let err = wrap_config(&path, "tk").unwrap_err();
        assert!(err.contains("backup"));
    }

    #[test]
    fn skips_already_wrapped_and_errors_when_nothing_to_do() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.json");
        std::fs::write(&path, r#"{"mcpServers":{"x":{"command":"tk","args":[]}}}"#).unwrap();
        assert!(wrap_config(&path, "tk").is_err());
    }

    #[test]
    fn errors_without_mcp_servers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.json");
        std::fs::write(&path, "{}").unwrap();
        assert!(wrap_config(&path, "tk").unwrap_err().contains("mcpServers"));
    }

    #[test]
    fn restore_brings_original_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_config(dir.path());
        wrap_config(&path, "tk").unwrap();
        restore_config(&path).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), CONFIG);
        assert!(!backup_path(&path).exists());
    }
}
