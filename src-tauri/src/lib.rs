// Turing Smart Screen — Rust + JS Library Entry Point
// Tauri v2 app setup, sensor loop, display loop

pub mod config;
pub mod display;
pub mod lhm;
pub mod screenshot;
pub mod sensors;
pub mod startup;
pub mod templates;
pub mod tray;
pub mod window_state;

use display::diff::{DirtyRect, FrameDiffer};
use display::rgb565::rgba_to_rgb565_le_into;
use display::{create_display, LcdDisplay, Orientation};
use log::{debug, error, info, warn};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use tauri::Manager;

/// Wrapper so we can store the restart sender in Tauri managed state.
pub struct RestartSender(pub mpsc::Sender<()>);

/// A dirty display region with pre-extracted RGBA pixel data, ready to send over serial.
struct RegionPayload {
    rect: DirtyRect,
    rgba: Vec<u8>,
}

/// Commands from the capture thread to the serial TX thread.
enum TxCommand {
    /// Update the display with the given dirty regions (RGBA data already extracted).
    Frame(Vec<RegionPayload>),
    /// Re-apply brightness and orientation (after a config reload via tray).
    Reinit { brightness: u8, orientation: Orientation },
}

/// Log a message from the webview to the Rust logger
#[tauri::command]
fn webview_log(level: String, msg: String) {
    if msg.len() > 4096 {
        warn!("[WEBVIEW] Message truncated (was {} bytes)", msg.len());
        return;
    }
    match level.as_str() {
        "error" => error!("[WEBVIEW] {}", msg),
        "warn" => warn!("[WEBVIEW] {}", msg),
        "debug" => debug!("[WEBVIEW] {}", msg),
        _ => info!("[WEBVIEW] {}", msg),
    }
}

pub fn run() {
    // Ensure writable data dir exists and migrate config from old exe-adjacent location.
    config::AppConfig::ensure_data_dir();
    config::AppConfig::migrate_from_install_dir();

    // Write logs to a file so we can diagnose issues even when running
    // as admin (no console window). File: turing-smart-screen.log in AppData.
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
            templates::user_template_exists,
            templates::clone_template,
            templates::make_builtin_editable,
            templates::reset_builtin_template,
            window_state::get_window_state,
            window_state::save_window_state,
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
                let _ = monitor_win.set_skip_taskbar(true);
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
                if let Err(e) =
                    run_display_loop(display_handle, config, loop_shared_config, restart_rx)
                {
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
                display::serial::signal_shutdown();
                lhm::stop_lhm();
            }
        });
}

