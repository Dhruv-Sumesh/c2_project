use sysinfo::{System, Disks};

pub struct Metrics {
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub disk_usage: f64,
}

pub fn get_os_name() -> String {
    System::name().unwrap_or_else(|| "Unknown".to_string())
}

pub fn get_hostname() -> String {
    System::host_name().unwrap_or_else(|| "Unknown".to_string())
}

pub fn get_system_metrics(sys: &mut System) -> Metrics {
    sys.refresh_cpu();
    sys.refresh_memory();
    
    let cpu_usage = sys.global_cpu_info().cpu_usage() as f64;
    
    let total_mem = sys.total_memory() as f64;
    let used_mem = sys.used_memory() as f64;
    let memory_usage = if total_mem > 0.0 {
        (used_mem / total_mem) * 100.0
    } else {
        0.0
    };
    
    let disks = Disks::new_with_refreshed_list();
    let mut total_disk = 0u64;
    let mut available_disk = 0u64;
    for disk in &disks {
        total_disk += disk.total_space();
        available_disk += disk.available_space();
    }
    let disk_usage = if total_disk > 0 {
        ((total_disk - available_disk) as f64 / total_disk as f64) * 100.0
    } else {
        0.0
    };
    
    Metrics {
        cpu_usage,
        memory_usage,
        disk_usage,
    }
}
