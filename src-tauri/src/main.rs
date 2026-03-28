// Turing Smart Screen — Rust + JS Rewrite
// Entry point: Tauri app setup, sensor loop, display loop

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod display;
mod lhm;
mod screenshot;
mod sensors;
mod startup;
mod templates;
mod tray;

use display::diff::FrameDiffer;
use display::rgb565::rgba_to_rgb565_le;
use display::{Orientation, create_display};
use log::{error, info, warn};
use serde::Serialize;
use std::sync::mpsc;
use tauri::Manager;

/// Wrapper so we can store the restart sender in Tauri managed state.
pub struct RestartSender(pub mpsc::Sender<()>);

/// Log a message from the webview to the Rust logger
#[tauri::command]
fn webview_log(level: String, msg: String) {
    match level.as_str() {
        "error" => error!("[WEBVIEW] {}", msg),
        "warn" => warn!("[WEBVIEW] {}", msg),
        _ => info!("[WEBVIEW] {}", msg),
    }
}

fn main() {
    // Write logs to a file so we can diagnose issues even when running
    // as admin (no console window). File: turing-smart-screen.log next to the exe.
    init_file_logger();
    info!("Starting Turing Smart Screen");

    let app_config = config::AppConfig::load_or_default();
    let display_config = app_config.display.clone();
    let shared_config = std::sync::Arc::new(std::sync::Mutex::new(app_config));

    // Channel for signaling the display loop to restart (reload config, re-apply brightness/orientation)
    let (restart_tx, restart_rx) = mpsc::channel::<()>();

    tauri::Builder::default()
        .manage(shared_config)
        .manage(RestartSender(restart_tx))
        .invoke_handler(tauri::generate_handler![
            get_config,
            save_config,
            list_serial_ports,
            restart_display,
            reload_monitor,
            get_run_on_startup,
            set_run_on_startup,
            templates::list_templates,
            templates::read_template_manifest,
            templates::read_template_files,
            templates::get_template_paths,
            templates::inject_custom_template,
            templates::save_template,
            templates::delete_template,
            templates::clone_template,
            open_editor,
            webview_log,
        ])
        .setup(move |app| {
            let app_handle = app.handle().clone();

            // Start LibreHardwareMonitor for CPU/GPU temperature sensors.
            // LHM runs as a hidden background process, outputs JSON to stdout.
            // The returned Arc is updated every ~1s by a reader thread.
            let lhm_sensors = if let Ok(resource_dir) = app.path().resource_dir() {
                lhm::start_lhm(&resource_dir)
            } else {
                warn!("Could not determine resource directory for LHM");
                std::sync::Arc::new(std::sync::Mutex::new(lhm::LhmSensorData::default()))
            };

            // Start the monitor window off-screen, then make it visible so
            // WebView2 keeps rendering (needed for screenshot capture).
            // The window starts with visible:false in tauri.conf.json to
            // prevent a brief flash on the desktop at startup.
            if let Some(monitor_win) = app_handle.get_webview_window("monitor") {
                let _ = monitor_win.set_position(tauri::PhysicalPosition::new(-9999, -9999));
                let _ = monitor_win.show();
            }

            // Set up system tray
            if let Err(e) = tray::setup_tray(&app_handle) {
                error!("Failed to setup system tray: {}", e);
            }

            // Spawn sensor loop in a dedicated thread
            let sensor_handle = app_handle.clone();
            std::thread::spawn(move || {
                sensors::sensor_loop(sensor_handle, lhm_sensors);
            });

            // Spawn display loop in a dedicated thread (serial I/O is blocking)
            let display_handle = app_handle.clone();
            let config = display_config.clone();
            let loop_shared_config = app.state::<SharedConfig>().inner().clone();
            std::thread::spawn(move || {
                if let Err(e) = run_display_loop(display_handle, config, loop_shared_config, restart_rx) {
                    error!("Display loop failed: {}", e);
                }
            });

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            if let tauri::RunEvent::Exit = event {
                info!("Shutting down — stopping LibreHardwareMonitor");
                lhm::stop_lhm();
            }
        });
}

