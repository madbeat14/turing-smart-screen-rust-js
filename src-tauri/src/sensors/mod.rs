pub mod cpu;
pub mod disk;
pub mod gpu;
pub mod memory;
pub mod network;

use crate::lhm::LhmSensorData;
use log::warn;
use serde::Serialize;
use std::sync::{Arc, Mutex};
use sysinfo::System;
use tauri::{AppHandle, Emitter};

/// All sensor readings, emitted to the JS frontend via Tauri events.
/// Fields are Option so missing sensors degrade gracefully.
#[derive(Debug, Clone, Serialize)]
pub struct SensorData {
    // CPU
    pub cpu_usage: Option<f64>,
    pub cpu_temp: Option<f64>,
    pub cpu_freq: Option<f64>,

    // GPU
    pub gpu_usage: Option<f64>,
    pub gpu_temp: Option<f64>,
    pub gpu_mem_used: Option<f64>,
    pub gpu_mem_total: Option<f64>,
    pub gpu_freq: Option<f64>,

    // Memory
    pub ram_used: Option<f64>,
    pub ram_total: Option<f64>,
    pub ram_usage: Option<f64>,

    // Disk
    pub disk_used: Option<f64>,
    pub disk_total: Option<f64>,
    pub disk_usage: Option<f64>,

    // Network
    pub net_upload: Option<f64>,
    pub net_download: Option<f64>,
}

/// Main sensor loop — runs every 500ms, emits sensor data to the webview.
///
/// Uses sysinfo for CPU usage/freq, memory, disk, network.
/// Uses LHM shared state (from LhmService.exe stdout pipe) for CPU temp,
/// GPU temp, GPU usage, GPU freq, GPU memory.
pub fn sensor_loop(app_handle: AppHandle, lhm_state: Arc<Mutex<LhmSensorData>>) {
    let mut sys = System::new_all();
    let mut disks = disk::create_disks();
    let mut networks = network::create_networks();

    // Need an initial refresh + short delay for CPU usage to be meaningful
    sys.refresh_all();
    std::thread::sleep(std::time::Duration::from_millis(500));

    let mut prev_net = network::NetworkSnapshot::capture(&sys, &mut networks);

    loop {
        sys.refresh_all();

        let cpu_data = cpu::read_cpu(&sys);
        let mem_data = memory::read_memory(&sys);
        let disk_data = disk::read_disks(&sys, &mut disks);

        let curr_net = network::NetworkSnapshot::capture(&sys, &mut networks);
        let net_data = network::compute_rates(&prev_net, &curr_net, 0.5);
        prev_net = curr_net;

        // Calculate percentages for RAM and Disk
        let ram_usage = match (mem_data.used, mem_data.total) {
            (Some(u), Some(t)) if t > 0.0 => Some((u / t) * 100.0),
            _ => None,
        };

        let disk_usage = match (disk_data.used, disk_data.total) {
            (Some(u), Some(t)) if t > 0.0 => Some((u / t) * 100.0),
            _ => None,
        };

        // Read LHM sensor data (updated by background reader thread)
        let lhm = match lhm_state.lock() {
            Ok(guard) => guard.clone(),
            Err(e) => {
                warn!("LHM mutex poisoned, using stale data: {}", e);
                e.into_inner().clone()
            }
        };

        let data = SensorData {
            // CPU: usage and freq from sysinfo, temp from LHM
            cpu_usage: cpu_data.usage,
            cpu_temp: lhm.cpu_temp,
            cpu_freq: cpu_data.frequency,

            // GPU: all from LHM (sysinfo doesn't provide GPU data)
            gpu_usage: lhm.gpu_usage,
            gpu_temp: lhm.gpu_temp,
            gpu_mem_used: lhm.gpu_mem_used,
            gpu_mem_total: lhm.gpu_mem_total,
            gpu_freq: lhm.gpu_freq,

            ram_used: mem_data.used,
            ram_total: mem_data.total,
            ram_usage,

            disk_used: disk_data.used,
            disk_total: disk_data.total,
            disk_usage,

            net_upload: net_data.upload_rate,
            net_download: net_data.download_rate,
        };

        if let Err(e) = app_handle.emit("sensor-update", &data) {
            warn!("Failed to emit sensor data: {}", e);
        }

        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}
