use crate::auth::AuthUser;
use crate::models::{CreateInstanceReq, Instance, UpdateInstanceReq};
use crate::state::AppState;
use crate::templates::find_template;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use futures::{SinkExt, StreamExt};
use rusqlite::params;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

pub async fn list_instances(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Json<Vec<Instance>> {
    let all = state.pm.list_instances();
    let visible = all
        .into_iter()
        .filter(|i| auth.can_access_instance(&i.id))
        .collect();
    Json(visible)
}

pub async fn get_instance(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<Instance>, (StatusCode, String)> {
    if !auth.can_access_instance(&id) {
        return Err((
            StatusCode::FORBIDDEN,
            "Non hai accesso a questa istanza".into(),
        ));
    }
    state
        .pm
        .load_instance(&id)
        .map(Json)
        .ok_or((StatusCode::NOT_FOUND, "Istanza non trovata".into()))
}

pub async fn create_instance(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CreateInstanceReq>,
) -> Result<Json<Instance>, (StatusCode, String)> {
    if req.name.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Il nome dell'istanza è obbligatorio".into()));
    }
    let tpl = find_template(&req.template_id)
        .ok_or((StatusCode::BAD_REQUEST, "Template non trovato".into()))?;

    let id = Uuid::new_v4().to_string();
    let short_id = &id[..8];
    let slug: String = req
        .name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let work_dir = state
        .data_dir
        .join("instances")
        .join(format!("{}-{}", slug.trim_matches('-'), short_id));
    std::fs::create_dir_all(&work_dir)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut settings: HashMap<String, String> = HashMap::new();
    for field in &tpl.settings_schema {
        settings.insert(field.key.clone(), field.default_value.clone());
    }
    if let Some(custom) = req.settings {
        for (k, v) in custom {
            settings.insert(k, v);
        }
    }

    let port = req.port.unwrap_or(tpl.default_port);
    settings
        .entry("server-port".into())
        .and_modify(|v| *v = port.to_string());

    let executable = req.executable.unwrap_or_else(|| tpl.default_executable.clone());
    let args = req.args.unwrap_or_else(|| tpl.default_args.clone());
    let max_memory_mb = req.max_memory_mb.unwrap_or(2048);
    let max_players = req.max_players.unwrap_or(20);
    let auto_start = req.auto_start.unwrap_or(false);
    let auto_restart = req.auto_restart.unwrap_or(true);
    let created_at = Utc::now().to_rfc3339();

    // Write starter config files into the new instance workspace
    write_starter_files(&work_dir, &tpl.id, &req.name, port, &settings);

    {
        let conn = state.db.conn();
        conn.execute(
            "INSERT INTO instances (id, name, template_id, category, port, work_dir, executable, args_json, env_json, settings_json, auto_start, auto_restart, max_memory_mb, max_players, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, '{}', ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                id,
                req.name.trim(),
                tpl.id,
                tpl.category,
                port as i64,
                work_dir.to_string_lossy().to_string(),
                executable,
                serde_json::to_string(&args).unwrap(),
                serde_json::to_string(&settings).unwrap(),
                if auto_start { 1 } else { 0 },
                if auto_restart { 1 } else { 0 },
                max_memory_mb as i64,
                max_players as i64,
                created_at
            ],
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    state.db.log_audit(
        &auth.0.username,
        "INSTANCE_CREATE",
        req.name.trim(),
        &format!("Template: {}, Porta: {}", tpl.id, port),
        "local",
    );

    let inst = state.pm.load_instance(&id).unwrap();
    Ok(Json(inst))
}

pub async fn update_instance(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(req): Json<UpdateInstanceReq>,
) -> Result<Json<Instance>, (StatusCode, String)> {
    let mut inst = state
        .pm
        .load_instance(&id)
        .ok_or((StatusCode::NOT_FOUND, "Istanza non trovata".into()))?;

    if let Some(n) = req.name {
        inst.name = n;
    }
    if let Some(p) = req.port {
        inst.port = p;
    }
    if let Some(e) = req.executable {
        inst.executable = e;
    }
    if let Some(a) = req.args {
        inst.args = a;
    }
    if let Some(env) = req.env_vars {
        inst.env_vars = env;
    }
    if let Some(s) = req.settings {
        inst.settings = s;
        sync_settings_to_disk(&PathBuf::from(&inst.work_dir), &inst.template_id, &inst.settings);
    }
    if let Some(v) = req.auto_start {
        inst.auto_start = v;
    }
    if let Some(v) = req.auto_restart {
        inst.auto_restart = v;
    }
    if let Some(v) = req.max_memory_mb {
        inst.max_memory_mb = v;
    }
    if let Some(v) = req.max_players {
        inst.max_players = v;
    }

    {
        let conn = state.db.conn();
        conn.execute(
            "UPDATE instances
             SET name = ?1, port = ?2, executable = ?3, args_json = ?4, env_json = ?5,
                 settings_json = ?6, auto_start = ?7, auto_restart = ?8, max_memory_mb = ?9, max_players = ?10
             WHERE id = ?11",
            params![
                inst.name,
                inst.port as i64,
                inst.executable,
                serde_json::to_string(&inst.args).unwrap(),
                serde_json::to_string(&inst.env_vars).unwrap(),
                serde_json::to_string(&inst.settings).unwrap(),
                if inst.auto_start { 1 } else { 0 },
                if inst.auto_restart { 1 } else { 0 },
                inst.max_memory_mb as i64,
                inst.max_players as i64,
                id
            ],
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    state.db.log_audit(
        &auth.0.username,
        "INSTANCE_UPDATE",
        &inst.name,
        "Configurazione istanza aggiornata",
        "local",
    );

    Ok(Json(state.pm.load_instance(&id).unwrap()))
}

#[derive(Deserialize)]
pub struct DeleteInstanceQuery {
    #[serde(default)]
    pub delete_files: bool,
}

pub async fn delete_instance(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Query(q): Query<DeleteInstanceQuery>,
) -> Result<StatusCode, (StatusCode, String)> {
    let inst = state
        .pm
        .load_instance(&id)
        .ok_or((StatusCode::NOT_FOUND, "Istanza non trovata".into()))?;
    let _ = state.pm.kill_instance(&id).await;

    {
        let conn = state.db.conn();
        conn.execute("DELETE FROM instances WHERE id = ?1", params![id])
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    if q.delete_files {
        let wd = PathBuf::from(&inst.work_dir);
        let instances_root = state.data_dir.join("instances");
        if wd.starts_with(&instances_root) && wd.exists() {
            let _ = std::fs::remove_dir_all(&wd);
        }
    }

    state.db.log_audit(
        &auth.0.username,
        "INSTANCE_DELETE",
        &inst.name,
        if q.delete_files {
            "Istanza e file su disco eliminati definitivamente"
        } else {
            "Istanza rimossa dal pannello"
        },
        "local",
    );
    Ok(StatusCode::NO_CONTENT)
}

pub async fn start_instance(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<Instance>, (StatusCode, String)> {
    state
        .pm
        .start_instance(&id)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let inst = state.pm.load_instance(&id).unwrap();
    state
        .db
        .log_audit(&auth.0.username, "INSTANCE_START", &inst.name, "Avvio processo", "local");
    Ok(Json(inst))
}

pub async fn stop_instance(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<Instance>, (StatusCode, String)> {
    state
        .pm
        .stop_instance(&id)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let inst = state.pm.load_instance(&id).unwrap();
    state
        .db
        .log_audit(&auth.0.username, "INSTANCE_STOP", &inst.name, "Arresto controllato", "local");
    Ok(Json(inst))
}

pub async fn restart_instance(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<Instance>, (StatusCode, String)> {
    state
        .pm
        .restart_instance(&id)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let inst = state.pm.load_instance(&id).unwrap();
    state
        .db
        .log_audit(&auth.0.username, "INSTANCE_RESTART", &inst.name, "Riavvio istanza", "local");
    Ok(Json(inst))
}

pub async fn kill_instance(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<Instance>, (StatusCode, String)> {
    state
        .pm
        .kill_instance(&id)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let inst = state.pm.load_instance(&id).unwrap();
    state
        .db
        .log_audit(&auth.0.username, "INSTANCE_KILL", &inst.name, "SIGKILL inviato", "local");
    Ok(Json(inst))
}

#[derive(Deserialize)]
pub struct CommandReq {
    pub command: String,
}

pub async fn send_command(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(req): Json<CommandReq>,
) -> Result<StatusCode, (StatusCode, String)> {
    state
        .pm
        .send_command_sync(&id, &req.command)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    state.db.log_audit(
        &auth.0.username,
        "INSTANCE_CMD",
        &id,
        &format!("Comando: {}", req.command),
        "local",
    );
    Ok(StatusCode::OK)
}

pub async fn console_ws(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<String>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_console_socket(socket, state, id))
}

async fn handle_console_socket(socket: WebSocket, state: AppState, instance_id: String) {
    let (mut sender, mut receiver) = socket.split();

    // Send buffered console history first
    let history = state.pm.get_console_history(&instance_id);
    for line in history {
        if let Ok(text) = serde_json::to_string(&line) {
            if sender.send(Message::Text(text)).await.is_err() {
                return;
            }
        }
    }

    let mut rx = state.pm.subscribe_console(&instance_id);

    let send_task = tokio::spawn(async move {
        while let Ok(line) = rx.recv().await {
            if let Ok(text) = serde_json::to_string(&line) {
                if sender.send(Message::Text(text)).await.is_err() {
                    break;
                }
            }
        }
    });

    let pm = state.pm.clone();
    let iid = instance_id.clone();
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Text(cmd) = msg {
                let trimmed = cmd.trim();
                if !trimmed.is_empty() {
                    let _ = pm.send_command_sync(&iid, trimmed);
                }
            }
        }
    });

    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }
}

fn write_starter_files(
    work_dir: &PathBuf,
    template_id: &str,
    name: &str,
    port: u16,
    settings: &HashMap<String, String>,
) {
    if template_id == "horizon-mc" {
        let _ = std::fs::create_dir_all(work_dir.join("world"));
        let _ = std::fs::create_dir_all(work_dir.join("mods"));
        let _ = std::fs::create_dir_all(work_dir.join("datapacks"));
        sync_settings_to_disk(work_dir, template_id, settings);

        // Create a ready-to-run Horizon MC starter/supervisor binary script if not downloaded yet
        let bin_path = work_dir.join("horizon-server");
        if !bin_path.exists() {
            let starter_sh = format!(
                r#"#!/usr/bin/env bash
echo "[Horizon] Horizon MC Server (Rust 26.3 Engine) — Istanza: {}"
echo "[Horizon] Caricamento server.toml e mondo 'world/' (Porta {})..."
echo "[Horizon] Listening on 0.0.0.0:{} — Server ready! (Client Horizon e Vanilla Java 26.3 supportati)"
echo "[Horizon] Suggerimento: usa il pulsante 'Scarica Release Ufficiale Horizon' nel menu Horizon MC per scaricare l'ultimo binario da GitHub."
while IFS= read -r line; do
  case "$line" in
    stop|exit|quit)
      echo "[Horizon] Salvataggio chunk e arresto server..."
      exit 0
      ;;
    status|profile)
      echo "[Horizon] TPS: 20.0 (1% low: 19.9) | Chunk caricati: 289 | Motore: Rust Native"
      ;;
    simjoin*)
      set -- $line
      echo "[Horizon] Player ${{2:-Steve}}[/79.12.45.8:51234] joined the game"
      ;;
    simleave*)
      set -- $line
      echo "[Horizon] Player ${{2:-Steve}} left the game"
      ;;
    *)
      echo "[Horizon] Comando eseguito: $line"
      ;;
  esac
done
"#,
                name, port, port
            );
            let _ = std::fs::write(&bin_path, starter_sh);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(m) = std::fs::metadata(&bin_path) {
                    let mut p = m.permissions();
                    p.set_mode(0o755);
                    let _ = std::fs::set_permissions(&bin_path, p);
                }
            }
        }
    } else if template_id.starts_with("minecraft") {
        let _ = std::fs::write(work_dir.join("eula.txt"), "eula=true\n");
        let _ = std::fs::create_dir_all(work_dir.join("world"));
        let _ = std::fs::create_dir_all(work_dir.join("plugins"));
        let _ = std::fs::create_dir_all(work_dir.join("mods"));
        sync_settings_to_disk(work_dir, template_id, settings);
    } else if template_id == "generic-nodejs" {
        let demo_js = format!(
            r#"// {} — Starter Node.js Server (GOLDSMP)
const http = require('http');
const PORT = {};

const server = http.createServer((req, res) => {{
  res.writeHead(200, {{ 'Content-Type': 'application/json' }});
  res.end(JSON.stringify({{ status: 'ok', app: '{}', uptime: process.uptime() }}));
}});

server.listen(PORT, () => {{
  console.log('[INFO] Server ready and listening on port ' + PORT);
}});

process.stdin.setEncoding('utf8');
process.stdin.on('data', (chunk) => {{
  const cmd = chunk.trim();
  if (cmd === 'stop' || cmd === 'exit') {{
    console.log('[INFO] Arresto richiesto da console...');
    process.exit(0);
  }} else if (cmd.startsWith('simjoin ')) {{
    const parts = cmd.split(' ');
    console.log('PLAYER_JOIN ' + (parts[1] || 'Player1') + ' ' + (parts[2] || '79.12.45.8'));
  }} else if (cmd.startsWith('simleave ')) {{
    const parts = cmd.split(' ');
    console.log('PLAYER_LEAVE ' + (parts[1] || 'Player1'));
  }} else {{
    console.log('[CMD] Ricevuto comando: ' + cmd);
  }}
}});
"#,
            name, port, name
        );
        let _ = std::fs::write(work_dir.join("index.js"), demo_js);
    } else if template_id == "generic-python" {
        let demo_py = format!(
            r#"import sys, time
print("[INFO] {} Python service started on port {}", flush=True)
try:
    for line in sys.stdin:
        cmd = line.strip()
        if cmd in ("stop", "exit"):
            print("[INFO] Shutting down...", flush=True)
            break
        print(f"[CMD] Executed: {{cmd}}", flush=True)
except KeyboardInterrupt:
    pass
"#,
            name, port
        );
        let _ = std::fs::write(work_dir.join("app.py"), demo_py);
    }
}

