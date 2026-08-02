use crate::state::AgentInfo;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use windows::core::PWSTR;
use windows::Win32::Foundation::*;
use windows::Win32::System::Diagnostics::ToolHelp::*;
use windows::Win32::System::Threading::*;

const MAX_HOPS: usize = 5;
const TARGET_EXES: [&str; 2] = ["opencode.exe", "claude.exe"];

#[derive(Deserialize, Clone)]
struct ClaudeSession {
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    entrypoint: Option<String>,
}

fn detect_launcher_from_parent_chain(
    pid: u32,
    parent_map: &HashMap<u32, (u32, String)>,
) -> String {
    let mut cur = pid;
    for _ in 0..MAX_HOPS {
        let Some(&(parent, ref name)) = parent_map.get(&cur) else {
            break;
        };
        if name.to_lowercase() == "code.exe" {
            return "vscode".to_string();
        }
        if parent == cur {
            break;
        }
        if !parent_map.contains_key(&parent) {
            break;
        }
        cur = parent;
    }
    "terminal".to_string()
}

fn claude_launcher(entrypoint: Option<&str>) -> String {
    match entrypoint {
        Some(ep) if ep.to_lowercase().contains("vscode") => "vscode".to_string(),
        _ => "terminal".to_string(),
    }
}

fn get_process_path(pid: u32) -> Option<String> {
    unsafe {
        let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return None;
        };

        let mut buf = [0u16; 260];
        let mut size = buf.len() as u32;

        let result = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_FORMAT(0),
            PWSTR(buf.as_mut_ptr()),
            &mut size,
        );

        let _ = CloseHandle(handle);

        if result.is_ok() && size > 0 {
            Some(String::from_utf16_lossy(&buf[..size as usize]))
        } else {
            None
        }
    }
}

fn get_command_line(pid: u32) -> Option<String> {
    unsafe {
        let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ, false, pid) else {
            return None;
        };

        let mut ret_len = 0u32;
        let status = NtQueryInformationProcess(
            handle,
            60,
            std::ptr::null_mut(),
            0,
            &mut ret_len,
        );

        if status != 0 && (status as u32) != 0xC0000004 && (status as u32) != 0x80000005 {
            let _ = CloseHandle(handle);
            return None;
        }

        let buf_size = ret_len.max(256) as usize;
        let mut buf: Vec<u8> = vec![0u8; buf_size];

        let status = NtQueryInformationProcess(
            handle,
            60,
            buf.as_mut_ptr() as *mut std::ffi::c_void,
            buf_size as u32,
            &mut ret_len,
        );

        let _ = CloseHandle(handle);

        if status != 0 || ret_len < 8 {
            return None;
        }

        let length = u16::from_ne_bytes([buf[0], buf[1]]) as usize;
        if length == 0 || length > buf_size - 4 {
            return None;
        }

        let buffer_offset = u32::from_ne_bytes([buf[4], buf[5], buf[6], buf[7]]) as usize;
        let start = buffer_offset.min(buf_size.saturating_sub(length));

        let end = start.saturating_add(length).min(buf_size);
        let raw = &buf[start..end];
        if raw.len() < 2 {
            return None;
        }

        let utf16: Vec<u16> = raw
            .chunks_exact(2)
            .map(|c| u16::from_ne_bytes([c[0], c[1]]))
            .collect();

        let s = String::from_utf16_lossy(&utf16);
        Some(s.trim_end_matches('\0').to_string())
    }
}

#[link(name = "ntdll")]
extern "system" {
    fn NtQueryInformationProcess(
        ProcessHandle: HANDLE,
        ProcessInformationClass: u32,
        ProcessInformation: *mut std::ffi::c_void,
        ProcessInformationLength: u32,
        ReturnLength: *mut u32,
    ) -> i32;
}

fn parse_working_directory(cmdline: &str) -> Option<String> {
    let lower = cmdline.to_lowercase();
    let patterns = ["--cwd", "--dir", "-cwd", "-dir", "--work-dir"];

    for pat in &patterns {
        if let Some(idx) = lower.find(pat) {
            let after = &cmdline[idx + pat.len()..];
            let trimmed = after.trim_start_matches(&[' ', '=', ':'][..]);
            let end = trimmed.find(|c: char| c == ' ' || c == '\t' || c == '"')
                .unwrap_or(trimmed.len());
            let path = trimmed[..end].trim_matches('"').to_string();
            if !path.is_empty() {
                return Some(path);
            }
        }
    }
    None
}

fn read_claude_sessions() -> HashMap<u32, ClaudeSession> {
    let mut sessions = HashMap::new();
    let Ok(entries) = std::fs::read_dir(crate::config::claude_sessions_dir()) else {
        return sessions;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(pid) = name.strip_suffix(".json").and_then(|s| s.parse::<u32>().ok()) else {
            continue;
        };
        let Ok(content) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        if let Ok(sess) = serde_json::from_str::<ClaudeSession>(&content) {
            sessions.insert(pid, sess);
        }
    }
    sessions
}

