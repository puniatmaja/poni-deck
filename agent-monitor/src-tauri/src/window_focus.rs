use std::collections::HashMap;
use windows::Win32::Foundation::*;
use windows::Win32::System::Diagnostics::ToolHelp::*;
use windows::Win32::System::Threading::*;
use windows::Win32::UI::WindowsAndMessaging::*;

const MAX_HOPS: usize = 5;
const CONSOLE_HOSTS: [&str; 2] = ["conhost.exe", "openconsole.exe"];

fn build_parent_map() -> HashMap<u32, (u32, String)> {
    let mut map = HashMap::new();
    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return map;
        };
        let mut entry = PROCESSENTRY32W::default();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let name = String::from_utf16_lossy(
                    &entry.szExeFile[..entry.szExeFile.iter().take_while(|&&c| c != 0).count()],
                );
                map.insert(entry.th32ProcessID, (entry.th32ParentProcessID, name));
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
    }
    map
}

struct FindWindowCtx<'a> {
    pid: u32,
    names: &'a HashMap<u32, (u32, String)>,
    found: Option<HWND>,
}

unsafe extern "system" fn find_window_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let ctx = &mut *(lparam.0 as *mut FindWindowCtx);

    if !IsWindowVisible(hwnd).as_bool() {
        return BOOL(1);
    }

    let mut win_pid = 0u32;
    let _ = GetWindowThreadProcessId(hwnd, Some(&mut win_pid));
    if win_pid != ctx.pid {
        return BOOL(1);
    }

    if let Some((_, name)) = ctx.names.get(&win_pid) {
        if name.to_lowercase() == "explorer.exe" {
            return BOOL(1);
        }
    }

    ctx.found = Some(hwnd);
    BOOL(0)
}

fn find_visible_window(pid: u32, names: &HashMap<u32, (u32, String)>) -> Option<HWND> {
    let mut ctx = FindWindowCtx {
        pid,
        names,
        found: None,
    };
    unsafe {
        let _ = EnumWindows(Some(find_window_cb), LPARAM(&mut ctx as *mut FindWindowCtx as isize));
    }
    ctx.found
}

fn activate_window(hwnd: HWND) -> bool {
    unsafe {
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }

        if SetForegroundWindow(hwnd).as_bool() {
            return true;
        }

        if BringWindowToTop(hwnd).is_ok() {
            return true;
        }

        let foreground = GetForegroundWindow();
        let fg_thread = GetWindowThreadProcessId(foreground, None);
        let target_thread = GetWindowThreadProcessId(hwnd, None);
        let current = GetCurrentThreadId();

        let _ = AttachThreadInput(current, fg_thread, BOOL(1));
        let _ = AttachThreadInput(current, target_thread, BOOL(1));
        let ok = SetForegroundWindow(hwnd).as_bool() || BringWindowToTop(hwnd).is_ok();
        let _ = AttachThreadInput(current, target_thread, BOOL(0));
        let _ = AttachThreadInput(current, fg_thread, BOOL(0));

        ok
    }
}

fn find_console_child_window(cur: u32, map: &HashMap<u32, (u32, String)>) -> Option<HWND> {
    for (&child_pid, &(parent, ref name)) in map {
        if parent != cur {
            continue;
        }
        if !CONSOLE_HOSTS.contains(&name.to_lowercase().as_str()) {
            continue;
        }
        if let Some(hwnd) = find_visible_window(child_pid, map) {
            return Some(hwnd);
        }
    }
    None
}

pub fn focus_agent_window(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }

    let map = build_parent_map();
    let mut cur = pid;

    for _ in 0..MAX_HOPS {
        if let Some(hwnd) = find_visible_window(cur, &map) {
            return activate_window(hwnd);
        }

        // Jaring pengaman: pada beberapa versi Windows, jendela konsol dimiliki
        // conhost.exe / OpenConsole.exe (child dari shell, bukan ancestor).
        if let Some(hwnd) = find_console_child_window(cur, &map) {
            return activate_window(hwnd);
        }

        let Some(&(parent, _)) = map.get(&cur) else {
            break;
        };
        if parent == cur {
            break;
        }
        cur = parent;
    }

    false
}

