use crate::db::Database;
use crate::files::dir_size_mb;
use crate::models::HostSystemInfo;
use crate::process_manager::ProcessManager;
use chrono::Utc;
use rusqlite::params;
use std::path::PathBuf;
use std::sync::Arc;
use sysinfo::{Disks, Networks, Pid, System};

pub fn get_host_system_info() -> HostSystemInfo {
    let mut sys = System::new_all();
    sys.refresh_cpu_usage();
    sys.refresh_memory();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[Pid::from_u32(std::process::id())]));

    let disks = Disks::new_with_refreshed_list();
    let mut total_disk: u64 = 0;
    let mut avail_disk: u64 = 0;
    for d in disks.list() {
        if d.mount_point() == std::path::Path::new("/") {
            total_disk = d.total_space();
            avail_disk = d.available_space();
            break;
        }
    }
    if total_disk == 0 {
        for d in disks.list() {
            total_disk += d.total_space();
            avail_disk += d.available_space();
        }
    }

    let my_pid = Pid::from_u32(std::process::id());
    let panel_memory_mb = sys
        .process(my_pid)
        .map(|p| (p.memory() as f64) / (1024.0 * 1024.0))
        .unwrap_or(12.0);

    let cpu_brand = sys
        .cpus()
        .first()
        .map(|c| c.brand().to_string())
        .unwrap_or_else(|| "CPU".into());

    HostSystemInfo {
        hostname: System::host_name().unwrap_or_else(|| "localhost".into()),
        os_name: System::name().unwrap_or_else(|| "Linux".into()),
        os_version: System::os_version().unwrap_or_default(),
        kernel_version: System::kernel_version().unwrap_or_default(),
        cpu_brand,
        cpu_cores: sys.cpus().len(),
        cpu_usage_percent: sys.global_cpu_usage(),
        total_memory_mb: sys.total_memory() / (1024 * 1024),
        used_memory_mb: sys.used_memory() / (1024 * 1024),
        total_disk_gb: (total_disk as f64) / (1024.0 * 1024.0 * 1024.0),
        used_disk_gb: ((total_disk.saturating_sub(avail_disk)) as f64) / (1024.0 * 1024.0 * 1024.0),
        uptime_secs: System::uptime(),
        panel_memory_mb,
        panel_version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

pub fn spawn_telemetry_collector(db: Database, pm: Arc<ProcessManager>) {
    tokio::spawn(async move {
        let mut sys = System::new_all();
        let mut disks = Disks::new_with_refreshed_list();
        let mut networks = Networks::new_with_refreshed_list();
        let mut tick: u64 = 0;

        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            tick += 1;

            sys.refresh_cpu_usage();
            sys.refresh_memory();
            sys.refresh_processes(sysinfo::ProcessesToUpdate::All);
            disks.refresh();
            networks.refresh();

            let now = Utc::now().timestamp();
            let host_cpu = sys.global_cpu_usage() as f64;
            let host_mem_mb = (sys.used_memory() as f64) / (1024.0 * 1024.0);

            let mut total_disk: u64 = 0;
            let mut avail_disk: u64 = 0;
            for d in disks.list() {
                if d.mount_point() == std::path::Path::new("/") {
                    total_disk = d.total_space();
                    avail_disk = d.available_space();
                    break;
                }
            }
            let host_disk_mb = (total_disk.saturating_sub(avail_disk) as f64) / (1024.0 * 1024.0);

            let mut net_rx_kb = 0.0;
            let mut net_tx_kb = 0.0;
            for (_name, data) in networks.list() {
                net_rx_kb += (data.received() as f64) / 1024.0;
                net_tx_kb += (data.transmitted() as f64) / 1024.0;
            }

            let instances = pm.list_instances();
            let mut total_players: i64 = 0;
            let mut per_inst_records: Vec<(String, f64, f64, f64, i64)> = Vec::new();

            for inst in &instances {
                let mut inst_cpu = 0.0f32;
                let mut inst_mem = 0.0f64;
                if let Some(pid_u32) = inst.pid {
                    if let Some(proc) = sys.process(Pid::from_u32(pid_u32)) {
                        inst_cpu = proc.cpu_usage();
                        inst_mem = (proc.memory() as f64) / (1024.0 * 1024.0);
                    }
                }
                // Recalculate disk size every 12 ticks (60s) to keep CPU overhead near zero
                let inst_disk = if tick % 12 == 1 {
                    dir_size_mb(&PathBuf::from(&inst.work_dir))
                } else {
                    inst.disk_mb
                };

                {
                    let mut map = pm.runtimes.lock().unwrap();
                    if let Some(rt) = map.get_mut(&inst.id) {
                        rt.cpu_percent = inst_cpu;
                        rt.memory_mb = inst_mem;
                        rt.disk_mb = inst_disk;
                    }
                }

                let players_cnt = inst.active_players.len() as i64;
                total_players += players_cnt;
                per_inst_records.push((inst.id.clone(), inst_cpu as f64, inst_mem, inst_disk, players_cnt));
            }

            // Persist telemetry batch to SQLite
            {
                let conn = db.conn();
                let _ = conn.execute(
                    "INSERT INTO metrics_history (timestamp, instance_id, cpu_percent, memory_mb, disk_mb, net_rx_kb, net_tx_kb, players_online)
                     VALUES (?1, 'host', ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![now, host_cpu, host_mem_mb, host_disk_mb, net_rx_kb, net_tx_kb, total_players],
                );
                for (iid, icpu, imem, idisk, iplayers) in per_inst_records {
                    let _ = conn.execute(
                        "INSERT INTO metrics_history (timestamp, instance_id, cpu_percent, memory_mb, disk_mb, net_rx_kb, net_tx_kb, players_online)
                         VALUES (?1, ?2, ?3, ?4, ?5, 0, 0, ?6)",
                        params![now, iid, icpu, imem, idisk, iplayers],
                    );
                }

                // Prune old telemetry once per hour
                if tick % 720 == 0 {
                    let retention_days = db.get_panel_settings().metrics_retention_days.max(1);
                    let cutoff = now - (retention_days * 86400);
                    let _ = conn.execute("DELETE FROM metrics_history WHERE timestamp < ?1", params![cutoff]);
                }
            }
        }
    });
}
