use crate::db::Database;
use chrono::Utc;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GDriveConfig {
    pub enabled: bool,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub server_folder_name: String,
    pub max_backups_per_instance: usize,
    pub connected: bool,
    pub connected_email: Option<String>,
    pub access_token: String,
    pub refresh_token: String,
    pub token_expires_at: i64,
}

impl Default for GDriveConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            client_id: String::new(),
            client_secret: String::new(),
            redirect_uri: "http://localhost:8080/api/gdrive/callback".into(),
            server_folder_name: "GOLDSMP-Server".into(),
            max_backups_per_instance: 5,
            connected: false,
            connected_email: None,
            access_token: String::new(),
            refresh_token: String::new(),
            token_expires_at: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GDrivePublicStatus {
    pub enabled: bool,
    pub client_id: String,
    pub has_client_secret: bool,
    pub redirect_uri: String,
    pub server_folder_name: String,
    pub max_backups_per_instance: usize,
    pub connected: bool,
    pub connected_email: Option<String>,
}

impl From<GDriveConfig> for GDrivePublicStatus {
    fn from(c: GDriveConfig) -> Self {
        Self {
            enabled: c.enabled,
            client_id: c.client_id,
            has_client_secret: !c.client_secret.is_empty(),
            redirect_uri: c.redirect_uri,
            server_folder_name: c.server_folder_name,
            max_backups_per_instance: c.max_backups_per_instance,
            connected: c.connected,
            connected_email: c.connected_email,
        }
    }
}

pub fn load_config(db: &Database) -> GDriveConfig {
    let conn = db.conn();
    conn.query_row(
        "SELECT value FROM kv_settings WHERE key = 'gdrive_config'",
        [],
        |r| r.get::<_, String>(0),
    )
    .ok()
    .and_then(|s| serde_json::from_str(&s).ok())
    .unwrap_or_default()
}

pub fn save_config(db: &Database, cfg: &GDriveConfig) -> Result<(), String> {
    let conn = db.conn();
    conn.execute(
        "INSERT OR REPLACE INTO kv_settings (key, value) VALUES ('gdrive_config', ?1)",
        params![serde_json::to_string(cfg).unwrap()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn build_oauth_url(cfg: &GDriveConfig) -> Result<String, String> {
    if cfg.client_id.trim().is_empty() {
        return Err("Inserisci prima il Google OAuth Client ID nelle impostazioni Google Drive".into());
    }
    let scope = "https://www.googleapis.com/auth/drive.file%20https://www.googleapis.com/auth/userinfo.email";
    Ok(format!(
        "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri={}&response_type=code&scope={}&access_type=offline&prompt=consent",
        url_encode(cfg.client_id.trim()),
        url_encode(cfg.redirect_uri.trim()),
        scope
    ))
}

fn url_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: Option<i64>,
    refresh_token: Option<String>,
}

pub async fn exchange_code(
    db: &Database,
    http: &reqwest::Client,
    code: &str,
) -> Result<GDriveConfig, String> {
    let mut cfg = load_config(db);
    let resp = http
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("code", code),
            ("client_id", cfg.client_id.trim()),
            ("client_secret", cfg.client_secret.trim()),
            ("redirect_uri", cfg.redirect_uri.trim()),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await
        .map_err(|e| format!("Errore connessione OAuth Google: {}", e))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Google OAuth ha rifiutato il codice: {}", body));
    }

    let tok: TokenResponse = resp
        .json()
        .await
        .map_err(|e| format!("Errore parsing token Google: {}", e))?;

    cfg.access_token = tok.access_token.clone();
    if let Some(rt) = tok.refresh_token {
        cfg.refresh_token = rt;
    }
    cfg.token_expires_at = Utc::now().timestamp() + tok.expires_in.unwrap_or(3500) - 60;
    cfg.connected = true;

    // Fetch user email for display
    if let Ok(info_resp) = http
        .get("https://www.googleapis.com/oauth2/v2/userinfo")
        .bearer_auth(&cfg.access_token)
        .send()
        .await
    {
        if let Ok(j) = info_resp.json::<serde_json::Value>().await {
            if let Some(email) = j.get("email").and_then(|e| e.as_str()) {
                cfg.connected_email = Some(email.to_string());
            }
        }
    }

    save_config(db, &cfg)?;
    Ok(cfg)
}

async fn ensure_valid_access_token(
    db: &Database,
    http: &reqwest::Client,
) -> Result<(String, GDriveConfig), String> {
    let mut cfg = load_config(db);
    if !cfg.connected || cfg.access_token.is_empty() {
        return Err("Google Drive non connesso".into());
    }
    let now = Utc::now().timestamp();
    if now >= cfg.token_expires_at && !cfg.refresh_token.is_empty() {
        let resp = http
            .post("https://oauth2.googleapis.com/token")
            .form(&[
                ("client_id", cfg.client_id.trim()),
                ("client_secret", cfg.client_secret.trim()),
                ("refresh_token", cfg.refresh_token.trim()),
                ("grant_type", "refresh_token"),
            ])
            .send()
            .await
            .map_err(|e| format!("Errore refresh token Google: {}", e))?;
        if resp.status().is_success() {
            if let Ok(tok) = resp.json::<TokenResponse>().await {
                cfg.access_token = tok.access_token;
                cfg.token_expires_at = now + tok.expires_in.unwrap_or(3500) - 60;
                let _ = save_config(db, &cfg);
            }
        }
    }
    let token = cfg.access_token.clone();
    Ok((token, cfg))
}

async fn find_or_create_folder(
    http: &reqwest::Client,
    token: &str,
    folder_name: &str,
    parent_id: Option<&str>,
) -> Result<String, String> {
    let escaped = folder_name.replace('\'', "\\'");
    let q = match parent_id {
        Some(pid) => format!(
            "mimeType='application/vnd.google-apps.folder' and name='{}' and '{}' in parents and trashed=false",
            escaped, pid
        ),
        None => format!(
            "mimeType='application/vnd.google-apps.folder' and name='{}' and 'root' in parents and trashed=false",
            escaped
        ),
    };

    let list_resp = http
        .get("https://www.googleapis.com/drive/v3/files")
        .bearer_auth(token)
        .query(&[("q", q.as_str()), ("fields", "files(id,name)")])
        .send()
        .await
        .map_err(|e| format!("Errore ricerca cartella Drive: {}", e))?;

    if let Ok(json) = list_resp.json::<serde_json::Value>().await {
        if let Some(id) = json
            .get("files")
            .and_then(|f| f.as_array())
            .and_then(|arr| arr.first())
            .and_then(|first| first.get("id"))
            .and_then(|v| v.as_str())
        {
            return Ok(id.to_string());
        }
    }

    // Create folder
    let mut meta = serde_json::json!({
        "name": folder_name,
        "mimeType": "application/vnd.google-apps.folder"
    });
    if let Some(pid) = parent_id {
        meta["parents"] = serde_json::json!([pid]);
    }

    let create_resp = http
        .post("https://www.googleapis.com/drive/v3/files")
        .bearer_auth(token)
        .json(&meta)
        .send()
        .await
        .map_err(|e| format!("Errore creazione cartella Drive: {}", e))?;

    let created: serde_json::Value = create_resp
        .json()
        .await
        .map_err(|e| format!("Errore risposta creazione cartella: {}", e))?;

    created
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "Impossibile ottenere ID cartella Google Drive".to_string())
}