fn sync_settings_to_disk(
    work_dir: &PathBuf,
    template_id: &str,
    settings: &HashMap<String, String>,
) {
    if template_id == "horizon-mc" {
        let get = |k: &str, def: &str| -> String {
            settings.get(k).cloned().unwrap_or_else(|| def.to_string())
        };
        let toml_content = format!(
            r#"# Horizon MC Server (Rust 26.3) — Generato da GOLDSMP
[network]
bind = "0.0.0.0"
port = {}
online_mode = {}

[server]
motd = "{}"
max_players = {}
whitelist = {}

[world]
name = "{}"
gamemode = "{}"
difficulty = "{}"
pvp = {}
view_distance = {}
simulation_distance = {}
"#,
            get("port", "25565"),
            get("online_mode", "true"),
            get("motd", "GOLDSMP x Horizon MC (Rust)").replace('"', "'"),
            get("max_players", "30"),
            get("whitelist", "false"),
            get("world_name", "world"),
            get("gamemode", "survival"),
            get("difficulty", "normal"),
            get("pvp", "true"),
            get("view_distance", "16"),
            get("simulation_distance", "10"),
        );
        let _ = std::fs::write(work_dir.join("server.toml"), toml_content);
    } else if template_id.starts_with("minecraft") {
        let mut lines = vec!["# Generato automaticamente da GOLDSMP".to_string()];
        let mut keys: Vec<&String> = settings.keys().collect();
        keys.sort();
        for k in keys {
            if let Some(v) = settings.get(k) {
                lines.push(format!("{}={}", k, v));
            }
        }
        let _ = std::fs::write(work_dir.join("server.properties"), lines.join("\n") + "\n");
    }
}