/// Fokus window VSCode (Code.exe) yang me-launch agent `agent_pid`.
/// Satu instance VSCode bisa memiliki beberapa window (folder & workspace) yang
/// seluruhnya dimiliki oleh satu main `Code.exe` — jadi PID saja tidak cukup.
/// Window target ditentukan dengan mencocokkan judul window VSCode terhadap
/// segmen path `working_dir` (folder/workspace tempat agent berjalan).
/// Mengembalikan `true` hanya jika window benar-benar ditemukan & diaktifkan.
pub fn focus_vscode_window(agent_pid: u32, working_dir: &str) -> bool {
    if agent_pid == 0 {
        return false;
    }

    let map = build_parent_map();
    let mut cur = agent_pid;
    let mut code_pids: Vec<u32> = Vec::new();

    for _ in 0..MAX_HOPS {
        let Some(&(parent, ref name)) = map.get(&cur) else {
            break;
        };
        if name.to_lowercase() == "code.exe" {
            if !code_pids.contains(&parent) {
                code_pids.push(parent);
            }
        }
        if parent == cur || !map.contains_key(&parent) {
            break;
        }
        cur = parent;
    }

    if code_pids.is_empty() {
        return false;
    }

    let mut windows: Vec<WindowInfo> = Vec::new();
    for pid in code_pids {
        windows.extend(collect_visible_windows(pid));
    }

    let hwnd = match best_match_window(&windows, working_dir) {
        Some(hwnd) => hwnd,
        // Hanya ada satu window VSCode → aman fokus tanpa perlu mencocokkan judul.
        None if windows.len() == 1 => windows[0].hwnd,
        None => return false,
    };
    activate_window(hwnd)
}

struct WindowInfo {
    hwnd: HWND,
    title: String,
}

struct CollectCtx {
    pid: u32,
    windows: Vec<WindowInfo>,
}

unsafe extern "system" fn collect_window_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let ctx = &mut *(lparam.0 as *mut CollectCtx);

    if !IsWindowVisible(hwnd).as_bool() {
        return BOOL(1);
    }

    let mut win_pid = 0u32;
    let _ = GetWindowThreadProcessId(hwnd, Some(&mut win_pid));
    if win_pid != ctx.pid {
        return BOOL(1);
    }

    let mut buf = [0u16; 512];
    let len = GetWindowTextW(hwnd, &mut buf);
    if len > 0 {
        ctx.windows.push(WindowInfo {
            hwnd,
            title: String::from_utf16_lossy(&buf[..len as usize]),
        });
    }

    BOOL(1)
}

fn collect_visible_windows(pid: u32) -> Vec<WindowInfo> {
    let mut ctx = CollectCtx {
        pid,
        windows: Vec::new(),
    };
    unsafe {
        let _ = EnumWindows(
            Some(collect_window_cb),
            LPARAM(&mut ctx as *mut CollectCtx as isize),
        );
    }
    ctx.windows
}

/// Normalisasi judul window VSCode menjadi "identitas folder/workspace".
/// Format umum: `[Administrator: ]<name> [(Workspace)] - Visual Studio Code`.
fn normalize_title(title: &str) -> String {
    let t = title.split(" - Visual Studio Code").next().unwrap_or(title);
    let t = t.strip_prefix("Administrator: ").unwrap_or(t);
    t.to_lowercase().replace(" (workspace)", "")
}

fn best_match_window(windows: &[WindowInfo], working_dir: &str) -> Option<HWND> {
    let segments: Vec<String> = working_dir
        .split(['/', '\\'])
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase())
        .collect();
    if segments.is_empty() {
        return None;
    }

    let total = segments.len();
    let mut best_score = 0;
    let mut best_depth = usize::MAX;
    let mut best_hwnd: Option<HWND> = None;

    for w in windows {
        let t = normalize_title(&w.title);
        for (i, seg) in segments.iter().enumerate() {
            let score = if t == *seg {
                4
            } else if t.starts_with(seg) || t.ends_with(seg) {
                3
            } else if t.contains(seg) {
                2
            } else {
                0
            };
            if score == 0 {
                continue;
            }
            // Prefer skor tertinggi; seri → segmen terdalam (paling spesifik).
            let depth_from_end = total - 1 - i;
            if score > best_score || (score == best_score && depth_from_end < best_depth) {
                best_score = score;
                best_depth = depth_from_end;
                best_hwnd = Some(w.hwnd);
            }
        }
    }

    best_hwnd
}
