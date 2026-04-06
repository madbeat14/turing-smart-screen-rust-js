use sysinfo::{Networks, System};

pub struct NetworkData {
    pub upload_rate: Option<f64>,
    pub download_rate: Option<f64>,
}

/// Snapshot of cumulative network bytes at a point in time
pub struct NetworkSnapshot {
    pub total_received: u64,
    pub total_transmitted: u64,
}

impl NetworkSnapshot {
    pub fn capture(_sys: &System, networks: &mut Networks) -> Self {
        networks.refresh(false);
        let mut total_rx: u64 = 0;
        let mut total_tx: u64 = 0;

        for (_name, data) in networks.iter() {
            total_rx += data.total_received();
            total_tx += data.total_transmitted();
        }

        Self {
            total_received: total_rx,
            total_transmitted: total_tx,
        }
    }
}

pub fn create_networks() -> Networks {
    Networks::new_with_refreshed_list()
}

/// Compute upload/download rates in bytes/sec from two snapshots
pub fn compute_rates(prev: &NetworkSnapshot, curr: &NetworkSnapshot, interval_secs: f64) -> NetworkData {
    if interval_secs <= 0.0 {
        return NetworkData {
            upload_rate: None,
            download_rate: None,
        };
    }

    let rx_delta = curr.total_received.saturating_sub(prev.total_received) as f64;
    let tx_delta = curr.total_transmitted.saturating_sub(prev.total_transmitted) as f64;

    NetworkData {
        upload_rate: Some(tx_delta / interval_secs),
        download_rate: Some(rx_delta / interval_secs),
    }
}