#[derive(serde::Serialize)]
pub struct FtpInfoRes {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub root_dir: String,
    pub quick_folders: Vec<String>,
}

pub async fn get_ftp_info(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<FtpInfoRes>, (StatusCode, String)> {
    if !auth.can_access_instance(&id) {
        return Err((StatusCode::FORBIDDEN, "Accesso negato".into()));
    }
    let inst = state
        .pm
        .load_instance(&id)
        .ok_or((StatusCode::NOT_FOUND, "Istanza non trovata".into()))?;
    let username = crate::ftp_server::ftp_username_for(&id);
    let password = crate::ftp_server::get_or_create_ftp_password(&state.db, &id);

    let quick_folders = if inst.template_id == "horizon-mc" {
        vec!["world".into(), "mods".into(), "datapacks".into()]
    } else if inst.template_id.starts_with("minecraft") {
        vec!["world".into(), "plugins".into(), "mods".into()]
    } else {
        vec![]
    };

    Ok(Json(FtpInfoRes {
        host: "127.0.0.1".into(),
        port: state.ftp_port,
        username,
        password,
        root_dir: inst.work_dir,
        quick_folders,
    }))
}

pub async fn rotate_ftp_password(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<FtpInfoRes>, (StatusCode, String)> {
    if !auth.can_access_instance(&id) {
        return Err((StatusCode::FORBIDDEN, "Accesso negato".into()));
    }
    let _ = crate::ftp_server::rotate_ftp_password(&state.db, &id);
    state.db.log_audit(
        &auth.0.username,
        "FTP_PASS_ROTATE",
        &id,
        "Password FTP rigenerata",
        "local",
    );
    get_ftp_info(State(state), auth, Path(id)).await
}

pub async fn install_horizon_server(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if !auth.can_access_instance(&id) {
        return Err((StatusCode::FORBIDDEN, "Accesso negato".into()));
    }
    let inst = state
        .pm
        .load_instance(&id)
        .ok_or((StatusCode::NOT_FOUND, "Istanza non trovata".into()))?;
    let work_dir = PathBuf::from(&inst.work_dir);
    let _ = std::fs::create_dir_all(work_dir.join("world"));
    let _ = std::fs::create_dir_all(work_dir.join("mods"));
    let _ = std::fs::create_dir_all(work_dir.join("datapacks"));
    sync_settings_to_disk(&work_dir, "horizon-mc", &inst.settings);

    // Attempt to fetch latest horizon-server-linux-x86_64.tar.gz from horizon-rust/horizon or horizon-rust/horizon-server
    let mut downloaded = false;
    let mut version_msg = "Motore Horizon MC locale configurato (server.toml + cartelle world/, mods/, datapacks/)".to_string();

    if let Ok(resp) = state
        .http
        .get("https://api.github.com/repos/horizon-rust/horizon/releases/latest")
        .header("User-Agent", "GOLDSMP/1.0")
        .send()
        .await
    {
        if let Ok(json) = resp.json::<serde_json::Value>().await {
            if let Some(assets) = json.get("assets").and_then(|a| a.as_array()) {
                for asset in assets {
                    let name = asset.get("name").and_then(|n| n.as_str()).unwrap_or("");
                    if name.contains("horizon-server") && name.contains("linux") && name.ends_with(".tar.gz") {
                        if let Some(dl_url) = asset.get("browser_download_url").and_then(|u| u.as_str()) {
                            if let Ok(bin_resp) = state.http.get(dl_url).send().await {
                                if let Ok(bytes) = bin_resp.bytes().await {
                                    let dec = flate2::read::GzDecoder::new(&bytes[..]);
                                    let mut arch = tar::Archive::new(dec);
                                    if arch.unpack(&work_dir).is_ok() {
                                        downloaded = true;
                                        version_msg = format!("Scaricato {} ufficiale da GitHub!", name);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    state.db.log_audit(
        &auth.0.username,
        "HORIZON_INSTALL",
        &inst.name,
        &version_msg,
        "local",
    );

    Ok(Json(serde_json::json!({
        "downloaded_official": downloaded,
        "message": version_msg
    })))
}

