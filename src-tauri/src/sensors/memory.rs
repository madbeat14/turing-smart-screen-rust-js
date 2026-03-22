use sysinfo::System;

pub struct MemoryData {
    pub used: Option<f64>,
    pub total: Option<f64>,
}

pub fn read_memory(sys: &System) -> MemoryData {
    MemoryData {
        used: Some(sys.used_memory() as f64),
        total: Some(sys.total_memory() as f64),
    }
}
