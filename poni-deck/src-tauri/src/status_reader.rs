use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const STATUS_TTL: Duration = Duration::from_secs(30);
// Claude Code's hook is event-driven (no periodic heartbeat like the opencode
// plugin), so its status file must stay valid far longer. Dead processes are
// cleaned by PID-scan removal (lib.rs) and orphan cleanup, not by TTL.
const CLAUDE_STATUS_TTL: Duration = Duration::from_secs(1800);
const VALID_STATUSES: [&str; 4] = ["working", "idle", "waiting_confirmation", "error"];
// A permission wait that receives no follow-up hook event (Claude Code does
// not fire PermissionDenied/Stop on a manual deny) would otherwise stay stuck
// forever. After this window without an update we treat it as ended -> idle.
const WAITING_CONFIRM_MAX_AGE: Duration = Duration::from_secs(30);

fn ttl_for_tool(tool: Option<&str>) -> Duration {
    match tool {
        Some("claude") => CLAUDE_STATUS_TTL,
        _ => STATUS_TTL,
    }
}

#[derive(Deserialize)]
pub struct StatusFile {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub launcher: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub tool: Option<String>,
}

fn status_path(pid: u32) -> PathBuf {
    crate::config::agents_dir().join(format!("{}.json", pid))
}

fn is_fresh(path: &Path, ttl: Duration) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    let Ok(modified) = meta.modified() else {
        return false;
    };
    match SystemTime::now().duration_since(modified) {
        Ok(age) => age <= ttl,
        Err(_) => false,
    }
}

fn read_fresh(pid: u32) -> Option<StatusFile> {
    let path = status_path(pid);
    let content = std::fs::read_to_string(&path).ok()?;
    let file: StatusFile = serde_json::from_str(&content).ok()?;
    if !is_fresh(&path, ttl_for_tool(file.tool.as_deref())) {
        return None;
    }
    Some(file)
}

fn file_age(path: &Path) -> Option<Duration> {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?;
    match SystemTime::now().duration_since(modified) {
        Ok(age) => Some(age),
        Err(_) => None,
    }
}

pub fn read_all(pid: u32) -> Option<StatusFile> {
    let mut file = read_fresh(pid)?;
    if !VALID_STATUSES.contains(&file.status.as_str()) {
        return None;
    }
    // A manual deny in Claude Code produces no follow-up hook event, so the
    // file would stay at waiting_confirmation forever. Treat a permission wait
    // older than the window as ended (idle) rather than stuck.
    if file.status == "waiting_confirmation"
        && file.tool.as_deref() == Some("claude")
        && file_age(&status_path(pid)).map_or(false, |age| age > WAITING_CONFIRM_MAX_AGE)
    {
        file.status = "idle".to_string();
    }
    Some(file)
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

pub fn file_mtime(pid: u32) -> Option<SystemTime> {
    std::fs::metadata(status_path(pid))
        .and_then(|m| m.modified())
        .ok()
}

pub fn remove_file(pid: u32) {
    let _ = std::fs::remove_file(status_path(pid));
}
