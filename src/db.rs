use crate::models::{PanelSettings, User};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use chrono::Utc;
use rusqlite::{params, Connection, Result};
use std::path::Path;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA foreign_keys = ON;
            PRAGMA busy_timeout = 5000;
            ",
        )?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.init_schema()?;
        db.seed_defaults()?;
        Ok(db)
    }

    pub fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().expect("database mutex poisoned")
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.conn();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                username TEXT UNIQUE NOT NULL,
                email TEXT NOT NULL,
                password_hash TEXT NOT NULL,
                role TEXT NOT NULL,
                permissions_json TEXT NOT NULL DEFAULT '[]',
                oidc_subject TEXT,
                created_at TEXT NOT NULL,
                last_login TEXT
            );

            CREATE TABLE IF NOT EXISTS instances (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                template_id TEXT NOT NULL,
                category TEXT NOT NULL,
                port INTEGER NOT NULL,
                work_dir TEXT NOT NULL,
                executable TEXT NOT NULL,
                args_json TEXT NOT NULL DEFAULT '[]',
                env_json TEXT NOT NULL DEFAULT '{}',
                settings_json TEXT NOT NULL DEFAULT '{}',
                auto_start INTEGER NOT NULL DEFAULT 0,
                auto_restart INTEGER NOT NULL DEFAULT 1,
                max_memory_mb INTEGER NOT NULL DEFAULT 2048,
                max_players INTEGER NOT NULL DEFAULT 20,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS metrics_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp INTEGER NOT NULL,
                instance_id TEXT NOT NULL,
                cpu_percent REAL NOT NULL,
                memory_mb REAL NOT NULL,
                disk_mb REAL NOT NULL,
                net_rx_kb REAL NOT NULL DEFAULT 0,
                net_tx_kb REAL NOT NULL DEFAULT 0,
                players_online INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_metrics_inst_ts ON metrics_history(instance_id, timestamp);

            CREATE TABLE IF NOT EXISTS player_sessions (
                id TEXT PRIMARY KEY,
                instance_id TEXT NOT NULL,
                instance_name TEXT NOT NULL,
                player_name TEXT NOT NULL,
                player_uuid TEXT,
                ip_address TEXT NOT NULL,
                country_code TEXT NOT NULL DEFAULT 'XX',
                country_name TEXT NOT NULL DEFAULT 'Unknown',
                joined_at TEXT NOT NULL,
                left_at TEXT,
                duration_secs INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_sessions_inst ON player_sessions(instance_id);
            CREATE INDEX IF NOT EXISTS idx_sessions_player ON player_sessions(player_name);

            CREATE TABLE IF NOT EXISTS geo_rules (
                id TEXT PRIMARY KEY,
                instance_id TEXT NOT NULL,
                country_code TEXT NOT NULL,
                country_name TEXT NOT NULL,
                action TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS scheduled_tasks (
                id TEXT PRIMARY KEY,
                instance_id TEXT NOT NULL,
                name TEXT NOT NULL,
                trigger_type TEXT NOT NULL,
                interval_secs INTEGER,
                event_name TEXT,
                action_type TEXT NOT NULL,
                action_payload TEXT NOT NULL DEFAULT '',
                enabled INTEGER NOT NULL DEFAULT 1,
                last_run TEXT,
                run_count INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS backups (
                id TEXT PRIMARY KEY,
                instance_id TEXT NOT NULL,
                name TEXT NOT NULL,
                filename TEXT NOT NULL,
                size_bytes INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                notes TEXT NOT NULL DEFAULT ''
            );

            CREATE TABLE IF NOT EXISTS webhooks (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                url TEXT NOT NULL,
                events_json TEXT NOT NULL DEFAULT '[]',
                enabled INTEGER NOT NULL DEFAULT 1,
                secret TEXT NOT NULL,
                last_triggered TEXT,
                last_status INTEGER,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS audit_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                username TEXT NOT NULL,
                action TEXT NOT NULL,
                target TEXT NOT NULL,
                details TEXT NOT NULL,
                ip_address TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_audit_ts ON audit_logs(id DESC);

            CREATE TABLE IF NOT EXISTS kv_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            ",
        )?;
        Ok(())
    }

    fn seed_defaults(&self) -> Result<()> {
        let conn = self.conn();
        let user_count: i64 = conn.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))?;
        if user_count == 0 {
            let salt = SaltString::generate(&mut OsRng);
            let hash = Argon2::default()
                .hash_password(b"admin123", &salt)
                .expect("failed to hash default password")
                .to_string();
            let perms = serde_json::to_string(&vec!["*"]).unwrap();
            conn.execute(
                "INSERT INTO users (id, username, email, password_hash, role, permissions_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    Uuid::new_v4().to_string(),
                    "admin",
                    "admin@goldsmp.local",
                    hash,
                    "superadmin",
                    perms,
                    Utc::now().to_rfc3339()
                ],
            )?;
        }

        let has_settings: i64 =
            conn.query_row("SELECT COUNT(*) FROM kv_settings WHERE key = 'panel_settings'", [], |r| r.get(0))?;
        if has_settings == 0 {
            let oidc_issuer = std::env::var("GOLDSMP_OIDC_ISSUER_URL").unwrap_or_default();
            let oidc_cid = std::env::var("GOLDSMP_OIDC_CLIENT_ID")
                .or_else(|_| std::env::var("GOLDSMP_OAUTH_CLIENT_ID"))
                .unwrap_or_default();
            let oidc_sec = std::env::var("GOLDSMP_OIDC_CLIENT_SECRET")
                .or_else(|_| std::env::var("GOLDSMP_OAUTH_CLIENT_SECRET"))
                .unwrap_or_default();
            let default_settings = PanelSettings {
                panel_name: "GOLDSMP".into(),
                accent_color: "#f5b932".into(),
                logo_url: "".into(),
                oidc_enabled: !oidc_issuer.trim().is_empty() && !oidc_cid.trim().is_empty(),
                oidc_issuer_url: oidc_issuer,
                oidc_client_id: oidc_cid,
                oidc_client_secret: oidc_sec,
                metrics_retention_days: 30,
                incoming_webhook_token: Uuid::new_v4().to_string().replace('-', ""),
            };
            conn.execute(
                "INSERT INTO kv_settings (key, value) VALUES ('panel_settings', ?1)",
                params![serde_json::to_string(&default_settings).unwrap()],
            )?;
        }
        Ok(())
    }

    pub fn log_audit(&self, username: &str, action: &str, target: &str, details: &str, ip: &str) {
        let conn = self.conn();
        let _ = conn.execute(
            "INSERT INTO audit_logs (timestamp, username, action, target, details, ip_address)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![Utc::now().to_rfc3339(), username, action, target, details, ip],
        );
    }

    pub fn get_panel_settings(&self) -> PanelSettings {
        let conn = self.conn();
        let val: Result<String> = conn.query_row(
            "SELECT value FROM kv_settings WHERE key = 'panel_settings'",
            [],
            |r| r.get(0),
        );
        val.ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| PanelSettings {
                panel_name: "GOLDSMP".into(),
                accent_color: "#f59e0b".into(),
                logo_url: "".into(),
                oidc_enabled: false,
                oidc_issuer_url: "".into(),
                oidc_client_id: "".into(),
                oidc_client_secret: "".into(),
                metrics_retention_days: 30,
                incoming_webhook_token: Uuid::new_v4().to_string().replace('-', ""),
            })
    }

    pub fn save_panel_settings(&self, settings: &PanelSettings) -> Result<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT OR REPLACE INTO kv_settings (key, value) VALUES ('panel_settings', ?1)",
            params![serde_json::to_string(settings).unwrap()],
        )?;
        Ok(())
    }

    pub fn get_user_by_username(&self, username: &str) -> Option<(User, String)> {
        let conn = self.conn();
        conn.query_row(
            "SELECT id, username, email, password_hash, role, permissions_json, oidc_subject, created_at, last_login
             FROM users WHERE username = ?1",
            params![username],
            |row| {
                let perms_str: String = row.get(5)?;
                let permissions: Vec<String> = serde_json::from_str(&perms_str).unwrap_or_default();
                let user = User {
                    id: row.get(0)?,
                    username: row.get(1)?,
                    email: row.get(2)?,
                    role: row.get(4)?,
                    permissions,
                    oidc_subject: row.get(6)?,
                    created_at: row.get(7)?,
                    last_login: row.get(8)?,
                };
                let password_hash: String = row.get(3)?;
                Ok((user, password_hash))
            },
        )
        .ok()
    }
}