/// Main display loop: screenshot → resize → RGB565 → diff → serial TX
fn run_display_loop(
    app_handle: tauri::AppHandle,
    config: config::DisplayConfig,
    shared_config: SharedConfig,
    restart_rx: mpsc::Receiver<()>,
) -> anyhow::Result<()> {
    // Give the webview a moment to render its first frame
    std::thread::sleep(std::time::Duration::from_secs(2));

    // Create display driver
    let mut display = match create_display(&config) {
        Ok(d) => d,
        Err(e) => {
            warn!("Could not connect to display: {}. Running without display output.", e);
            return Ok(());
        }
    };

    // Initialize display
    display.initialize()?;

    if config.reset_on_startup {
        info!("Resetting display...");
        if let Err(e) = display.reset() {
            warn!("Display reset failed: {}", e);
        }
        // Reset causes USB re-enumeration — the old serial handle goes stale.
        // Keep retrying initialize() until the port comes back and handshake succeeds.
        loop {
            match display.initialize() {
                Ok(()) => break,
                Err(e) => {
                    info!("Post-reset initialize pending ({}), retrying...", e);
                    std::thread::sleep(std::time::Duration::from_secs(2));
                }
            }
        }
        // Clear the reconnected flag — this was an expected reset, not a cable unplug
        display.take_reconnected();
    }

    // Set brightness and orientation
    display.set_brightness(config.brightness)?;

    let orientation = if config.display_reverse {
        Orientation::ReverseLandscape
    } else {
        Orientation::Landscape
    };
    display.set_orientation(orientation)?;

    let screen_w = display.get_width();
    let screen_h = display.get_height();
    info!("Display ready: {}x{}", screen_w, screen_h);

    // Frame differ for optimizing serial traffic
    let mut differ = FrameDiffer::new(screen_w, screen_h, 32);

    // Pre-allocated buffer for extracting dirty rect RGBA regions (avoids per-rect allocation)
    let mut region_rgba_buf: Vec<u8> = Vec::with_capacity((screen_w as usize) * (screen_h as usize) * 4);

    // Get the webview window handle
    let window = app_handle
        .get_webview_window("monitor")
        .ok_or_else(|| anyhow::anyhow!("Monitor window not found"))?;

    // Track iterations for periodic health check (every ~5 seconds)
    let mut health_check_counter: u32 = 0;
    // Frame counter for debug logging (first few frames)
    let mut frame_counter: u64 = 0;

    loop {
        // Check for restart signal (settings changed or tray "Reset Monitor")
        if restart_rx.try_recv().is_ok() {
            info!("Display restart signal received — reloading config");
            if let Ok(cfg) = shared_config.lock() {
                let new_display = &cfg.display;
                if let Err(e) = display.set_brightness(new_display.brightness) {
                    warn!("Failed to set brightness: {}", e);
                }
                let new_orientation = if new_display.display_reverse {
                    Orientation::ReverseLandscape
                } else {
                    Orientation::Landscape
                };
                if let Err(e) = display.set_orientation(new_orientation) {
                    warn!("Failed to set orientation: {}", e);
                }
                info!("Display config reapplied (brightness={}, reverse={})",
                    new_display.brightness, new_display.display_reverse);
            }
            // Force full frame redraw
            differ.reset();
            // Wait for webview to reload and the new template to finish rendering.
            // Custom templates load asynchronously via IPC, so without this delay
            // the display loop would screenshot a blank/partially-rendered page.
            std::thread::sleep(std::time::Duration::from_secs(3));
            // Reset frame counter so we get debug logs for the post-restart frames
            frame_counter = 0;
            info!("Post-restart: resuming screenshot capture");
        }

        // Check if a reconnection happened (write-error path sets this flag).
        // Also periodically poll the OS port list to catch silent disconnects
        // where writes don't fail (Windows can buffer serial writes).
        let needs_reinit = if display.take_reconnected() {
            info!("Write-error reconnection detected");
            true
        } else {
            health_check_counter += 1;
            if health_check_counter >= 10 {
                health_check_counter = 0;
                if display.check_port_health() {
                    info!("USB cable reconnection detected via port health check");
                    true
                } else {
                    false
                }
            } else {
                false
            }
        };

        if needs_reinit {
            // Mirror what "Reset Monitor" does — just re-apply brightness and
            // orientation. No initialize() call — that would send a HELLO handshake
            // which isn't needed and can interfere with the display state.
            info!("Re-applying display settings after reconnection");
            std::thread::sleep(std::time::Duration::from_secs(2));
            if let Err(e) = display.set_brightness(config.brightness) {
                warn!("Failed to restore brightness: {}", e);
            }
            if let Err(e) = display.set_orientation(orientation) {
                warn!("Failed to restore orientation: {}", e);
            }
            differ.reset();
            info!("Display settings restored after reconnection");
            continue;
        }

        // Capture webview screenshot
        let rgba = match screenshot::capture_webview_screenshot(
            &window,
            screen_w as u32,
            screen_h as u32,
        ) {
            Ok(pixels) => {
                if frame_counter < 5 {
                    info!("Screenshot #{}: {} bytes, {}x{}", frame_counter, pixels.len(), screen_w, screen_h);
                }
                // Save the first screenshot after each restart as a debug PNG
                if frame_counter == 0 {
                    // Count non-black pixels to detect blank renders
                    let non_black = pixels.chunks(4)
                        .filter(|p| p[0] > 5 || p[1] > 5 || p[2] > 5)
                        .count();
                    let total = pixels.len() / 4;
                    info!("Screenshot pixel analysis: {}/{} non-black pixels ({:.1}%)",
                        non_black, total, (non_black as f64 / total as f64) * 100.0);
                    
                    let ts = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let debug_path = config::AppConfig::config_dir()
                        .join(format!("debug_screenshot_{}.png", ts));
                    if let Some(img) = image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(
                        screen_w as u32, screen_h as u32, pixels.clone()
                    ) {
                        match img.save(&debug_path) {
                            Ok(_) => info!("Debug screenshot saved to {:?}", debug_path),
                            Err(e) => warn!("Failed to save debug screenshot: {}", e),
                        }
                    }
                }
                pixels
            }
            Err(e) => {
                warn!("Screenshot failed: {}", e);
                std::thread::sleep(std::time::Duration::from_secs(1));
                continue;
            }
        };

        // Convert to RGB565 for diffing
        let rgb565 = rgba_to_rgb565_le(&rgba);

        // Diff against previous frame
        let dirty_rects = differ.diff(&rgb565);

        if dirty_rects.is_empty() {
            // Frame unchanged — skip serial TX
            if frame_counter < 5 {
                info!("Frame #{}: no dirty rects (unchanged)", frame_counter);
            }
            frame_counter += 1;
            std::thread::sleep(std::time::Duration::from_millis(100));
            continue;
        }

        if frame_counter < 10 {
            info!("Frame #{}: {} dirty rects to send", frame_counter, dirty_rects.len());
        }

        // Send only changed regions
        for rect in &dirty_rects {
            // Extract the RGBA sub-region into the reusable buffer
            let stride = (screen_w as usize) * 4;
            region_rgba_buf.clear();

            for row in 0..rect.h as usize {
                let y_offset = (rect.y as usize + row) * stride;
                let x_offset = (rect.x as usize) * 4;
                let start = y_offset + x_offset;
                let end = start + (rect.w as usize) * 4;
                if end <= rgba.len() {
                    region_rgba_buf.extend_from_slice(&rgba[start..end]);
                }
            }

            if let Err(e) =
                display.display_rgba_image(&region_rgba_buf, rect.x, rect.y, rect.w, rect.h)
            {
                warn!("Display update failed for rect {:?}: {}", rect, e);
                break;
            }
        }

        frame_counter += 1;

        // Adaptive sleep: shorter when many changes, longer when few
        let sleep_ms = if dirty_rects.len() > 5 { 200 } else { 500 };
        std::thread::sleep(std::time::Duration::from_millis(sleep_ms));
    }
}

