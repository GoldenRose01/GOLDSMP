use crate::db::Database;
use crate::files::dir_size_mb;
use crate::models::{ConsoleLine, Instance};
use crate::templates::find_template;
use crate::webhooks;
use chrono::Utc;
use regex::Regex;
use rusqlite::params;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::{broadcast, mpsc};
use uuid::Uuid;

const CONSOLE_BUFFER_SIZE: usize = 500;

pub struct RuntimeState {
    pub status: String,
    pub pid: Option<u32>,
    pub cpu_percent: f32,
    pub memory_mb: f64,
    pub disk_mb: f64,
    pub started_at: Option<Instant>,
    pub active_players: Vec<String>,
    pub stdin_tx: Option<mpsc::Sender<String>>,
    pub kill_tx: Option<mpsc::Sender<()>>,
    pub console_buffer: VecDeque<ConsoleLine>,
    pub console_tx: broadcast::Sender<ConsoleLine>,
}

impl RuntimeState {
    fn new() -> Self {
        let (console_tx, _) = broadcast::channel(256);
        Self {
            status: "stopped".into(),
            pid: None,
            cpu_percent: 0.0,
            memory_mb: 0.0,
            disk_mb: 0.0,
            started_at: None,
            active_players: Vec::new(),
            stdin_tx: None,
            kill_tx: None,
            console_buffer: VecDeque::with_capacity(CONSOLE_BUFFER_SIZE),
            console_tx,
        }
    }

    pub fn push_line(&mut self, stream: &str, message: String) {
        let line = ConsoleLine {
            timestamp: Utc::now().format("%H:%M:%S").to_string(),
            stream: stream.to_string(),
            message,
        };
        if self.console_buffer.len() >= CONSOLE_BUFFER_SIZE {
            self.console_buffer.pop_front();
        }
        self.console_buffer.push_back(line.clone());
        let _ = self.console_tx.send(line);
    }
}

pub struct ProcessManager {
    pub db: Database,
    pub data_dir: PathBuf,
    pub http: reqwest::Client,
    pub runtimes: Arc<Mutex<HashMap<String, RuntimeState>>>,
}

