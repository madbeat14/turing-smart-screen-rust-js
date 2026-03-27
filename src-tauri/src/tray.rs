/// System tray setup for Turing Smart Screen.
///
/// Provides a tray icon with menu items:
/// - Show/Hide Monitor: toggle virtual monitor window visibility
/// - Configure: opens the settings window
/// - Exit: quits the application
///
/// The monitor window is always created and rendering (needed for WebView2
/// screenshot capture), but starts hidden. "Show Monitor" makes it visible
/// on the desktop. The physical display always receives the captured content.

use log::info;
use tauri::{
    AppHandle, Manager,
    menu::{CheckMenuItem, Menu, MenuItem},
    tray::TrayIconBuilder,
};

pub fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let show_monitor = MenuItem::with_id(app, "show_monitor", "Show Virtual Monitor", true, None::<&str>)?;
    let hide_monitor = MenuItem::with_id(app, "hide_monitor", "Hide Virtual Monitor", true, None::<&str>)?;
    let reset_monitor = MenuItem::with_id(app, "reset_monitor", "Reset Monitor", true, None::<&str>)?;
    let boot_item = CheckMenuItem::with_id(
        app,
        "toggle_boot",
        "Start on Boot",
        true,
        crate::startup::get_run_on_startup(),
        None::<&str>,
    )?;
    let configure = MenuItem::with_id(app, "configure", "Configure", true, None::<&str>)?;
    let template_editor = MenuItem::with_id(app, "template_editor", "Template Editor", true, None::<&str>)?;
    let exit = MenuItem::with_id(app, "exit", "Exit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_monitor, &hide_monitor, &reset_monitor, &boot_item, &configure, &template_editor, &exit])?;

    let icon = load_tray_icon();

    let _tray = TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .tooltip("Turing Smart Screen")
        .on_menu_event(move |app, event| {
            match event.id.as_ref() {
                "show_monitor" => {
                    show_virtual_monitor(app);
                }
                "hide_monitor" => {
                    hide_virtual_monitor(app);
                }
                "reset_monitor" => {
                    info!("Reset Monitor requested from tray");
                    if let Some(sender) = app.try_state::<crate::RestartSender>() {
                        let _ = sender.0.send(());
                    }
                }
                "toggle_boot" => {
                    let current = crate::startup::get_run_on_startup();
                    info!("Toggling Start on Boot: {} -> {}", current, !current);
                    if let Err(e) = crate::startup::set_run_on_startup(!current) {
                        log::warn!("Failed to toggle startup: {}", e);
                    }
                }
                "configure" => {
                    info!("Opening settings window");
                    open_settings_window(app);
                }
                "template_editor" => {
                    info!("Opening template editor");
                    open_editor_window(app);
                }
                "exit" => {
                    info!("Exiting from tray");
                    app.exit(0);
                }
                _ => {}
            }
        })
        .build(app)?;

    Ok(())
}

/// Show the virtual monitor window on the desktop.
/// Moves it back to center-screen and brings it to front.
fn show_virtual_monitor(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("monitor") {
        info!("Showing virtual monitor");
        let _ = window.center();
        let _ = window.set_always_on_top(true);
        let _ = window.set_focus();
        // Remove always-on-top after a short delay so it doesn't stay pinned
        let w = window.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(500));
            let _ = w.set_always_on_top(false);
        });
    }
}

/// Hide the virtual monitor by moving it off-screen.
/// We don't use window.hide() because WebView2 stops rendering hidden windows,
/// and we need it to keep rendering for the physical display screenshot capture.
fn hide_virtual_monitor(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("monitor") {
        info!("Hiding virtual monitor (moving off-screen)");
        let _ = window.set_position(tauri::PhysicalPosition::new(-9999, -9999));
    }
}

fn load_tray_icon() -> tauri::image::Image<'static> {
    let png_bytes = include_bytes!("../icons/icon.png");
    match image::load_from_memory(png_bytes) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            tauri::image::Image::new_owned(rgba.into_raw(), w, h)
        }
        Err(_) => {
            // Fallback: 16x16 solid blue icon
            let size = 16u32;
            let mut pixels = Vec::with_capacity((size * size * 4) as usize);
            for _ in 0..(size * size) {
                pixels.extend_from_slice(&[0x00, 0x88, 0xFF, 0xFF]);
            }
            tauri::image::Image::new_owned(pixels, size, size)
        }
    }
}

pub fn open_editor_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("editor") {
        let _ = window.set_focus();
        return;
    }

    let builder = tauri::WebviewWindowBuilder::new(
        app,
        "editor",
        tauri::WebviewUrl::App("editor.html".into()),
    )
    .title("Template Editor — Turing Smart Screen")
    .inner_size(1100.0, 750.0)
    .min_inner_size(900.0, 600.0)
    .resizable(true)
    .center();

    match builder.build() {
        Ok(_) => info!("Template editor window opened"),
        Err(e) => log::error!("Failed to open template editor: {}", e),
    }
}

fn open_settings_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.set_focus();
        return;
    }

    let builder = tauri::WebviewWindowBuilder::new(
        app,
        "settings",
        tauri::WebviewUrl::App("settings.html".into()),
    )
    .title("Settings — Turing Smart Screen")
    .inner_size(500.0, 400.0)
    .resizable(false)
    .center();

    match builder.build() {
        Ok(_) => info!("Settings window opened"),
        Err(e) => log::error!("Failed to open settings window: {}", e),
    }
}
