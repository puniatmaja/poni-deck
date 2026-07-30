use anyhow::Result;
use std::process::Command;

pub fn open_terminal(path: &str) -> Result<()> {
    Command::new("cmd")
        .args(["/C", "start", "cmd", "/K", "cd", "/d", path])
        .spawn()?;
    Ok(())
}

pub fn open_vscode(path: &str) -> Result<()> {
    Command::new("code")
        .arg(path)
        .spawn()
        .map_err(|_| {
            open_terminal(path).ok();
            anyhow::anyhow!("VS Code not found, opened terminal instead")
        })?;
    Ok(())
}

pub fn open_path_with_action(path: &str, action: &str) -> Result<()> {
    match action {
        "code" => open_vscode(path),
        _ => open_terminal(path),
    }
}
