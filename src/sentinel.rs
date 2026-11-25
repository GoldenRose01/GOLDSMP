use crate::analytics::get_host_system_info;
use crate::db::Database;
use crate::process_manager::ProcessManager;
use crate::webhooks;
use chrono::Utc;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::net::TcpStream;

const MAX_SENTINEL_EVENTS: usize = 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentinelConfig {
    pub enabled: bool,
    pub check_interval_secs: u64,
    pub auto_heal_crashed: bool,
    pub auto_heal_unresponsive_port: bool,
    pub cpu_warn_threshold: f32,
    pub mem_warn_threshold: f32,
    pub disk_warn_threshold: f32,
    pub push_endpoint: String,
    pub push_token: String,
    pub push_interval_secs: u64,
}

impl Default for SentinelConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            check_interval_secs: 15,
            auto_heal_crashed: true,
            auto_heal_unresponsive_port: false,
            cpu_warn_threshold: 88.0,
            mem_warn_threshold: 88.0,
            disk_warn_threshold: 90.0,
            push_endpoint: std::env::var("GOLDSMP_SENTINEL_ENDPOINT").unwrap_or_default(),
            push_token: std::env::var("GOLDSMP_SENTINEL_TOKEN").unwrap_or_default(),
            push_interval_secs: 30,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentinelInstanceHealth {
    pub instance_id: String,
    pub instance_name: String,
    pub template_id: String,
    pub port: u16,
    pub process_status: String,
    pub health_state: String, // "healthy" | "starting" | "unhealthy" | "degraded" | "stopped" | "healing"
    pub port_reachable: bool,
    pub latency_ms: Option<u64>,
    pub cpu_percent: f32,
    pub memory_mb: f64,
    pub max_memory_mb: u64,
    pub memory_usage_pct: f32,
    pub consecutive_failures: u32,
    pub auto_heals_count: u64,
    pub last_checked: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentinelEvent {
    pub timestamp: String,
    pub level: String, // "info" | "warn" | "critical" | "heal"
    pub target: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentinelStatus {
    pub config: SentinelConfig,
    pub agent_state: String, // "active" | "paused" | "warning" | "critical"
    pub last_heartbeat: Option<String>,
    pub total_checks: u64,
    pub total_auto_heals: u64,
    pub active_warnings: Vec<String>,
    pub host_cpu_pct: f32,
    pub host_mem_pct: f32,
    pub host_disk_pct: f32,
    pub instances: Vec<SentinelInstanceHealth>,
    pub recent_events: Vec<SentinelEvent>,
    pub last_push_status: Option<String>,
}

struct SentinelInternalState {
    agent_state: String,
    last_heartbeat: Option<String>,
    total_checks: u64,
    total_auto_heals: u64,
    active_warnings: Vec<String>,
    host_cpu_pct: f32,
    host_mem_pct: f32,
    host_disk_pct: f32,
    instances: Vec<SentinelInstanceHealth>,
    failure_counts: std::collections::HashMap<String, u32>,
    heal_counts: std::collections::HashMap<String, u64>,
    events: VecDeque<SentinelEvent>,
    last_push_status: Option<String>,
}

impl Default for SentinelInternalState {
    fn default() -> Self {
        let mut events = VecDeque::with_capacity(MAX_SENTINEL_EVENTS);
        events.push_back(SentinelEvent {
            timestamp: Utc::now().to_rfc3339(),
            level: "info".into(),
            target: "sentinel".into(),
            message: "Coolify Sentinel Watchdog inizializzato e in ascolto sui processi nativi.".into(),
        });
        Self {
            agent_state: "active".into(),
            last_heartbeat: None,
            total_checks: 0,
            total_auto_heals: 0,
            active_warnings: Vec::new(),
            host_cpu_pct: 0.0,
            host_mem_pct: 0.0,
            host_disk_pct: 0.0,
            instances: Vec::new(),
            failure_counts: std::collections::HashMap::new(),
            heal_counts: std::collections::HashMap::new(),
            events,
            last_push_status: None,
        }
    }
}

pub struct SentinelMonitor {
    db: Database,
    pm: Arc<ProcessManager>,
    http: reqwest::Client,
    state: Mutex<SentinelInternalState>,
}

impl SentinelMonitor {
    pub fn new(db: Database, pm: Arc<ProcessManager>, http: reqwest::Client) -> Self {
        Self {
            db,
            pm,
            http,
            state: Mutex::new(SentinelInternalState::default()),
        }
    }

    pub fn load_config(&self) -> SentinelConfig {
        let conn = self.db.conn();
        conn.query_row(
            "SELECT value FROM kv_settings WHERE key = 'sentinel_config'",
            [],
            |r| r.get::<_, String>(0),
        )
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
    }

    pub fn save_config(&self, mut cfg: SentinelConfig) -> Result<SentinelStatus, String> {
        cfg.check_interval_secs = cfg.check_interval_secs.clamp(5, 600);
        cfg.push_interval_secs = cfg.push_interval_secs.clamp(10, 3600);
        cfg.cpu_warn_threshold = cfg.cpu_warn_threshold.clamp(20.0, 100.0);
        cfg.mem_warn_threshold = cfg.mem_warn_threshold.clamp(20.0, 100.0);
        cfg.disk_warn_threshold = cfg.disk_warn_threshold.clamp(20.0, 100.0);

        {
            let conn = self.db.conn();
            conn.execute(
                "INSERT OR REPLACE INTO kv_settings (key, value) VALUES ('sentinel_config', ?1)",
                params![serde_json::to_string(&cfg).unwrap()],
            )
            .map_err(|e| e.to_string())?;
        }

        {
            let mut st = self.state.lock().unwrap();
            st.agent_state = if cfg.enabled { "active".into() } else { "paused".into() };
            Self::push_event_locked(
                &mut st,
                "info",
                "sentinel",
                format!(
                    "Configurazione Sentinel aggiornata (intervallo {}s, auto-heal={})",
                    cfg.check_interval_secs, cfg.auto_heal_crashed
                ),
            );
        }

        Ok(self.get_status())
    }

    fn push_event_locked(
        st: &mut SentinelInternalState,
        level: &str,
        target: &str,
        message: String,
    ) {
        if st.events.len() >= MAX_SENTINEL_EVENTS {
            st.events.pop_front();
        }
        st.events.push_back(SentinelEvent {
            timestamp: Utc::now().to_rfc3339(),
            level: level.to_string(),
            target: target.to_string(),
            message,
        });
    }

    pub fn get_status(&self) -> SentinelStatus {
        let cfg = self.load_config();
        let st = self.state.lock().unwrap();
        let mut recent_events: Vec<SentinelEvent> = st.events.iter().cloned().collect();
        recent_events.reverse();
        SentinelStatus {
            config: cfg,
            agent_state: st.agent_state.clone(),
            last_heartbeat: st.last_heartbeat.clone(),
            total_checks: st.total_checks,
            total_auto_heals: st.total_auto_heals,
            active_warnings: st.active_warnings.clone(),
            host_cpu_pct: st.host_cpu_pct,
            host_mem_pct: st.host_mem_pct,
            host_disk_pct: st.host_disk_pct,
            instances: st.instances.clone(),
            recent_events,
            last_push_status: st.last_push_status.clone(),
        }
    }

    /// Runs an immediate Sentinel heartbeat sweep across the host and all instances,
    /// performing TCP port liveness probes, threshold evaluations, and self-healing if enabled.
    pub async fn run_check_cycle(&self) -> SentinelStatus {
        let cfg = self.load_config();
        let host = get_host_system_info();
        let host_cpu = host.cpu_usage_percent;
        let host_mem = if host.total_memory_mb > 0 {
            ((host.used_memory_mb as f64 / host.total_memory_mb as f64) * 100.0) as f32
        } else {
            0.0
        };
        let host_disk = if host.total_disk_gb > 0.0 {
            ((host.used_disk_gb / host.total_disk_gb) * 100.0) as f32
        } else {
            0.0
        };

        let mut warnings: Vec<String> = Vec::new();
        if host_cpu >= cfg.cpu_warn_threshold {
            warnings.push(format!(
                "CPU Host sopra soglia Sentinel: {:.1}% >= {:.0}%",
                host_cpu, cfg.cpu_warn_threshold
            ));
        }
        if host_mem >= cfg.mem_warn_threshold {
            warnings.push(format!(
                "RAM Host sopra soglia Sentinel: {:.1}% >= {:.0}%",
                host_mem, cfg.mem_warn_threshold
            ));
        }
        if host_disk >= cfg.disk_warn_threshold {
            warnings.push(format!(
                "Disco Host sopra soglia Sentinel: {:.1}% >= {:.0}%",
                host_disk, cfg.disk_warn_threshold
            ));
        }

        let instances = self.pm.list_instances();
        let mut health_reports: Vec<SentinelInstanceHealth> = Vec::with_capacity(instances.len());
        let mut to_heal: Vec<(String, String, String)> = Vec::new();

        for inst in instances {
            let now_str = Utc::now().to_rfc3339();
            let mem_pct = if inst.max_memory_mb > 0 {
                ((inst.memory_mb / inst.max_memory_mb as f64) * 100.0) as f32
            } else {
                0.0
            };

            let (port_reachable, latency_ms) = if inst.status == "running" {
                probe_tcp_port(inst.port).await
            } else {
                (false, None)
            };

            let mut fail_cnt = {
                let st = self.state.lock().unwrap();
                *st.failure_counts.get(&inst.id).unwrap_or(&0)
            };
            let heal_cnt = {
                let st = self.state.lock().unwrap();
                *st.heal_counts.get(&inst.id).unwrap_or(&0)
            };

            let health_state = match inst.status.as_str() {
                "stopped" | "stopping" => {
                    fail_cnt = 0;
                    "stopped".to_string()
                }
                "starting" => "starting".to_string(),
                "crashed" => {
                    fail_cnt += 1;
                    warnings.push(format!("Istanza '{}' in stato CRASHED", inst.name));
                    if cfg.enabled && cfg.auto_heal_crashed {
                        to_heal.push((
                            inst.id.clone(),
                            inst.name.clone(),
                            "Processo in stato crashed".into(),
                        ));
                        "healing".to_string()
                    } else {
                        "unhealthy".to_string()
                    }
                }
                "running" => {
                    if port_reachable {
                        fail_cnt = 0;
                        if mem_pct >= cfg.mem_warn_threshold {
                            warnings.push(format!(
                                "Istanza '{}' usa {:.0}% della RAM allocata",
                                inst.name, mem_pct
                            ));
                            "degraded".to_string()
                        } else {
                            "healthy".to_string()
                        }
                    } else {
                        // Give starting servers at least 25s uptime before counting unresponsive port
                        if inst.uptime_secs > 25 {
                            fail_cnt += 1;
                        }
                        if fail_cnt >= 3 {
                            warnings.push(format!(
                                "Porta :{} di '{}' non risponde al probe TCP Sentinel ({} tentativi)",
                                inst.port, inst.name, fail_cnt
                            ));
                            if cfg.enabled && cfg.auto_heal_unresponsive_port {
                                to_heal.push((
                                    inst.id.clone(),
                                    inst.name.clone(),
                                    format!("Porta :{} non risponde dopo {} check", inst.port, fail_cnt),
                                ));
                                "healing".to_string()
                            } else {
                                "degraded".to_string()
                            }
                        } else {
                            "healthy".to_string()
                        }
                    }
                }
                _ => "stopped".to_string(),
            };

            {
                let mut st = self.state.lock().unwrap();
                st.failure_counts.insert(inst.id.clone(), fail_cnt);
            }

            health_reports.push(SentinelInstanceHealth {
                instance_id: inst.id,
                instance_name: inst.name,
                template_id: inst.template_id,
                port: inst.port,
                process_status: inst.status,
                health_state,
                port_reachable,
                latency_ms,
                cpu_percent: inst.cpu_percent,
                memory_mb: inst.memory_mb,
                max_memory_mb: inst.max_memory_mb,
                memory_usage_pct: mem_pct,
                consecutive_failures: fail_cnt,
                auto_heals_count: heal_cnt,
                last_checked: now_str,
            });
        }

        // Execute self-healing restarts outside the lock
        for (iid, iname, reason) in to_heal {
            let res = self.pm.restart_instance(&iid).await;
            let mut st = self.state.lock().unwrap();
            if res.is_ok() {
                st.total_auto_heals += 1;
                let entry = st.heal_counts.entry(iid.clone()).or_insert(0);
                *entry += 1;
                st.failure_counts.insert(iid.clone(), 0);
                Self::push_event_locked(
                    &mut st,
                    "heal",
                    &iname,
                    format!("Auto-Heal Sentinel completato: riavviata istanza ({})", reason),
                );
                self.db.log_audit(
                    "sentinel",
                    "SENTINEL_AUTO_HEAL",
                    &iname,
                    &format!("Riavvio automatico eseguito da Sentinel: {}", reason),
                    "sentinel",
                );
                webhooks::dispatch_event(
                    self.db.clone(),
                    self.http.clone(),
                    "sentinel_auto_heal".into(),
                    iid,
                    iname,
                    format!("Coolify Sentinel ha riavviato automaticamente l'istanza ({})", reason),
                );
            } else if let Err(e) = res {
                Self::push_event_locked(
                    &mut st,
                    "critical",
                    &iname,
                    format!("Auto-Heal fallito: {}", e),
                );
            }
        }

        // Optional Coolify Sentinel external push
        let mut push_res_str: Option<String> = None;
        if cfg.enabled && !cfg.push_endpoint.trim().is_empty() {
            let payload = serde_json::json!({
                "agent": "goldsmp-coolify-sentinel",
                "version": env!("CARGO_PKG_VERSION"),
                "timestamp": Utc::now().to_rfc3339(),
                "hostname": host.hostname,
                "host": {
                    "cpu_usage_percent": host_cpu,
                    "memory_usage_percent": host_mem,
                    "disk_usage_percent": host_disk,
                    "uptime_secs": host.uptime_secs
                },
                "instances": health_reports,
            });
            let mut req = self.http.post(cfg.push_endpoint.trim()).json(&payload);
            if !cfg.push_token.trim().is_empty() {
                req = req.bearer_auth(cfg.push_token.trim());
            }
            match req.send().await {
                Ok(r) => {
                    push_res_str = Some(format!("HTTP {} ({})", r.status().as_u16(), Utc::now().format("%H:%M:%S")));
                }
                Err(e) => {
                    push_res_str = Some(format!("Errore push: {}", e));
                }
            }
        }

        {
            let mut st = self.state.lock().unwrap();
            st.total_checks += 1;
            st.last_heartbeat = Some(Utc::now().to_rfc3339());
            st.host_cpu_pct = host_cpu;
            st.host_mem_pct = host_mem;
            st.host_disk_pct = host_disk;
            st.instances = health_reports;
            if let Some(p) = push_res_str {
                st.last_push_status = Some(p);
            }

            if !cfg.enabled {
                st.agent_state = "paused".into();
            } else if warnings.iter().any(|w| w.contains("CRASHED")) {
                st.agent_state = "critical".into();
            } else if !warnings.is_empty() {
                st.agent_state = "warning".into();
            } else {
                st.agent_state = "active".into();
            }

            if warnings != st.active_warnings {
                for w in &warnings {
                    if !st.active_warnings.contains(w) {
                        Self::push_event_locked(&mut st, "warn", "sentinel", w.clone());
                    }
                }
                st.active_warnings = warnings;
            }
        }

        self.get_status()
    }

    pub fn spawn_background_loop(self: &Arc<Self>) {
        let monitor = Arc::clone(self);
        tokio::spawn(async move {
            // Initial check shortly after startup
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            let _ = monitor.run_check_cycle().await;
            loop {
                let interval = monitor.load_config().check_interval_secs.clamp(5, 600);
                tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
                if monitor.load_config().enabled {
                    let _ = monitor.run_check_cycle().await;
                }
            }
        });
    }
}

async fn probe_tcp_port(port: u16) -> (bool, Option<u64>) {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let start = std::time::Instant::now();
    match tokio::time::timeout(
        std::time::Duration::from_millis(650),
        TcpStream::connect(addr),
    )
    .await
    {
        Ok(Ok(_stream)) => (true, Some(start.elapsed().as_millis() as u64)),
        _ => (false, None),
    }
}
