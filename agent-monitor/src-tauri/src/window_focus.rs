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
