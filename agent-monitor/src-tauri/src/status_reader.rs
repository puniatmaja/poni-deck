use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const STATUS_TTL: Duration = Duration::from_secs(30);
const VALID_STATUSES: [&str; 4] = ["working", "idle", "waiting_confirmation", "error"];
const VALID_LAUNCHERS: [&str; 2] = ["vscode", "terminal"];

#[derive(Deserialize)]
struct StatusFile {
    #[serde(default)]
    status: String,
    #[serde(default)]
    launcher: Option<String>,
    #[serde(default)]
    cwd: Option<String>,
}

fn status_path(pid: u32) -> PathBuf {
    crate::config::agents_dir().join(format!("{}.json", pid))
}

fn is_fresh(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    let Ok(modified) = meta.modified() else {
        return false;
    };
    match SystemTime::now().duration_since(modified) {
        Ok(age) => age <= STATUS_TTL,
        Err(_) => false,
    }
}

fn read_fresh(pid: u32) -> Option<StatusFile> {
    let path = status_path(pid);
    if !is_fresh(&path) {
        return None;
    }
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

#[allow(dead_code)]
pub fn read(pid: u32) -> Option<String> {
    let file = read_fresh(pid)?;
    if VALID_STATUSES.contains(&file.status.as_str()) {
        Some(file.status)
    } else {
        None
    }
}

pub fn read_state_and_launcher(pid: u32) -> Option<(String, Option<String>)> {
    let file = read_fresh(pid)?;
    if !VALID_STATUSES.contains(&file.status.as_str()) {
        return None;
    }
    let launcher = file
        .launcher
        .filter(|l| VALID_LAUNCHERS.contains(&l.as_str()));
    Some((file.status, launcher))
}

pub fn file_mtime(pid: u32) -> Option<SystemTime> {
    std::fs::metadata(status_path(pid))
        .and_then(|m| m.modified())
        .ok()
}

pub fn read_cwd(pid: u32) -> Option<String> {
    let file = read_fresh(pid)?;
    file.cwd.filter(|c| !c.is_empty())
}

pub fn remove_file(pid: u32) {
    let _ = std::fs::remove_file(status_path(pid));
}