/// Main display loop: sets up the display, spawns the serial TX thread, then runs
/// the capture loop.
fn run_display_loop(
    app_handle: tauri::AppHandle,
    config: config::DisplayConfig,
    shared_config: SharedConfig,
    restart_rx: mpsc::Receiver<()>,
) -> anyhow::Result<()> {
    // Give the webview a moment to render its first frame
    std::thread::sleep(std::time::Duration::from_secs(2));

    // Create and initialise display
    let mut display = match create_display(&config) {
        Ok(d) => d,
        Err(e) => {
            warn!(
                "Could not connect to display: {}. Running without display output.",
                e
            );
            return Ok(());
        }
    };

    display.initialize()?;

    if config.reset_on_startup {
        info!("Resetting display...");
        if let Err(e) = display.reset() {
            warn!("Display reset failed: {}", e);
        }
        loop {
            match display.initialize() {
                Ok(()) => break,
                Err(e) => {
                    info!("Post-reset initialize pending ({}), retrying...", e);
                    std::thread::sleep(std::time::Duration::from_secs(2));
                }
            }
        }
        display.take_reconnected();
    }

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

    // AtomicBool: TX thread sets this after a write-error reconnect so the capture
    // thread knows to reset the frame differ on the next iteration.
    let write_reconnected = Arc::new(AtomicBool::new(false));
    let write_reconnected_tx = write_reconnected.clone();

    // Bounded channel (capacity 1) — natural back-pressure when TX is busy.
    let (tx_send, tx_recv) = mpsc::sync_channel::<TxCommand>(1);

    // Spawn TX thread — takes ownership of display (LcdDisplay: Send).
    let tx_brightness = config.brightness;
    let tx_orientation = orientation;
    std::thread::spawn(move || {
        run_tx_loop(display, tx_recv, write_reconnected_tx, tx_brightness, tx_orientation);
    });

    // --- Capture loop ---
    let mut differ = FrameDiffer::new(screen_w, screen_h, 16);
    let mut rgb565_buf: Vec<u8> =
        Vec::with_capacity((screen_w as usize) * (screen_h as usize) * 2);

    let window = app_handle
        .get_webview_window("monitor")
        .ok_or_else(|| anyhow::anyhow!("Monitor window not found"))?;

    let mut frame_counter: u64 = 0;

    loop {
        // Check for restart signal
        if restart_rx.try_recv().is_ok() {
            info!("Display restart signal received — reloading config");
            let (new_brightness, new_orientation) = if let Ok(cfg) = shared_config.lock() {
                let o = if cfg.display.display_reverse {
                    Orientation::ReverseLandscape
                } else {
                    Orientation::Landscape
                };
                info!(
                    "Display config reapplied (brightness={}, reverse={})",
                    cfg.display.brightness, cfg.display.display_reverse
                );
                (cfg.display.brightness, o)
            } else {
                (config.brightness, orientation)
            };
            let _ = tx_send.send(TxCommand::Reinit {
                brightness: new_brightness,
                orientation: new_orientation,
            });
            differ.reset();
            std::thread::sleep(std::time::Duration::from_secs(3));
            frame_counter = 0;
            info!("Post-restart: resuming screenshot capture");
        }

        // Write-error reconnect detected by TX thread
        if write_reconnected.swap(false, Ordering::Relaxed) {
            info!("Capture: write-error reconnect signalled — resetting frame differ");
            differ.reset();
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
                    info!(
                        "Screenshot #{}: {} bytes, {}x{}",
                        frame_counter,
                        pixels.len(),
                        screen_w,
                        screen_h
                    );
                }
                pixels
            }
            Err(e) => {
                warn!("Screenshot failed: {}", e);
                std::thread::sleep(std::time::Duration::from_secs(1));
                continue;
            }
        };

        // Convert to RGB565 and diff against previous frame
        rgba_to_rgb565_le_into(&rgba, &mut rgb565_buf);
        let mut dirty_rects = differ.diff(&rgb565_buf);

        let full_frame_bytes = (screen_w as usize) * (screen_h as usize) * 2;
        let total_dirty_bytes: usize = dirty_rects
            .iter()
            .map(|r| (r.w as usize) * (r.h as usize) * 2)
            .sum();
        if total_dirty_bytes >= full_frame_bytes * 85 / 100 {
            dirty_rects = vec![DirtyRect { x: 0, y: 0, w: screen_w, h: screen_h }];
        }

        if dirty_rects.is_empty() {
            frame_counter += 1;
            std::thread::sleep(std::time::Duration::from_millis(50));
            continue;
        }

        let stride = (screen_w as usize) * 4;
        let regions: Vec<RegionPayload> = dirty_rects
            .iter()
            .map(|rect| {
                let mut rgba_region =
                    Vec::with_capacity((rect.w as usize) * (rect.h as usize) * 4);
                for row in 0..rect.h as usize {
                    let y_offset = (rect.y as usize + row) * stride;
                    let x_offset = (rect.x as usize) * 4;
                    let start = y_offset + x_offset;
                    let end = start + (rect.w as usize) * 4;
                    if end <= rgba.len() {
                        rgba_region.extend_from_slice(&rgba[start..end]);
                    }
                }
                RegionPayload { rect: rect.clone(), rgba: rgba_region }
            })
            .collect();

        if tx_send.send(TxCommand::Frame(regions)).is_err() {
            warn!("TX thread exited unexpectedly — stopping capture loop");
            break;
        }

        frame_counter += 1;
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    Ok(())
}

