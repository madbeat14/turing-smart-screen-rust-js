// Turing Smart Screen — Rust + JS Rewrite
// Entry point: Tauri app setup, sensor loop, display loop

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod display;
mod lhm;
mod screenshot;
mod sensors;
mod startup;
mod tray;

use display::diff::FrameDiffer;
use display::rgb565::rgba_to_rgb565_le;
use display::{Orientation, create_display};
use log::{error, info, warn};
use std::sync::mpsc;
use tauri::Manager;

/// Wrapper so we can store the restart sender in Tauri managed state.
pub struct RestartSender(pub mpsc::Sender<()>);

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
            get_run_on_startup,
            set_run_on_startup,
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

            // Move monitor window off-screen at startup — the virtual monitor
            // is hidden by default. User clicks "Show Monitor" in tray to see it.
            // We keep it "visible" (not hidden) so WebView2 continues rendering
            // content for the physical display screenshot capture.
            if let Some(monitor_win) = app_handle.get_webview_window("monitor") {
                let _ = monitor_win.set_position(tauri::PhysicalPosition::new(-9999, -9999));
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
        // Re-initialize after reset
        display.initialize()?;
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

    // Get the webview window handle
    let window = app_handle
        .get_webview_window("monitor")
        .ok_or_else(|| anyhow::anyhow!("Monitor window not found"))?;

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
        }

        // Capture webview screenshot
        let rgba = match screenshot::capture_webview_screenshot(
            &window,
            screen_w as u32,
            screen_h as u32,
        ) {
            Ok(pixels) => pixels,
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
            std::thread::sleep(std::time::Duration::from_millis(100));
            continue;
        }

        // Send only changed regions
        for rect in &dirty_rects {
            // Extract the RGBA sub-region for this dirty rect
            let stride = (screen_w as usize) * 4;
            let mut region_rgba =
                Vec::with_capacity((rect.w as usize) * (rect.h as usize) * 4);

            for row in 0..rect.h as usize {
                let y_offset = (rect.y as usize + row) * stride;
                let x_offset = (rect.x as usize) * 4;
                let start = y_offset + x_offset;
                let end = start + (rect.w as usize) * 4;
                if end <= rgba.len() {
                    region_rgba.extend_from_slice(&rgba[start..end]);
                }
            }

            if let Err(e) =
                display.display_rgba_image(&region_rgba, rect.x, rect.y, rect.w, rect.h)
            {
                warn!("Display update failed for rect {:?}: {}", rect, e);
            }
        }

        // Adaptive sleep: shorter when many changes, longer when few
        let sleep_ms = if dirty_rects.len() > 5 { 200 } else { 500 };
        std::thread::sleep(std::time::Duration::from_millis(sleep_ms));
    }
}

// --- Tauri Commands ---

type SharedConfig = std::sync::Arc<std::sync::Mutex<config::AppConfig>>;

#[tauri::command]
fn get_config(config: tauri::State<SharedConfig>) -> Result<config::AppConfig, String> {
    let cfg = config.lock().map_err(|e| e.to_string())?;
    Ok(cfg.clone())
}

#[tauri::command]
fn save_config(
    config: tauri::State<SharedConfig>,
    new_config: config::AppConfig,
) -> Result<(), String> {
    // Save to file
    let yaml = serde_yaml::to_string(&new_config).map_err(|e| e.to_string())?;
    std::fs::write("config.yaml", yaml).map_err(|e| e.to_string())?;

    // Update in-memory config
    let mut cfg = config.lock().map_err(|e| e.to_string())?;
    *cfg = new_config;

    info!("Configuration saved");
    Ok(())
}

#[tauri::command]
fn restart_display(sender: tauri::State<RestartSender>) -> Result<(), String> {
    sender.0.send(()).map_err(|e| e.to_string())?;
    info!("Display restart signal sent");
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
fn set_run_on_startup(enable: bool) {
    startup::set_run_on_startup(enable);
}

/// Initialize logger that writes to a file next to the executable.
/// This ensures we can read logs even when running as admin (no console).
fn init_file_logger() {
    use std::io::Write;

    let log_path = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("turing-smart-screen.log")))
        .unwrap_or_else(|| std::path::PathBuf::from("turing-smart-screen.log"));

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
                        .unwrap_or_default()
                        .as_secs();
                    let msg = format!(
                        "[{} {} {}] {}\n",
                        now,
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
