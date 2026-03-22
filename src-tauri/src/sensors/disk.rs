use sysinfo::{Disks, System};

pub struct DiskData {
    pub used: Option<f64>,
    pub total: Option<f64>,
}

pub fn read_disks(_sys: &System) -> DiskData {
    let disks = Disks::new_with_refreshed_list();

    let mut total_space: u64 = 0;
    let mut available_space: u64 = 0;

    for disk in disks.iter() {
        total_space += disk.total_space();
        available_space += disk.available_space();
    }

    if total_space > 0 {
        DiskData {
            used: Some((total_space - available_space) as f64),
            total: Some(total_space as f64),
        }
    } else {
        DiskData {
            used: None,
            total: None,
        }
    }
}