/// Serial TX thread: receives pre-extracted frame regions and writes them to the display.
fn run_tx_loop(
    mut display: Box<dyn LcdDisplay>,
    rx: mpsc::Receiver<TxCommand>,
    write_reconnected: Arc<AtomicBool>,
    mut brightness: u8,
    mut orientation: Orientation,
) {
    use std::sync::mpsc::RecvTimeoutError;
    let mut health_counter: u32 = 0;

    loop {
        let cmd = match rx.recv_timeout(std::time::Duration::from_secs(2)) {
            Ok(cmd) => cmd,
            Err(RecvTimeoutError::Timeout) => {
                if display.check_port_health() {
                    info!("TX: USB reconnect via idle health check — restoring settings");
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    if let Err(e) = display.set_brightness(brightness) {
                        warn!("TX: failed to restore brightness: {}", e);
                    }
                    if let Err(e) = display.set_orientation(orientation) {
                        warn!("TX: failed to restore orientation: {}", e);
                    }
                    write_reconnected.store(true, Ordering::Relaxed);
                }
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => {
                info!("TX: channel disconnected — exiting TX loop");
                break;
            }
        };

        match cmd {
            TxCommand::Reinit { brightness: b, orientation: o } => {
                brightness = b;
                orientation = o;
                info!(
                    "TX: applying display settings (brightness={}, orientation={:?})",
                    brightness, orientation
                );
                if let Err(e) = display.set_brightness(brightness) {
                    warn!("TX: failed to set brightness: {}", e);
                }
                if let Err(e) = display.set_orientation(orientation) {
                    warn!("TX: failed to set orientation: {}", e);
                }
                display.take_reconnected();
            }

            TxCommand::Frame(regions) => {
                for region in &regions {
                    if let Err(e) = display.display_rgba_image(
                        &region.rgba,
                        region.rect.x,
                        region.rect.y,
                        region.rect.w,
                        region.rect.h,
                    ) {
                        warn!("TX: display update failed for {:?}: {}", region.rect, e);
                        break;
                    }
                }

                if display.take_reconnected() {
                    info!("TX: write-error reconnect detected — restoring settings");
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    if let Err(e) = display.set_brightness(brightness) {
                        warn!("TX: failed to restore brightness: {}", e);
                    }
                    if let Err(e) = display.set_orientation(orientation) {
                        warn!("TX: failed to restore orientation: {}", e);
                    }
                    write_reconnected.store(true, Ordering::Relaxed);
                }

                health_counter += 1;
                if health_counter >= 10 {
                    health_counter = 0;
                    if display.check_port_health() {
                        info!("TX: USB reconnect via periodic health check");
                        std::thread::sleep(std::time::Duration::from_secs(2));
                        if let Err(e) = display.set_brightness(brightness) {
                            warn!("TX: failed to restore brightness: {}", e);
                        }
                        if let Err(e) = display.set_orientation(orientation) {
                            warn!("TX: failed to restore orientation: {}", e);
                        }
                        write_reconnected.store(true, Ordering::Relaxed);
                    }
                }
            }
        }
    }
}

// --- Tauri Commands ---

pub type SharedConfig = std::sync::Arc<std::sync::Mutex<config::AppConfig>>;

/// Client-safe config that omits sensitive fields like API keys.
#[derive(Serialize)]
pub struct ClientConfig {
    pub config: ClientGeneralConfig,
    pub display: config::DisplayConfig,
}

#[derive(Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct ClientGeneralConfig {
    pub com_port: String,
    pub theme: String,
    pub hw_sensors: String,
    pub eth: String,
    pub wlo: String,
    pub cpu_fan: String,
    pub ping: String,
    pub weather_api_key_set: bool,
    pub weather_latitude: f64,
    pub weather_longitude: f64,
    pub weather_units: String,
    pub weather_language: String,
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

    // Save to file in data dir
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
        win.eval("window.location.reload();")
            .map_err(|e| e.to_string())?;
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
async fn open_editor(app: tauri::AppHandle) -> Result<(), String> {
    let handle = app.clone();
    app.run_on_main_thread(move || {
        tray::open_editor_window(&handle);
    })
    .map_err(|e| format!("Failed to open editor: {}", e))
}

/// Initialize logger that writes to a file in the data dir.
fn init_file_logger() {
    use std::io::Write;

    let log_path = config::AppConfig::data_dir().join("turing-smart-screen.log");

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
                    let days = secs / 86400;
                    let time_of_day = secs % 86400;
                    let hours = time_of_day / 3600;
                    let minutes = (time_of_day % 3600) / 60;
                    let seconds = time_of_day % 60;
                    let (year, month, day) = epoch_days_to_date(days);
                    let msg = format!(
                        "[{:04}-{:02}-{:02} {:02}:{:02}:{:02} {} {}] {}\n",
                        year,
                        month,
                        day,
                        hours,
                        minutes,
                        seconds,
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
            env_logger::Builder::new()
                .filter_level(log::LevelFilter::Info)
                .init();
        }
    }
}

/// Convert days since Unix epoch to (year, month, day). Simple algorithm, valid 1970-2099.
pub fn epoch_days_to_date(days: u64) -> (u64, u64, u64) {
    let mut y = 1970;
    let mut remaining = days;
    loop {
        let days_in_year = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
            366
        } else {
            365
        };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let month_days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_epoch_days_to_date_known_dates() {
        // 1970-01-01 is epoch day 0
        assert_eq!(epoch_days_to_date(0), (1970, 1, 1));
        // 2000-01-01 is epoch day 10957
        assert_eq!(epoch_days_to_date(10957), (2000, 1, 1));
        // 2024-02-29 (leap day)
        // 1970 to 2024 is 54 years (with 13 leap years between 1972 and 2020: 72,76,80,84,88,92,96,00,04,08,12,16,20)
        // 54 * 365 + 13 + 31 (Jan) + 28 = 19710 + 13 + 59 = 19782
        assert_eq!(epoch_days_to_date(19782), (2024, 2, 29));
    }
}
