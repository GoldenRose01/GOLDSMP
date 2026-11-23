mod analytics;
mod api;
mod auth;
mod backups;
mod db;
mod files;
mod ftp_server;
mod gdrive_backup;
mod mailer;
mod models;
mod modrinth;
mod process_manager;
mod scheduler;
mod security;
mod state;
mod templates;
mod updater;
mod webhooks;

use crate::db::Database;
use crate::process_manager::ProcessManager;
use crate::security::SecurityGuard;
use crate::state::AppState;
use crate::updater::AutoUpdater;
use chrono::Utc;
use rusqlite::params;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let port: u16 = std::env::var("GOLDSMP_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let data_dir = std::env::var("GOLDSMP_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("./goldsmp-data"));

    std::fs::create_dir_all(data_dir.join("instances")).expect("Impossibile creare directory dati");
    std::fs::create_dir_all(data_dir.join("backups")).expect("Impossibile creare directory backup");

    let db_path = data_dir.join("goldsmp.db");
    let db = Database::open(&db_path).expect("Impossibile aprire database SQLite");

    let jwt_secret = std::env::var("GOLDSMP_JWT_SECRET")
        .unwrap_or_else(|_| "goldsmp-ultra-light-rust-secret-change-me".into());

    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .expect("Impossibile creare HTTP client");

    let pm = Arc::new(ProcessManager::new(db.clone(), data_dir.clone(), http.clone()));
    let updater = Arc::new(AutoUpdater::new(db.clone(), data_dir.clone(), http.clone()));

    seed_demo_instance_if_empty(&db, &data_dir);

    let ftp_port: u16 = std::env::var("GOLDSMP_FTP_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(2121);

    // Start background telemetry collector (5s resolution), task scheduler, zero-downtime auto-updater, and native FTP server
    analytics::spawn_telemetry_collector(db.clone(), pm.clone());
    scheduler::spawn_scheduler(db.clone(), pm.clone(), data_dir.clone(), http.clone());
    updater.spawn_background_loop();
    ftp_server::spawn_ftp_server(db.clone(), pm.clone(), ftp_port);

    let state = AppState {
        db,
        pm,
        updater,
        security: SecurityGuard::new(),
        data_dir,
        jwt_secret,
        http,
        ftp_port,
    };

    let app = api::build_router(state);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    tracing::info!(
        "GOLDSMP v{} attivo su http://{} (Login iniziale: admin / admin123)",
        env!("CARGO_PKG_VERSION"),
        addr
    );

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Impossibile fare bind sulla porta");
    axum::serve(listener, app).await.expect("Errore server HTTP");
}

fn seed_demo_instance_if_empty(db: &Database, data_dir: &PathBuf) {
    let conn = db.conn();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM instances", [], |r| r.get(0))
        .unwrap_or(0);
    if count > 0 {
        return;
    }

    // Create a ready-to-run Node.js Demo Game/App Server so the user can test Start/Console/Analytics immediately
    let id = Uuid::new_v4().to_string();
    let work_dir = data_dir.join("instances").join("demo-node-server");
    let _ = std::fs::create_dir_all(&work_dir);

    let demo_js = r#"// Demo Server GOLDSMP — Test Live Console, CPU/RAM & Player Analytics
const http = require('http');
const PORT = 3010;

const server = http.createServer((req, res) => {
  res.writeHead(200, { 'Content-Type': 'application/json' });
  res.end(JSON.stringify({ status: 'running', engine: 'GOLDSMP Native', uptime: Math.floor(process.uptime()) }));
});

server.listen(PORT, () => {
  console.log('[INFO] Demo Node Server ready and listening on port ' + PORT);
  console.log('[INFO] Suggerimento: digita "simjoin Alex 79.12.45.8" o "simleave Alex" nella console per testare Player Analytics in tempo reale!');
});

process.stdin.setEncoding('utf8');
process.stdin.on('data', (chunk) => {
  const cmd = chunk.trim();
  if (!cmd) return;
  if (cmd === 'stop' || cmd === 'exit') {
    console.log('[INFO] Arresto pulito in corso...');
    process.exit(0);
  } else if (cmd.startsWith('simjoin ')) {
    const parts = cmd.split(/\s+/);
    const name = parts[1] || 'Player1';
    const ip = parts[2] || '79.12.45.8';
    console.log('PLAYER_JOIN ' + name + ' ' + ip);
  } else if (cmd.startsWith('simleave ')) {
    const parts = cmd.split(/\s+/);
    console.log('PLAYER_LEAVE ' + (parts[1] || 'Player1'));
  } else if (cmd === 'help') {
    console.log('[HELP] Comandi disponibili: help, status, simjoin <nome> [ip], simleave <nome>, stop');
  } else if (cmd === 'status') {
    console.log('[STATUS] Uptime: ' + Math.floor(process.uptime()) + 's | RSS: ' + Math.round(process.memoryUsage().rss / 1024 / 1024) + ' MB');
  } else {
    console.log('[CMD] Eseguito: ' + cmd);
  }
});
"#;
    let _ = std::fs::write(work_dir.join("index.js"), demo_js);

    let _ = conn.execute(
        "INSERT INTO instances (id, name, template_id, category, port, work_dir, executable, args_json, env_json, settings_json, auto_start, auto_restart, max_memory_mb, max_players, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, '{}', ?9, 0, 1, 512, 50, ?10)",
        params![
            id,
            "Demo Server (Node.js)",
            "generic-nodejs",
            "generic",
            3010,
            work_dir.to_string_lossy().to_string(),
            "node",
            r#"["index.js"]"#,
            r#"{"NODE_ENV":"production","entry_file":"index.js"}"#,
            Utc::now().to_rfc3339()
        ],
    );
}