/// Uploads a backup file into `<server_folder_name>/<instance_name>_backup/` on Google Drive
/// and prunes older backups on Drive keeping only the newest `max_backups_per_instance`.
pub async fn upload_backup_and_prune(
    db: &Database,
    http: &reqwest::Client,
    instance_name: &str,
    local_file: &Path,
    filename: &str,
) -> Result<String, String> {
    let (token, cfg) = ensure_valid_access_token(db, http).await?;
    if !cfg.enabled {
        return Err("Upload Google Drive disabilitato".into());
    }

    let server_root = if cfg.server_folder_name.trim().is_empty() {
        "GOLDSMP-Server"
    } else {
        cfg.server_folder_name.trim()
    };
    let safe_inst: String = instance_name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let inst_folder_name = format!("{}_backup", safe_inst.trim_matches('_'));

    // 1. Ensure <nome-server>/<nome_istanza>_backup exists
    let root_id = find_or_create_folder(http, &token, server_root, None).await?;
    let inst_folder_id =
        find_or_create_folder(http, &token, &inst_folder_name, Some(&root_id)).await?;

    // 2. Upload file via multipart/related
    let file_bytes = std::fs::read(local_file).map_err(|e| e.to_string())?;
    let boundary = "goldsmp_drive_boundary_987654";
    let metadata = serde_json::json!({
        "name": filename,
        "parents": [inst_folder_id]
    });

    let mut body = Vec::new();
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Type: application/json; charset=UTF-8\r\n\r\n");
    body.extend_from_slice(metadata.to_string().as_bytes());
    body.extend_from_slice(format!("\r\n--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Type: application/gzip\r\n\r\n");
    body.extend_from_slice(&file_bytes);
    body.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());

    let up_resp = http
        .post("https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart")
        .bearer_auth(&token)
        .header(
            "Content-Type",
            format!("multipart/related; boundary={}", boundary),
        )
        .body(body)
        .send()
        .await
        .map_err(|e| format!("Errore upload su Google Drive: {}", e))?;

    if !up_resp.status().is_success() {
        let err_txt = up_resp.text().await.unwrap_or_default();
        return Err(format!("Google Drive upload fallito: {}", err_txt));
    }

    // 3. Enforce retention on Google Drive: keep only newest `max_backups_per_instance`
    let keep = cfg.max_backups_per_instance.max(1);
    let list_q = format!("'{}' in parents and trashed=false", inst_folder_id);
    if let Ok(list_resp) = http
        .get("https://www.googleapis.com/drive/v3/files")
        .bearer_auth(&token)
        .query(&[
            ("q", list_q.as_str()),
            ("orderBy", "createdTime desc"),
            ("fields", "files(id,name,createdTime)"),
        ])
        .send()
        .await
    {
        if let Ok(json) = list_resp.json::<serde_json::Value>().await {
            if let Some(files) = json.get("files").and_then(|f| f.as_array()) {
                for old in files.iter().skip(keep) {
                    if let Some(old_id) = old.get("id").and_then(|v| v.as_str()) {
                        let del_url = format!("https://www.googleapis.com/drive/v3/files/{}", old_id);
                        let _ = http.delete(&del_url).bearer_auth(&token).send().await;
                    }
                }
            }
        }
    }

    Ok(format!("{}/{}/{}", server_root, inst_folder_name, filename))
}