impl ProcessManager {
    pub fn new(db: Database, data_dir: PathBuf, http: reqwest::Client) -> Self {
        Self {
            db,
            data_dir,
            http,
            runtimes: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn ensure_runtime_entry(&self, instance_id: &str) {
        let mut map = self.runtimes.lock().unwrap();
        map.entry(instance_id.to_string())
            .or_insert_with(RuntimeState::new);
    }

    pub fn get_console_history(&self, instance_id: &str) -> Vec<ConsoleLine> {
        self.ensure_runtime_entry(instance_id);
        let map = self.runtimes.lock().unwrap();
        map.get(instance_id)
            .map(|rt| rt.console_buffer.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn subscribe_console(&self, instance_id: &str) -> broadcast::Receiver<ConsoleLine> {
        self.ensure_runtime_entry(instance_id);
        let map = self.runtimes.lock().unwrap();
        map.get(instance_id).unwrap().console_tx.subscribe()
    }

    pub fn load_instance(&self, instance_id: &str) -> Option<Instance> {
        let conn = self.db.conn();
        let row_res = conn.query_row(
            "SELECT id, name, template_id, category, port, work_dir, executable, args_json, env_json, settings_json,
                    auto_start, auto_restart, max_memory_mb, max_players, created_at
             FROM instances WHERE id = ?1",
            params![instance_id],
            |r| {
                let args_str: String = r.get(7)?;
                let env_str: String = r.get(8)?;
                let set_str: String = r.get(9)?;
                Ok(Instance {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    template_id: r.get(2)?,
                    category: r.get(3)?,
                    status: "stopped".into(),
                    port: r.get::<_, i64>(4)? as u16,
                    work_dir: r.get(5)?,
                    executable: r.get(6)?,
                    args: serde_json::from_str(&args_str).unwrap_or_default(),
                    env_vars: serde_json::from_str(&env_str).unwrap_or_default(),
                    settings: serde_json::from_str(&set_str).unwrap_or_default(),
                    auto_start: r.get::<_, i64>(10)? != 0,
                    auto_restart: r.get::<_, i64>(11)? != 0,
                    max_memory_mb: r.get::<_, i64>(12)? as u64,
                    max_players: r.get::<_, i64>(13)? as u32,
                    pid: None,
                    cpu_percent: 0.0,
                    memory_mb: 0.0,
                    disk_mb: 0.0,
                    active_players: Vec::new(),
                    uptime_secs: 0,
                    created_at: r.get(14)?,
                })
            },
        );
        drop(conn);

        let mut inst = row_res.ok()?;
        let map = self.runtimes.lock().unwrap();
        if let Some(rt) = map.get(&inst.id) {
            inst.status = rt.status.clone();
            inst.pid = rt.pid;
            inst.cpu_percent = rt.cpu_percent;
            inst.memory_mb = rt.memory_mb;
            inst.disk_mb = rt.disk_mb;
            inst.active_players = rt.active_players.clone();
            inst.uptime_secs = rt.started_at.map(|s| s.elapsed().as_secs()).unwrap_or(0);
        } else {
            inst.disk_mb = dir_size_mb(&PathBuf::from(&inst.work_dir));
        }
        Some(inst)
    }

    pub fn list_instances(&self) -> Vec<Instance> {
        let ids: Vec<String> = {
            let conn = self.db.conn();
            let mut stmt = conn
                .prepare("SELECT id FROM instances ORDER BY created_at ASC")
                .unwrap();
            stmt.query_map([], |r| r.get(0))
                .unwrap()
                .filter_map(|r| r.ok())
                .collect()
        };
        ids.into_iter()
            .filter_map(|id| self.load_instance(&id))
            .collect()
    }

    pub async fn start_instance(self: &Arc<Self>, instance_id: &str) -> Result<(), String> {
        let inst = self
            .load_instance(instance_id)
            .ok_or_else(|| "Istanza non trovata".to_string())?;

        self.ensure_runtime_entry(instance_id);

        {
            let map = self.runtimes.lock().unwrap();
            let rt = map.get(instance_id).unwrap();
            if rt.status == "running" || rt.status == "starting" {
                return Err("L'istanza è già in esecuzione".into());
            }
        }

        let work_dir = PathBuf::from(&inst.work_dir);
        std::fs::create_dir_all(&work_dir).map_err(|e| e.to_string())?;

        // Resolve template placeholders in args
        let resolved_args: Vec<String> = inst
            .args
            .iter()
            .map(|a| {
                a.replace("{PORT}", &inst.port.to_string())
                    .replace("{MAX_MEMORY}", &inst.max_memory_mb.to_string())
                    .replace("{MAX_PLAYERS}", &inst.max_players.to_string())
                    .replace("{SERVER_NAME}", &inst.name)
            })
            .collect();

        let mut cmd = Command::new(&inst.executable);
        cmd.args(&resolved_args)
            .current_dir(&work_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (k, v) in &inst.env_vars {
            cmd.env(k, v);
        }

        let mut child = cmd.spawn().map_err(|e| {
            let msg = format!(
                "Impossibile avviare '{}' {:?}: {}",
                inst.executable, resolved_args, e
            );
            let mut map = self.runtimes.lock().unwrap();
            if let Some(rt) = map.get_mut(instance_id) {
                rt.push_line("system", format!("[ERRORE] {}", msg));
            }
            msg
        })?;

        let pid = child.id();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let mut stdin = child.stdin.take();

        let (stdin_tx, mut stdin_rx) = mpsc::channel::<String>(64);
        let (kill_tx, mut kill_rx) = mpsc::channel::<()>(2);

        {
            let mut map = self.runtimes.lock().unwrap();
            let rt = map.get_mut(instance_id).unwrap();
            rt.status = "running".into();
            rt.pid = pid;
            rt.started_at = Some(Instant::now());
            rt.active_players.clear();
            rt.stdin_tx = Some(stdin_tx);
            rt.kill_tx = Some(kill_tx);
            rt.push_line(
                "system",
                format!(
                    "[GOLDSMP] Processo avviato (PID: {:?}) -> {} {}",
                    pid,
                    inst.executable,
                    resolved_args.join(" ")
                ),
            );
        }

        webhooks::dispatch_event(
            self.db.clone(),
            self.http.clone(),
            "server_start".into(),
            inst.id.clone(),
            inst.name.clone(),
            format!("Istanza '{}' avviata (PID {:?})", inst.name, pid),
        );

        let template = find_template(&inst.template_id);
        let join_re = template
            .as_ref()
            .and_then(|t| t.log_patterns.player_join.as_ref())
            .and_then(|p| Regex::new(p).ok());
        let leave_re = template
            .as_ref()
            .and_then(|t| t.log_patterns.player_leave.as_ref())
            .and_then(|p| Regex::new(p).ok());

        // Stdin writer task
        tokio::spawn(async move {
            while let Some(line) = stdin_rx.recv().await {
                if let Some(ref mut s) = stdin {
                    let _ = s.write_all(format!("{}\n", line).as_bytes()).await;
                    let _ = s.flush().await;
                }
            }
        });

        // Stdout reader task with live player/event log parsing
        if let Some(out) = stdout {
            let pm = Arc::clone(self);
            let id = inst.id.clone();
            let name = inst.name.clone();
            let jre = join_re.clone();
            let lre = leave_re.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(out).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    {
                        let mut map = pm.runtimes.lock().unwrap();
                        if let Some(rt) = map.get_mut(&id) {
                            rt.push_line("stdout", line.clone());
                        }
                    }
                    pm.parse_log_line(&id, &name, &line, &jre, &lre);
                }
            });
        }

        // Stderr reader task
        if let Some(err) = stderr {
            let pm = Arc::clone(self);
            let id = inst.id.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(err).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let mut map = pm.runtimes.lock().unwrap();
                    if let Some(rt) = map.get_mut(&id) {
                        rt.push_line("stderr", line);
                    }
                }
            });
        }