pub fn scan_agents() -> Vec<AgentInfo> {
    let mut agents = Vec::new();
    let mut parent_map: HashMap<u32, (u32, String)> = HashMap::new();

    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return agents;
        };

        let mut entry = PROCESSENTRY32W::default();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let name = String::from_utf16_lossy(
                    &entry.szExeFile[..entry.szExeFile.iter().take_while(|&&c| c != 0).count()]
                );

                parent_map.insert(entry.th32ProcessID, (entry.th32ParentProcessID, name.clone()));

                if TARGET_EXES.contains(&name.to_lowercase().as_str()) {
                    let pid = entry.th32ProcessID;
                    let exe_path = get_process_path(pid).unwrap_or_default();
                    let cmdline = get_command_line(pid);
                    let working_dir = cmdline
                        .as_ref()
                        .and_then(|c| parse_working_directory(c))
                        .or_else(|| {
                            std::path::Path::new(&exe_path)
                                .parent()
                                .map(|p| p.to_string_lossy().to_string())
                        })
                        .unwrap_or_default();

                    let tool = if name.eq_ignore_ascii_case("claude.exe") {
                        "claude"
                    } else {
                        "opencode"
                    };

                    agents.push(AgentInfo {
                        pid,
                        exe_path: exe_path.clone(),
                        working_dir,
                        state: "running".to_string(),
                        launcher: "terminal".to_string(),
                        tool: tool.to_string(),
                    });
                }

                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }

        let _ = CloseHandle(snapshot);
    }

    let claude_sessions = read_claude_sessions();

    let mut known_pids: HashSet<u32> = agents.iter().map(|a| a.pid).collect();

    for (spid, sess) in &claude_sessions {
        if known_pids.contains(spid) {
            continue;
        }
        if !parent_map.contains_key(spid) {
            continue;
        }
        let exe_path = get_process_path(*spid).unwrap_or_default();
        agents.push(AgentInfo {
            pid: *spid,
            exe_path,
            working_dir: sess.cwd.clone().unwrap_or_default(),
            state: "running".to_string(),
            launcher: claude_launcher(sess.entrypoint.as_deref()),
            tool: "claude".to_string(),
        });
        known_pids.insert(*spid);
    }

    for agent in &mut agents {
        if agent.tool == "claude" {
            if let Some(sess) = claude_sessions.get(&agent.pid) {
                if let Some(cwd) = sess.cwd.as_deref().filter(|c| !c.is_empty()) {
                    agent.working_dir = cwd.to_string();
                }
                agent.launcher = claude_launcher(sess.entrypoint.as_deref());
            }
        }

        if let Some(file) = crate::status_reader::read_all(agent.pid) {
            agent.state = file.status.clone();
            let launcher = file
                .launcher
                .filter(|l| l == "vscode" || l == "terminal");
            agent.launcher = launcher
                .unwrap_or_else(|| detect_launcher_from_parent_chain(agent.pid, &parent_map));
            if let Some(cwd) = file.cwd.as_deref().filter(|c| !c.is_empty()) {
                agent.working_dir = cwd.to_string();
            }
            if let Some(tool) = file.tool.as_deref() {
                if tool == "claude" || tool == "opencode" {
                    agent.tool = tool.to_string();
                }
            }
        } else {
            agent.launcher = detect_launcher_from_parent_chain(agent.pid, &parent_map);
        }
    }
    dedup_agents(&mut agents, &claude_sessions);

    agents
}

fn dedup_agents(agents: &mut Vec<AgentInfo>, claude_sessions: &HashMap<u32, ClaudeSession>) {
    let mut remove: HashSet<u32> = HashSet::new();

    // Rule 1: multiple processes of the same tool in the same working_dir
    // (e.g. opencode TUI + server, claude wrapper + session). Keep only the
    // process(es) that are an actual status/session source; drop the rest.
    let mut groups: HashMap<(String, String), Vec<usize>> = HashMap::new();
    for (i, agent) in agents.iter().enumerate() {
        if !agent.working_dir.is_empty() {
            groups
                .entry((agent.tool.clone(), agent.working_dir.clone()))
                .or_default()
                .push(i);
        }
    }
    for members in groups.values() {
        if members.len() <= 1 {
            continue;
        }
        let sources: Vec<bool> = members
            .iter()
            .map(|&i| {
                let agent = &agents[i];
                crate::status_reader::read_all(agent.pid).is_some()
                    || (agent.tool == "claude" && claude_sessions.contains_key(&agent.pid))
            })
            .collect();
        let source_count = sources.iter().filter(|&&s| s).count();
        if source_count >= 1 && source_count < members.len() {
            for (&i, &is_source) in members.iter().zip(sources.iter()) {
                if !is_source {
                    remove.insert(agents[i].pid);
                }
            }
        }
    }

    // Rule 2: drop a claude process that is neither a session nor a status
    // source while another claude session is present — these are short-lived
    // bootstrap/helper processes that otherwise flash a duplicate entry.
    let has_claude_session = agents
        .iter()
        .any(|a| a.tool == "claude" && claude_sessions.contains_key(&a.pid));
    if has_claude_session {
        for agent in agents.iter() {
            if agent.tool == "claude"
                && !claude_sessions.contains_key(&agent.pid)
                && crate::status_reader::file_mtime(agent.pid).is_none()
            {
                remove.insert(agent.pid);
            }
        }
    }

    if !remove.is_empty() {
        agents.retain(|a| !remove.contains(&a.pid));
    }
}
