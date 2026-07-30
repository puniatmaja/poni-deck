use crate::state::AgentInfo;
use windows::core::PWSTR;
use windows::Win32::Foundation::*;
use windows::Win32::System::Diagnostics::ToolHelp::*;
use windows::Win32::System::Threading::*;

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

pub fn scan_agents() -> Vec<AgentInfo> {
    let mut agents = Vec::new();

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

                if name.to_lowercase() == "opencode.exe" {
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

                    agents.push(AgentInfo {
                        pid,
                        exe_path: exe_path.clone(),
                        working_dir,
                        state: "running".to_string(),
                    });
                }

                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }

        let _ = CloseHandle(snapshot);
    }

    agents
}