        // Process waiter / kill listener
        let pm = Arc::clone(self);
        let id = inst.id.clone();
        let name = inst.name.clone();
        tokio::spawn(async move {
            let exit_code = tokio::select! {
                status = child.wait() => {
                    status.ok().and_then(|s| s.code())
                }
                _ = kill_rx.recv() => {
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                    Some(-9)
                }
            };

            let was_stopping = {
                let mut map = pm.runtimes.lock().unwrap();
                let mut stopping = false;
                if let Some(rt) = map.get_mut(&id) {
                    stopping = rt.status == "stopping";
                    let crashed = !stopping && exit_code.unwrap_or(0) != 0 && exit_code != Some(-9);
                    rt.status = if crashed { "crashed".into() } else { "stopped".into() };
                    rt.pid = None;
                    rt.cpu_percent = 0.0;
                    rt.memory_mb = 0.0;
                    rt.started_at = None;
                    rt.stdin_tx = None;
                    rt.kill_tx = None;
                    rt.active_players.clear();
                    rt.push_line(
                        "system",
                        format!(
                            "[GOLDSMP] Processo terminato (codice: {:?})",
                            exit_code
                        ),
                    );
                }
                stopping
            };

            // Close any open player sessions in DB
            {
                let conn = pm.db.conn();
                let now = Utc::now().to_rfc3339();
                let _ = conn.execute(
                    "UPDATE player_sessions
                     SET left_at = ?1,
                         duration_secs = MAX(1, CAST((julianday(?1) - julianday(joined_at)) * 86400 AS INTEGER))
                     WHERE instance_id = ?2 AND left_at IS NULL",
                    params![now, id],
                );
            }

            let event_name = if !was_stopping && exit_code.unwrap_or(0) != 0 && exit_code != Some(-9) {
                "server_crash"
            } else {
                "server_stop"
            };

            webhooks::dispatch_event(
                pm.db.clone(),
                pm.http.clone(),
                event_name.into(),
                id.clone(),
                name.clone(),
                format!("Istanza '{}' terminata (codice {:?})", name, exit_code),
            );
        });

