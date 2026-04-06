use sysinfo::{Disks, System};

pub struct DiskData {
    pub used: Option<f64>,
    pub total: Option<f64>,
}

pub fn create_disks() -> Disks {
    Disks::new_with_refreshed_list()
}

pub fn read_disks(_sys: &System, disks: &mut Disks) -> DiskData {
    disks.refresh(false);

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
