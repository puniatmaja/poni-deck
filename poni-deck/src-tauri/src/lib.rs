mod click_handler;
mod config;
mod notifier;
mod process_scanner;
mod state;
mod status_reader;
mod tray;
mod window_focus;

use state::{AgentInfo, AppState, Config};
use std::collections::{HashMap, HashSet};
use tauri::{Emitter, Manager};
use windows::Win32::Foundation::*;
use windows::Win32::System::Threading::*;

#[tauri::command]
fn get_agents(state: tauri::State<'_, AppState>) -> Result<Vec<AgentInfo>, String> {
    let agents = state.agents.lock().map_err(|e| e.to_string())?;
    Ok(agents.values().cloned().collect())
}

#[tauri::command]
fn get_config(state: tauri::State<'_, AppState>) -> Result<Config, String> {
    let cfg = state.config.lock().map_err(|e| e.to_string())?;
    Ok(cfg.clone())
}

#[tauri::command]
fn set_config(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    new_config: Config,
) -> Result<(), String> {
    {
        let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
        *cfg = new_config.clone();
    }
    config::save_config(&new_config).map_err(|e| e.to_string())?;

    if let Some(window) = app.get_webview_window("overlay") {
        window
            .set_always_on_top(new_config.always_on_top)
            .map_err(|e| e.to_string())?;
    }

    let cfg = state.config.lock().map_err(|e| e.to_string())?;
    if cfg.auto_start != new_config.auto_start {
        config::set_auto_start(new_config.auto_start).map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
fn open_terminal(path: String) -> Result<(), String> {
    click_handler::open_terminal(&path).map_err(|e| e.to_string())
}

#[tauri::command]
fn open_vscode(path: String) -> Result<(), String> {
    click_handler::open_vscode(&path).map_err(|e| e.to_string())
}

#[tauri::command]
fn open_for_launcher(
    path: String,
    launcher: String,
    pid: u32,
) -> Result<(), String> {
    click_handler::open_focus_or_new(&path, &launcher, pid).map_err(|e| e.to_string())
}

#[tauri::command]
fn resize_window(
    app: tauri::AppHandle,
    width: f64,
    height: f64,
    preserve_center_x: Option<bool>,
    expand_up: Option<bool>,
) -> Result<u32, String> {
    // Ruang aman (physical px) antara tepi window dan tepi work-area monitor
    // (taskbar), agar handle resize tidak pernah keluar layar / tertutup taskbar.
    const EDGE_MARGIN: i32 = 24;

    let window = app.get_webview_window("overlay").ok_or("window not found")?;

    let current_height = window.outer_size().unwrap_or_default().height as i32;
    let expand_up = expand_up.unwrap_or(false);
    let mut final_h = height as i32;

    // Clamp hanya saat window bertambah tinggi; menyusut (collapse) selalu diizinkan.
    if final_h > current_height {
        if let Ok(Some(monitor)) = window.current_monitor() {
            let wa = monitor.work_area();
            let top_y = window.outer_position().unwrap_or_default().y;
            if expand_up {
                // Tumbuh ke atas: batasi agar tepi atas tidak naik melewati work-area.
                let work_top = wa.position.y;
                let max_h = (top_y - work_top - EDGE_MARGIN).max(current_height);
                final_h = final_h.min(max_h);
            } else {
                // Tumbuh ke bawah: batasi agar tepi bawah tetap di atas work-area.
                let work_bottom = wa.position.y + wa.size.height as i32;
                let max_h = (work_bottom - top_y - EDGE_MARGIN).max(current_height);
                final_h = final_h.min(max_h);
            }
        }
    }
    let final_h = final_h.max(1);
    let size = tauri::PhysicalSize::new(width as u32, final_h as u32);

    let pos = window.outer_position().map_err(|e| e.to_string())?;
    let old_size = window.outer_size().unwrap_or(size);

    // Mode up: jangkar di tepi bawah — geser posisi agar tepi bawah tetap diam
    // saat tinggi berubah (tumbuh ke atas maupun menyusut kembali).
    let mut new_y = pos.y;
    if expand_up {
        new_y = pos.y - (final_h - old_size.height as i32);
    }

    if preserve_center_x == Some(true) {
        let dx = ((old_size.width as i64 - width as i64) / 2) as i32;
        let mut new_x = pos.x + dx;

        if let Ok(Some(monitor)) = window.current_monitor() {
            let area = monitor.position();
            let mw = monitor.size().width as i32;
            new_x = if size.width as i32 <= mw {
                new_x.clamp(area.x, area.x + mw - size.width as i32)
            } else {
                area.x
            };
        }

        window.set_size(size).map_err(|e| e.to_string())?;
        window
            .set_position(tauri::PhysicalPosition::new(new_x, new_y))
            .map_err(|e| e.to_string())?;
    } else {
        window.set_size(size).map_err(|e| e.to_string())?;
        if new_y != pos.y {
            window
                .set_position(tauri::PhysicalPosition::new(pos.x, new_y))
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(final_h as u32)
}

fn create_single_instance_mutex() -> Option<HANDLE> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;

    unsafe {
        let name: Vec<u16> = OsStr::new("PoniDeck-SingleInstance")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let Ok(handle) = CreateMutexW(
            None,
            false,
            PCWSTR(name.as_ptr()),
        ) else {
            return None;
        };

        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(handle);
            return None;
        }

        Some(handle)
    }
}

pub fn run() {
    let app_state = AppState::new();

    let _mutex_handle = create_single_instance_mutex().unwrap_or_else(|| {
        eprintln!("Another instance of Poni Deck is already running.");
        std::process::exit(0);
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state)
        .setup(|app| {
            if let Some(window) = app.get_webview_window("overlay") {
                if let Ok(Some(monitor)) = window.current_monitor() {
                    let screen = monitor.size();
                    let win_size = window.outer_size().unwrap_or_default();
                    let x = (screen.width as i32 - win_size.width as i32).max(0) / 2;
                    let _ = window.set_position(tauri::PhysicalPosition::new(x, 0));
                }
                if let Ok(cfg) = app.state::<AppState>().config.lock() {
                    let _ = window.set_always_on_top(cfg.always_on_top);
                }
            }

            tray::create_tray(app)
                .map_err(|e| format!("Failed to create system tray: {}", e))?;

            clear_agents_dir();

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                polling_loop(handle).await;
            });

            tauri::async_runtime::spawn(async move {
                let mut interval =
                    tokio::time::interval(std::time::Duration::from_secs(60));
                loop {
                    interval.tick().await;
                    cleanup_orphan_files();
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_agents,
            get_config,
            set_config,
            open_terminal,
            open_vscode,
            open_for_launcher,
            resize_window,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

async fn polling_loop(app: tauri::AppHandle) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(
        get_polling_interval(&app),
    ));

    loop {
        interval.tick().await;

        let agents = process_scanner::scan_agents();
        let current_pids: HashSet<u32> = agents.iter().map(|a| a.pid).collect();

        let state = app.state::<AppState>();
        let notifications_enabled = state
            .config
            .lock()
            .map(|c| c.notifications_enabled)
            .unwrap_or(true);

        let mut prev_pids = match state.previous_pids.lock() {
            Ok(p) => p,
            Err(_) => continue,
        };

        for agent in &agents {
            if !prev_pids.contains(&agent.pid) {
                if notifications_enabled {
                    notifier::notify_started(&app, agent);
                }
                let _ = app.emit(
                    "agent-event",
                    serde_json::json!({
                        "type": "started",
                        "agent": agent
                    }),
                );
            }
        }

        for pid in prev_pids.iter() {
            if !current_pids.contains(pid) {
                if let Ok(mut agents_map) = state.agents.lock() {
                    if let Some(info) = agents_map.remove(pid) {
                        if notifications_enabled {
                            notifier::notify_stopped(&app, &info);
                        }
                        let _ = app.emit(
                            "agent-event",
                            serde_json::json!({
                                "type": "stopped",
                                "agent": info
                            }),
                        );
                    }
                }
                status_reader::remove_file(*pid);
            }
        }

        *prev_pids = current_pids;

        if let Ok(mut agents_map) = state.agents.lock() {
            agents_map.clear();
            for agent in &agents {
                agents_map.insert(agent.pid, agent.clone());
            }
        }

        let counts = count_statuses(&agents);

        let _ = app.emit("agent-update", &agents);
        tray::update_tray_tooltip(&app, agents.len(), &counts);
    }
}

fn count_statuses(agents: &[AgentInfo]) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for agent in agents {
        *counts.entry(agent.state.clone()).or_insert(0) += 1;
    }
    counts
}

fn clear_agents_dir() {
    if let Ok(entries) = std::fs::read_dir(config::agents_dir()) {
        for entry in entries.flatten() {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

fn cleanup_orphan_files() {
    let pids: HashSet<u32> = process_scanner::scan_agents().iter().map(|a| a.pid).collect();
    if let Ok(entries) = std::fs::read_dir(config::agents_dir()) {
        for entry in entries.flatten() {
            let name = match entry.file_name().to_str() {
                Some(name) => name.to_string(),
                None => continue,
            };
            let Some(pid) = name.strip_suffix(".json").and_then(|s| s.parse::<u32>().ok()) else {
                continue;
            };
            if !pids.contains(&pid) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

fn get_polling_interval(app: &tauri::AppHandle) -> u64 {
    app.state::<AppState>()
        .config
        .lock()
        .map(|c| c.polling_interval_ms)
        .unwrap_or(2000)
}