        Ok(())
    }

    fn parse_log_line(
        self: &Arc<Self>,
        instance_id: &str,
        instance_name: &str,
        line: &str,
        join_re: &Option<Regex>,
        leave_re: &Option<Regex>,
    ) {
        if let Some(re) = join_re {
            if let Some(caps) = re.captures(line) {
                let player = caps.get(1).map(|m| m.as_str().trim().to_string()).unwrap_or_default();
                let ip = caps
                    .get(2)
                    .map(|m| m.as_str().trim().to_string())
                    .unwrap_or_else(|| "127.0.0.1".to_string());
                if !player.is_empty() {
                    self.record_player_join(instance_id, instance_name, &player, &ip);
                }
            }
        }
        if let Some(re) = leave_re {
            if let Some(caps) = re.captures(line) {
                let player = caps.get(1).map(|m| m.as_str().trim().to_string()).unwrap_or_default();
                if !player.is_empty() {
                    self.record_player_leave(instance_id, instance_name, &player);
                }
            }
        }
    }

    pub fn record_player_join(
        self: &Arc<Self>,
        instance_id: &str,
        instance_name: &str,
        player_name: &str,
        ip: &str,
    ) {
        let (cc, cname) = classify_ip_geo(ip);
        {
            let mut map = self.runtimes.lock().unwrap();
            if let Some(rt) = map.get_mut(instance_id) {
                if !rt.active_players.iter().any(|p| p == player_name) {
                    rt.active_players.push(player_name.to_string());
                }
            }
        }
        {
            let conn = self.db.conn();
            let _ = conn.execute(
                "INSERT INTO player_sessions (id, instance_id, instance_name, player_name, ip_address, country_code, country_name, joined_at, duration_secs)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0)",
                params![
                    Uuid::new_v4().to_string(),
                    instance_id,
                    instance_name,
                    player_name,
                    ip,
                    cc,
                    cname,
                    Utc::now().to_rfc3339()
                ],
            );

            // Check geo-filter rules
            let blocked: bool = conn
                .query_row(
                    "SELECT COUNT(*) FROM geo_rules
                     WHERE enabled = 1 AND action = 'block' AND country_code = ?1
                       AND (instance_id = '*' OR instance_id = ?2)",
                    params![cc, instance_id],
                    |r| r.get::<_, i64>(0),
                )
                .unwrap_or(0)
                > 0;

            if blocked {
                drop(conn);
                let _ = self.send_command_sync(
                    instance_id,
                    &format!("kick {} Connessione bloccata dal Geo-Filter ({})", player_name, cname),
                );
            }
        }

        webhooks::dispatch_event(
            self.db.clone(),
            self.http.clone(),
            "player_join".into(),
            instance_id.to_string(),
            instance_name.to_string(),
            format!("{} si è connesso ({} - {})", player_name, ip, cname),
        );
    }

    pub fn record_player_leave(
        self: &Arc<Self>,
        instance_id: &str,
        instance_name: &str,
        player_name: &str,
    ) {
        {
            let mut map = self.runtimes.lock().unwrap();
            if let Some(rt) = map.get_mut(instance_id) {
                rt.active_players.retain(|p| p != player_name);
            }
        }
        {
            let conn = self.db.conn();
            let now = Utc::now().to_rfc3339();
            let _ = conn.execute(
                "UPDATE player_sessions
                 SET left_at = ?1,
                     duration_secs = MAX(1, CAST((julianday(?1) - julianday(joined_at)) * 86400 AS INTEGER))
                 WHERE instance_id = ?2 AND player_name = ?3 AND left_at IS NULL",
                params![now, instance_id, player_name],
            );
        }

        webhooks::dispatch_event(
            self.db.clone(),
            self.http.clone(),
            "player_leave".into(),
            instance_id.to_string(),
            instance_name.to_string(),
            format!("{} ha lasciato il server", player_name),
        );
    }

    pub fn send_command_sync(&self, instance_id: &str, cmd: &str) -> Result<(), String> {
        let tx = {
            let mut map = self.runtimes.lock().unwrap();
            let rt = map
                .get_mut(instance_id)
                .ok_or_else(|| "Istanza non in esecuzione".to_string())?;
            rt.push_line("stdin", format!("> {}", cmd));
            rt.stdin_tx
                .clone()
                .ok_or_else(|| "Stdin non disponibile (istanza ferma)".to_string())?
        };
        let command = cmd.to_string();
        tokio::spawn(async move {
            let _ = tx.send(command).await;
        });
        Ok(())
    }

    pub async fn stop_instance(self: &Arc<Self>, instance_id: &str) -> Result<(), String> {
        let inst = self
            .load_instance(instance_id)
            .ok_or_else(|| "Istanza non trovata".to_string())?;
        let stop_cmd = find_template(&inst.template_id)
            .map(|t| t.stop_command)
            .unwrap_or_default();

        let (stdin_tx, kill_tx) = {
            let mut map = self.runtimes.lock().unwrap();
            let rt = map
                .get_mut(instance_id)
                .ok_or_else(|| "Istanza non in esecuzione".to_string())?;
            if rt.status != "running" && rt.status != "starting" {
                return Err("L'istanza non è in esecuzione".into());
            }
            rt.status = "stopping".into();
            rt.push_line("system", "[GOLDSMP] Arresto controllato richiesto...".into());
            (rt.stdin_tx.clone(), rt.kill_tx.clone())
        };

        if !stop_cmd.is_empty() {
            if let Some(tx) = stdin_tx {
                let _ = tx.send(stop_cmd).await;
            }
            // Wait up to 8 seconds for graceful exit, then kill
            for _ in 0..16 {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                let still_running = {
                    let map = self.runtimes.lock().unwrap();
                    map.get(instance_id)
                        .map(|r| r.status == "stopping" || r.status == "running")
                        .unwrap_or(false)
                };
                if !still_running {
                    return Ok(());
                }
            }
        }

        if let Some(ktx) = kill_tx {
            let _ = ktx.send(()).await;
        }
        Ok(())
    }

    pub async fn kill_instance(&self, instance_id: &str) -> Result<(), String> {
        let kill_tx = {
            let mut map = self.runtimes.lock().unwrap();
            let rt = map
                .get_mut(instance_id)
                .ok_or_else(|| "Istanza non trovata".to_string())?;
            rt.status = "stopping".into();
            rt.push_line("system", "[GOLDSMP] Terminazione forzata (SIGKILL)...".into());
            rt.kill_tx.clone()
        };
        if let Some(ktx) = kill_tx {
            let _ = ktx.send(()).await;
        }
        Ok(())
    }

    pub async fn restart_instance(self: &Arc<Self>, instance_id: &str) -> Result<(), String> {
        let _ = self.stop_instance(instance_id).await;
        tokio::time::sleep(std::time::Duration::from_millis(800)).await;
        self.start_instance(instance_id).await
    }
}