// --- Tauri Commands ---

type SharedConfig = std::sync::Arc<std::sync::Mutex<config::AppConfig>>;

/// Client-safe config that omits sensitive fields like API keys.
#[derive(Serialize)]
struct ClientConfig {
    config: ClientGeneralConfig,
    display: config::DisplayConfig,
}

#[derive(Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
struct ClientGeneralConfig {
    com_port: String,
    theme: String,
    hw_sensors: String,
    eth: String,
    wlo: String,
    cpu_fan: String,
    ping: String,
    weather_api_key_set: bool,
    weather_latitude: f64,
    weather_longitude: f64,
    weather_units: String,
    weather_language: String,
}

#[tauri::command]
fn get_config(config: tauri::State<SharedConfig>) -> Result<ClientConfig, String> {
    let cfg = config.lock().map_err(|e| {
        error!("Config lock failed: {}", e);
        "Failed to read configuration".to_string()
    })?;
    Ok(ClientConfig {
        config: ClientGeneralConfig {
            com_port: cfg.config.com_port.clone(),
            theme: cfg.config.theme.clone(),
            hw_sensors: cfg.config.hw_sensors.clone(),
            eth: cfg.config.eth.clone(),
            wlo: cfg.config.wlo.clone(),
            cpu_fan: cfg.config.cpu_fan.clone(),
            ping: cfg.config.ping.clone(),
            weather_api_key_set: !cfg.config.weather_api_key.is_empty(),
            weather_latitude: cfg.config.weather_latitude,
            weather_longitude: cfg.config.weather_longitude,
            weather_units: cfg.config.weather_units.clone(),
            weather_language: cfg.config.weather_language.clone(),
        },
        display: cfg.display.clone(),
    })
}

