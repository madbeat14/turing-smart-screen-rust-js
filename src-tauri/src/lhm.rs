/// LibreHardwareMonitor headless service manager.
///
/// Launches LhmService.exe as a hidden background process. The service outputs
/// sensor data as JSON lines to stdout every second. A reader thread parses
/// them into `LhmSensorData` shared via `Arc<Mutex<>>`.
///
/// Architecture:
///   LhmService.exe (C#) → stdout JSON lines → reader thread → Arc<Mutex<LhmSensorData>>
///   sensor_loop reads from the shared state and merges with sysinfo data.
use log::{error, info, warn};
use serde::Deserialize;
use std::io::BufRead;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

static LHM_PROCESS: Mutex<Option<Child>> = Mutex::new(None);

/// Sensor data parsed from LhmService JSON output.
/// All fields are populated from LhmService; some may not be used by this app yet.
#[derive(Debug, Clone, Default, Deserialize)]
#[allow(dead_code)]
pub struct LhmSensorData {
    pub cpu_temp: Option<f64>,
    pub cpu_usage: Option<f64>,
    pub cpu_freq: Option<f64>,
    pub gpu_temp: Option<f64>,
    pub gpu_usage: Option<f64>,
    pub gpu_freq: Option<f64>,
    pub gpu_mem_used: Option<f64>,
    pub gpu_mem_total: Option<f64>,
}

/// Start LhmService and return shared sensor state.
/// The returned Arc is updated every ~1 second by a background reader thread.
pub fn start_lhm(resource_dir: &std::path::Path) -> Arc<Mutex<LhmSensorData>> {
    let shared = Arc::new(Mutex::new(LhmSensorData::default()));

    let lhm_dir = resource_dir.join("external").join("lhm");
    let service_exe = lhm_dir.join("LhmService.exe");

    info!("LHM resource_dir: {:?}", resource_dir);
    info!("LHM service_exe: {:?}", service_exe);

    if !service_exe.exists() {
        warn!("LhmService.exe not found at {:?}", service_exe);
        return shared;
    }

    // Always kill any stale LhmService processes before launching our own.
    // A leftover instance from a previous run (possibly without admin) would
    // cause is_lhm_running() to return true and skip launching, leaving us
    // with no stdout pipe and no sensor data.
    kill_existing_lhm();

    launch_and_read(service_exe, shared.clone());
    shared
}

/// Kill any existing LhmService.exe processes.
fn kill_existing_lhm() {
    #[cfg(target_os = "windows")]
    {
        if is_lhm_running() {
            info!("Killing existing LhmService.exe before launching our own");
            let _ = Command::new("taskkill")
                .args(["/IM", "LhmService.exe", "/F"])
                .creation_flags(CREATE_NO_WINDOW)
                .output();
            // Brief wait for the process to fully exit
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    }
}

/// Launch LhmService with stdout piped, spawn reader thread.
fn launch_and_read(service_exe: std::path::PathBuf, shared: Arc<Mutex<LhmSensorData>>) {
    #[cfg(target_os = "windows")]
    {
        info!("Starting LHM sensor service from {:?}", service_exe);

        let working_dir = service_exe.parent().unwrap_or(std::path::Path::new("."));

        match Command::new(&service_exe)
            .current_dir(working_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
        {
            Ok(mut child) => {
                info!("LHM service started (PID: {})", child.id());

                // Take stdout and stderr before storing child
                let stdout = child.stdout.take();
                let stderr = child.stderr.take();

                if let Ok(mut guard) = LHM_PROCESS.lock() {
                    *guard = Some(child);
                }

                // Spawn stderr logger thread
                if let Some(stderr) = stderr {
                    if let Err(e) =
                        std::thread::Builder::new()
                            .name("lhm-stderr".into())
                            .spawn(move || {
                                let reader = std::io::BufReader::new(stderr);
                                for line in reader.lines().map_while(Result::ok) {
                                    if !line.is_empty() {
                                        info!("LhmService stderr: {}", line);
                                    }
                                }
                            })
                    {
                        warn!("Failed to spawn LHM stderr reader thread: {}", e);
                    }
                }

                // Spawn stdout reader thread for sensor data
                if let Some(stdout) = stdout {
                    let reader_shared = shared.clone();
                    if let Err(e) =
                        std::thread::Builder::new()
                            .name("lhm-reader".into())
                            .spawn(move || {
                                read_sensor_output(stdout, reader_shared);
                            })
                    {
                        error!("Failed to spawn LHM reader thread: {}", e);
                    }
                }
            }
            Err(e) => {
                error!(
                    "Failed to start LHM service: {} (os error: {:?})",
                    e,
                    e.raw_os_error()
                );
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (service_exe, shared);
        warn!("LHM is only supported on Windows");
    }
}

/// Read JSON lines from LhmService stdout, update shared state.
fn read_sensor_output(stdout: std::process::ChildStdout, shared: Arc<Mutex<LhmSensorData>>) {
    let reader = std::io::BufReader::new(stdout);
    let mut count = 0u64;

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                error!("LHM stdout read error (after {} lines): {}", count, e);
                break;
            }
        };

        if line.is_empty() {
            continue;
        }

        count += 1;
        match serde_json::from_str::<LhmSensorData>(&line) {
            Ok(data) => {
                if count <= 3 {
                    info!("LHM sensor data #{}: {:?}", count, data);
                }
                if let Ok(mut guard) = shared.lock() {
                    *guard = data;
                }
            }
            Err(e) => {
                warn!("LHM JSON parse error: {} — line: {}", e, line);
            }
        }
    }

    info!(
        "LHM sensor reader thread exiting (read {} lines total)",
        count
    );
}

/// Check if LhmService is already running.
fn is_lhm_running() -> bool {
    #[cfg(target_os = "windows")]
    {
        match Command::new("tasklist")
            .args(["/FI", "IMAGENAME eq LhmService.exe", "/NH"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
        {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                stdout.contains("LhmService.exe")
            }
            Err(_) => false,
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

/// Kill the LHM service. Called on app exit.
pub fn stop_lhm() {
    let mut guard = match LHM_PROCESS.lock() {
        Ok(g) => g,
        Err(e) => e.into_inner(), // recover from poisoned mutex
    };
    if let Some(mut child) = guard.take() {
        info!("Stopping LHM service (PID: {})", child.id());
        let _ = child.kill();
        let _ = child.wait();
        return;
    }

    #[cfg(target_os = "windows")]
    {
        info!("Stopping LHM service via taskkill");
        let _ = Command::new("taskkill")
            .args(["/IM", "LhmService.exe", "/F"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
    }
}
