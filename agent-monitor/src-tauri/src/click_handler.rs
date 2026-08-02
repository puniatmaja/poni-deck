use anyhow::Result;
use std::path::PathBuf;
use std::process::Command;

pub fn open_terminal(path: &str) -> Result<()> {
    Command::new("cmd")
        .args(["/C", "start", "cmd", "/K", "cd", "/d", path])
        .spawn()?;
    Ok(())
}

fn find_vscode() -> Option<PathBuf> {
    let path_env = std::env::var_os("PATH")?;
    let dirs: Vec<PathBuf> = std::env::split_paths(&path_env).collect();

    for dir in &dirs {
        for name in ["code.exe", "Code.exe"] {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    for dir in &dirs {
        for name in ["code.cmd", "Code.cmd"] {
            let candidate = dir.join(name);
            if candidate.is_file() {
                if let Some(install) = candidate.parent().and_then(|bin| bin.parent()) {
                    let exe = install.join("Code.exe");
                    if exe.is_file() {
                        return Some(exe);
                    }
                }
            }
        }
    }

    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let candidate = PathBuf::from(local).join(r"Programs\Microsoft VS Code\Code.exe");
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    for known in [
        r"C:\Program Files\Microsoft VS Code\Code.exe",
        r"C:\Program Files (x86)\Microsoft VS Code\Code.exe",
    ] {
        let candidate = PathBuf::from(known);
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}

fn code_cli_on_path() -> bool {
    let Some(path_env) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path_env).any(|dir| {
        ["code.cmd", "Code.cmd", "code.exe", "Code.exe"]
            .iter()
            .any(|name| dir.join(name).is_file())
    })
}

pub fn open_vscode(path: &str) -> Result<()> {
    if let Some(exe) = find_vscode() {
        if Command::new(exe).arg(path).spawn().is_ok() {
            return Ok(());
        }
    }

    if code_cli_on_path() {
        let spawned = Command::new("cmd")
            .args(["/C", "start", "", "code", path])
            .spawn();
        if spawned.is_ok() {
            return Ok(());
        }
    }

    open_terminal(path).ok();
    Err(anyhow::anyhow!("VS Code not found, opened terminal instead"))
}

pub fn open_focus_or_new(path: &str, launcher: &str, pid: u32) -> Result<()> {
    if crate::window_focus::focus_agent_window(pid) {
        return Ok(());
    }

    match launcher {
        "vscode" => open_vscode(path),
        _ => open_terminal(path),
    }
}