#[tauri::command]
fn save_config(
    config: tauri::State<SharedConfig>,
    new_config: config::AppConfig,
) -> Result<(), String> {
    // Validate before saving
    new_config.validate()?;

    // Save to file next to the executable (not CWD)
    let config_path = config::AppConfig::config_path();
    let yaml = serde_yaml::to_string(&new_config).map_err(|e| {
        error!("Config serialization failed: {}", e);
        "Failed to serialize configuration".to_string()
    })?;
    std::fs::write(&config_path, &yaml).map_err(|e| {
        error!("Config write failed to {}: {}", config_path.display(), e);
        "Failed to write configuration file".to_string()
    })?;

    // Update in-memory config
    let mut cfg = config.lock().map_err(|e| {
        error!("Config lock failed: {}", e);
        "Failed to update configuration".to_string()
    })?;
    *cfg = new_config;

    info!("Configuration saved to {}", config_path.display());
    Ok(())
}

#[tauri::command]
fn restart_display(sender: tauri::State<RestartSender>) -> Result<(), String> {
    sender.0.send(()).map_err(|e| e.to_string())?;
    info!("Display restart signal sent");
    Ok(())
}

#[tauri::command]
fn reload_monitor(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("monitor") {
        win.eval("window.location.reload();").map_err(|e| e.to_string())?;
        info!("Monitor webview reloaded");
    }
    Ok(())
}

#[tauri::command]
fn list_serial_ports() -> Result<Vec<String>, String> {
    let ports = serialport::available_ports().map_err(|e| e.to_string())?;
    Ok(ports.iter().map(|p| p.port_name.clone()).collect())
}

#[tauri::command]
fn get_run_on_startup() -> bool {
    startup::get_run_on_startup()
}

#[tauri::command]
fn set_run_on_startup(enable: bool) -> Result<(), String> {
    startup::set_run_on_startup(enable)
}

#[tauri::command]
fn open_editor(app: tauri::AppHandle) -> Result<(), String> {
    tray::open_editor_window(&app);
    Ok(())
}

/// Initialize logger that writes to a file next to the executable.
/// This ensures we can read logs even when running as admin (no console).
fn init_file_logger() {
    use std::io::Write;

    let log_path = config::AppConfig::config_dir().join("turing-smart-screen.log");

    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&log_path);

    match file {
        Ok(file) => {
            let file = std::sync::Mutex::new(file);
            env_logger::Builder::new()
                .filter_level(log::LevelFilter::Info)
                .format(move |_buf, record| {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default();
                    let secs = now.as_secs();
                    // Format as human-readable UTC: YYYY-MM-DD HH:MM:SS
                    let days = secs / 86400;
                    let time_of_day = secs % 86400;
                    let hours = time_of_day / 3600;
                    let minutes = (time_of_day % 3600) / 60;
                    let seconds = time_of_day % 60;
                    // Simple epoch-to-date (valid 2000-2099)
                    let (year, month, day) = epoch_days_to_date(days);
                    let msg = format!(
                        "[{:04}-{:02}-{:02} {:02}:{:02}:{:02} {} {}] {}\n",
                        year, month, day, hours, minutes, seconds,
                        record.level(),
                        record.target(),
                        record.args()
                    );
                    if let Ok(mut f) = file.lock() {
                        let _ = f.write_all(msg.as_bytes());
                        let _ = f.flush();
                    }
                    Ok(())
                })
                .init();
        }
        Err(_) => {
            // Fallback to stdout/env_logger
            env_logger::Builder::new()
                .filter_level(log::LevelFilter::Info)
                .init();
        }
    }
}

/// Convert days since Unix epoch to (year, month, day). Simple algorithm, valid 1970-2099.
fn epoch_days_to_date(days: u64) -> (u64, u64, u64) {
    let mut y = 1970;
    let mut remaining = days;
    loop {
        let days_in_year = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let month_days = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut m = 0;
    for md in &month_days {
        if remaining < *md {
            break;
        }
        remaining -= md;
        m += 1;
    }
    (y, m + 1, remaining + 1)
}
