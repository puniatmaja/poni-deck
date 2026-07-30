use tauri::{
    AppHandle,
    Manager,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    image::Image,
};

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

pub fn create_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let show = MenuItem::with_id(app, "show", "Show Overlay", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    let icon = create_icon();

    TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .tooltip("Agent Monitor — 0 agent(s) running")
        .on_menu_event(|app, event| {
            match event.id().as_ref() {
                "show" => {
                    if let Some(window) = app.get_webview_window("overlay") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
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

pub fn update_tray_tooltip(app: &AppHandle, count: usize) {
    if let Some(tray) = app.tray_by_id("main") {
        let tooltip = if count == 0 {
            "Agent Monitor — 0 agent(s) running".to_string()
        } else {
            format!("Agent Monitor — {} agent(s) running", count)
        };
        let _ = tray.set_tooltip(Some(&tooltip));
    }
}