/// Lightweight built-in IP geolocation helper (no external paid GeoIP license required).
/// Identifies local/LAN IPs and maps public IP ranges deterministically for immediate analytics;
/// can be extended with a local MMDB file.
fn classify_ip_geo(ip: &str) -> (&'static str, &'static str) {
    if ip.starts_with("127.") || ip.starts_with("192.168.") || ip.starts_with("10.") || ip == "::1" {
        return ("LOCAL", "Rete Locale");
    }
    let first: u8 = ip
        .split('.')
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    match first {
        2 | 5 | 31 | 37 | 79 | 80 | 82 | 87 | 93 | 151 => ("IT", "Italia"),
        46 | 62 | 77 | 78 | 84 | 85 | 88 | 89 | 91 | 92 => ("DE", "Germania"),
        81 | 83 | 86 | 90 | 176 => ("FR", "Francia"),
        51 | 109 | 146 | 185 => ("GB", "Regno Unito"),
        3 | 13 | 18 | 23 | 24 | 34 | 35 | 44 | 50 | 52 | 54 | 63..=76 | 96..=108 => ("US", "Stati Uniti"),
        1 | 14 | 27 | 36 | 42 | 49 | 58..=61 | 110..=126 => ("JP", "Giappone"),
        177..=191 | 200..=201 => ("BR", "Brasile"),
        _ => ("EU", "Europa"),
    }
}
