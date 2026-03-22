use sysinfo::System;

pub struct CpuData {
    pub usage: Option<f64>,
    pub frequency: Option<f64>,
}

/// Read CPU usage and frequency from sysinfo.
/// CPU temperature comes from LHM via the shared LhmSensorData state.
pub fn read_cpu(sys: &System) -> CpuData {
    let usage = Some(sys.global_cpu_usage() as f64);

    let cpus = sys.cpus();
    let frequency = if !cpus.is_empty() {
        let avg: f64 = cpus.iter().map(|c| c.frequency() as f64).sum::<f64>() / cpus.len() as f64;
        Some(avg)
    } else {
        None
    };

    CpuData { usage, frequency }
}
