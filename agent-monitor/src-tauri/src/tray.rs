use std::collections::HashMap;
use tauri::{
    AppHandle,
    Emitter,
    Manager,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    image::Image,
};

const STATUS_PRIORITY: [&str; 5] = ["error", "waiting_confirmation", "working", "running", "idle"];

fn create_icon() -> Image<'static> {
    let png_bytes = include_bytes!("../icons/icon.png");
    let decoder = png::Decoder::new(&png_bytes[..]);
    if let Ok(mut reader) = decoder.read_info() {
        let info = reader.info();
        let width = info.width;
        let height = info.height;
        let mut buf = vec![0u8; reader.output_buffer_size()];
        if reader.next_frame(&mut buf).is_ok() {
            return Image::new_owned(buf, width, height);
        }
    }
    Image::new_owned(vec![0u8; 32 * 32 * 4], 32, 32)
}

fn show_overlay(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub fn create_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let show = MenuItem::with_id(app, "show", "Show Overlay", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings...", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &settings, &quit])?;

    let icon = create_icon();

    TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .tooltip("Agent Monitor — 0 agents · idle")
        .on_menu_event(|app, event| {
            match event.id().as_ref() {
                "show" => show_overlay(app),
                "settings" => {
                    show_overlay(app);
                    let _ = app.emit("open-settings", ());
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {}
            }
        })
        .build(app)?;

    Ok(())
}

pub fn update_tray_tooltip(app: &AppHandle, count: usize, counts: &HashMap<String, usize>) {
    if let Some(tray) = app.tray_by_id("main") {
        let summary = summarize_statuses(count, counts);
        let tooltip = format!("Agent Monitor — {} agents · {}", count, summary);
        let _ = tray.set_tooltip(Some(&tooltip));
    }
}

fn summarize_statuses(count: usize, counts: &HashMap<String, usize>) -> String {
    if count == 0 {
        return "idle".to_string();
    }

    let present: Vec<&str> = STATUS_PRIORITY
        .iter()
        .copied()
        .filter(|s| counts.get(*s).copied().unwrap_or(0) > 0)
        .collect();

    if present.is_empty() {
        return "running".to_string();
    }

    if present.len() == 1 {
        return present[0].to_string();
    }

    let top = present[0];
    format!("{} {}", counts.get(top).copied().unwrap_or(0), top)
}
