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
    #[serde(default)]
    pub auth_mode: String, // "oauth2" | "gdrive_cli" | "direct_token"
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
            auth_mode: String::new(),
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
    pub auth_mode: String,
    pub cli_available: bool,
}

impl From<GDriveConfig> for GDrivePublicStatus {
    fn from(c: GDriveConfig) -> Self {
        let cli_avail = is_gdrive_cli_authenticated();
        Self {
            enabled: c.enabled,
            client_id: c.client_id,
            has_client_secret: !c.client_secret.is_empty(),
            redirect_uri: c.redirect_uri,
            server_folder_name: c.server_folder_name,
            max_backups_per_instance: c.max_backups_per_instance,
            connected: c.connected,
            connected_email: c.connected_email,
            auth_mode: c.auth_mode,
            cli_available: cli_avail,
        }
    }
}

pub fn is_gdrive_cli_authenticated() -> bool {
    std::process::Command::new("gdrive")
        .args(["auth", "status", "--json"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| serde_json::from_slice::<serde_json::Value>(&o.stdout).ok())
        .and_then(|j| j.get("authenticated").and_then(|b| b.as_bool()))
        .unwrap_or(false)
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

/// Renders an interactive Direct Login / Account Chooser HTML page (`/api/gdrive/login`)
/// that either redirects immediately to Google's official OAuth2 Account Chooser if Client ID is present,
/// or displays a self-contained Google Login Options portal with 1-Click Local Account detection,
/// OAuth2 Redirect Login, and Direct Google Token / OAuth Playground login.
pub fn build_direct_login_portal_html(cfg: &GDriveConfig) -> String {
    if !cfg.client_id.trim().is_empty() {
        if let Ok(oauth_url) = build_oauth_url(cfg) {
            return format!(
                r#"<!DOCTYPE html>
<html lang="it">
<head>
  <meta charset="UTF-8" />
  <title>Reindirizzamento al Login Google Drive — GOLDSMP</title>
  <meta http-equiv="refresh" content="0;url={oauth_url}" />
  <style>
    body {{ background:#0b0b10; color:#fff4d2; font-family:system-ui,sans-serif; display:flex; align-items:center; justify-content:center; height:100vh; margin:0; }}
    .box {{ text-align:center; padding:32px; border:1px solid rgba(245,185,50,0.45); border-radius:14px; background:#14141d; max-width:480px; }}
    a {{ color:#f5b932; }}
  </style>
</head>
<body>
  <div class="box">
    <h2 style="color:#f5b932;margin-top:0">Apertura Login Google Drive...</h2>
    <p>Verrai reindirizzato automaticamente alla pagina di scelta account di Google.</p>
    <p><a href="{oauth_url}">Clicca qui se non vieni reindirizzato</a></p>
  </div>
  <script>window.location.replace({oauth_url_js});</script>
</body>
</html>"#,
                oauth_url = oauth_url,
                oauth_url_js = serde_json::to_string(&oauth_url).unwrap_or_default()
            );
        }
    }

    let cli_ok = is_gdrive_cli_authenticated();
    let cli_badge = if cli_ok {
        r#"<span style="background:rgba(16,185,129,0.2);color:#34d399;padding:3px 9px;border-radius:99px;font-size:12px;border:1px solid rgba(16,185,129,0.4)">Account Google Drive Rilevato sul Server</span>"#
    } else {
        r#"<span style="background:rgba(245,185,50,0.16);color:#f5b932;padding:3px 9px;border-radius:99px;font-size:12px;border:1px solid rgba(245,185,50,0.4)">Scegli Metodo di Accesso</span>"#
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="it">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>Login Diretto Google Drive — GOLDSMP</title>
  <style>
    * {{ box-sizing: border-box; }}
    body {{
      margin: 0;
      min-height: 100vh;
      background: radial-gradient(circle at 50% 0%, rgba(245,185,50,0.15), #09090e 70%);
      color: #f3f4f6;
      font-family: system-ui, -apple-system, sans-serif;
      display: flex;
      align-items: center;
      justify-content: center;
      padding: 24px;
    }}
    .portal {{
      width: 100%;
      max-width: 640px;
      background: #13131c;
      border: 1px solid rgba(245,185,50,0.48);
      border-radius: 16px;
      padding: 28px;
      box-shadow: 0 24px 70px rgba(0,0,0,0.85), 0 0 36px rgba(245,185,50,0.12);
    }}
    h1 {{
      margin: 0 0 6px;
      font-size: 22px;
      color: #fff4d2;
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 10px;
      flex-wrap: wrap;
    }}
    p.sub {{ margin: 0 0 20px; font-size: 13.5px; color: #9ca3af; line-height: 1.45; }}
    .option-card {{
      background: rgba(255,255,255,0.03);
      border: 1px solid rgba(255,255,255,0.1);
      border-radius: 12px;
      padding: 16px;
      margin-bottom: 14px;
    }}
    .option-card.highlight {{
      border-color: rgba(56,189,248,0.45);
      background: linear-gradient(135deg, rgba(56,189,248,0.1), rgba(20,20,30,0.9));
    }}
    .option-title {{
      font-weight: 700;
      font-size: 15px;
      color: #fff4d2;
      margin-bottom: 6px;
      display: flex;
      align-items: center;
      justify-content: space-between;
    }}
    .option-desc {{ font-size: 12.5px; color: #9ca3af; margin-bottom: 12px; line-height: 1.4; }}
    input {{
      width: 100%;
      padding: 9px 12px;
      border-radius: 8px;
      border: 1px solid rgba(255,255,255,0.16);
      background: #0b0b12;
      color: #fff;
      font-size: 13px;
      margin-bottom: 8px;
    }}
    .btn {{
      display: inline-flex;
      align-items: center;
      justify-content: center;
      gap: 8px;
      padding: 10px 16px;
      border-radius: 8px;
      font-weight: 700;
      font-size: 13.5px;
      cursor: pointer;
      border: none;
      text-decoration: none;
      width: 100%;
    }}
    .btn-gold {{
      background: linear-gradient(135deg, #f5b932, #d97706);
      color: #09090e;
    }}
    .btn-cyan {{
      background: linear-gradient(135deg, #38bdf8, #0284c7);
      color: #09090e;
    }}
    .btn-outline {{
      background: transparent;
      border: 1px solid rgba(255,255,255,0.2);
      color: #e5e7eb;
      margin-top: 6px;
    }}
    code {{ color: #f5b932; font-size: 12px; }}
  </style>
</head>
<body>
  <div class="portal">
    <h1>
      <span>Accesso Diretto Google Drive</span>
      {cli_badge}
    </h1>
    <p class="sub">
      Collega il tuo Google Drive a <strong>GOLDSMP</strong>: i backup verranno caricati automaticamente in
      <code>{server_folder}/&lt;nome_istanza&gt;_backup/</code> mantenendo solo gli ultimi <strong>{max_keep}</strong> backup.
    </p>

    <!-- Opzione 1: 1-Click Direct Server Session / Plugin Google Drive -->
    <div class="option-card highlight">
      <div class="option-title">
        <span>1. Accesso Istantaneo 1-Click (Account Google Connesso)</span>
        <span style="color:#38bdf8;font-size:11.5px">Consigliato • Zero Config</span>
      </div>
      <div class="option-desc">
        Usa direttamente l'autenticazione Google Drive attiva sull'ambiente server senza dover inserire Client ID o Client Secret.
      </div>
      <form method="GET" action="/api/gdrive/quick-connect">
        <button type="submit" class="btn btn-cyan">
          Connetti Subito Account Google Drive (1-Click)
        </button>
      </form>
    </div>

    <!-- Opzione 2: Login OAuth2 Diretto Pagina Google -->
    <div class="option-card">
      <div class="option-title">
        <span>2. Pagina Login Ufficiale Google OAuth2 (Scelta Account)</span>
      </div>
      <div class="option-desc">
        Inserisci il tuo Google OAuth Client ID (e Secret) per aprire subito la schermata ufficiale di login Google (<code>accounts.google.com</code>) con redirect automatico a <code>{redirect_uri}</code>.
      </div>
      <form method="GET" action="/api/gdrive/direct-oauth">
        <input name="client_id" placeholder="Google OAuth2 Client ID (xxxx.apps.googleusercontent.com)" value="{client_id}" required />
        <input name="client_secret" type="password" placeholder="Google OAuth2 Client Secret" />
        <input name="redirect_uri" value="{redirect_uri}" placeholder="Redirect URI" />
        <button type="submit" class="btn btn-gold">
          Vai alla Pagina di Login Google &rarr;
        </button>
      </form>
    </div>

    <!-- Opzione 3: Token Diretto Google Drive -->
    <div class="option-card">
      <div class="option-title">
        <span>3. Login Rapido con Access Token Google Drive</span>
      </div>
      <div class="option-desc">
        Hai già un Access Token Google (es. da <a href="https://developers.google.com/oauthplayground/?scope=https://www.googleapis.com/auth/drive.file" target="_blank" style="color:#38bdf8">Google OAuth Playground</a>)? Incollalo qui per attivare subito Google Drive.
      </div>
      <form method="GET" action="/api/gdrive/token-login">
        <input name="access_token" type="password" placeholder="Incolla Access Token Google (ya29....)" required />
        <button type="submit" class="btn btn-outline">
          Attiva con Access Token Diretto
        </button>
      </form>
    </div>

    <a href="/" class="btn btn-outline" style="margin-top:4px">Torna al Pannello GOLDSMP</a>
  </div>
</body>
</html>"#,
        cli_badge = cli_badge,
        server_folder = cfg.server_folder_name,
        max_keep = cfg.max_backups_per_instance,
        redirect_uri = cfg.redirect_uri,
        client_id = cfg.client_id
    )
}

pub fn build_oauth_url(cfg: &GDriveConfig) -> Result<String, String> {
    if cfg.client_id.trim().is_empty() {
        return Err("Inserisci il Google OAuth Client ID oppure usa la pagina di Login Diretto Google Drive".into());
    }
    let scope = "https://www.googleapis.com/auth/drive.file%20https://www.googleapis.com/auth/userinfo.email";
    Ok(format!(
        "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri={}&response_type=code&scope={}&access_type=offline&prompt=select_account%20consent",
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

pub async fn connect_via_direct_token(
    db: &Database,
    http: &reqwest::Client,
    access_token: &str,
) -> Result<GDriveConfig, String> {
    let clean = access_token.trim();
    if clean.is_empty() {
        return Err("Access token vuoto".into());
    }
    let mut cfg = load_config(db);
    cfg.access_token = clean.to_string();
    cfg.token_expires_at = Utc::now().timestamp() + 3500;
    cfg.connected = true;
    cfg.auth_mode = "direct_token".into();

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
    if cfg.connected_email.is_none() {
        cfg.connected_email = Some("Account Google (Token Diretto)".into());
    }
    save_config(db, &cfg)?;
    Ok(cfg)
}

pub async fn connect_via_gdrive_cli(db: &Database) -> Result<GDriveConfig, String> {
    let mut cfg = load_config(db);
    if is_gdrive_cli_authenticated() {
        // Attempt to read user email from gdrive about/quota or status
        let mut email = "Google Drive (Sessione Server 1-Click)".to_string();
        if let Ok(out) = tokio::process::Command::new("gdrive")
            .args(["auth", "status", "--json"])
            .output()
            .await
        {
            if let Ok(j) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
                if let Some(em) = j.get("email").and_then(|s| s.as_str()).filter(|s| !s.is_empty()) {
                    email = em.to_string();
                }
            }
        }
        cfg.connected = true;
        cfg.connected_email = Some(email);
        cfg.auth_mode = "gdrive_cli".into();
        save_config(db, &cfg)?;
        return Ok(cfg);
    }
    Err("Nessuna sessione CLI pre-autenticata rilevata. Usa il Login OAuth2 o il Token Diretto.".into())
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
    cfg.auth_mode = "oauth2".into();

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

async fn upload_via_gdrive_cli(
    server_root: &str,
    inst_folder_name: &str,
    local_file: &Path,
    filename: &str,
    max_keep: usize,
) -> Result<String, String> {
    // 1. Find or create root folder
    let find_or_mkdir = |name: String, parent: Option<String>| async move {
        let mut search_args = vec![
            "readonly".to_string(),
            "search".to_string(),
            "--name-contains".to_string(),
            name.clone(),
            "--json".to_string(),
        ];
        if let Some(ref p) = parent {
            search_args.push("--parent".to_string());
            search_args.push(p.clone());
        }
        if let Ok(out) = tokio::process::Command::new("gdrive")
            .args(&search_args)
            .output()
            .await
        {
            if let Ok(j) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
                let items = j
                    .get("files")
                    .and_then(|f| f.as_array())
                    .or_else(|| j.as_array());
                if let Some(arr) = items {
                    for item in arr {
                        let item_name = item.get("name").and_then(|s| s.as_str()).unwrap_or("");
                        let mime = item.get("mimeType").and_then(|s| s.as_str()).unwrap_or("");
                        if item_name == name && (mime.contains("folder") || mime.is_empty()) {
                            if let Some(id) = item.get("id").and_then(|s| s.as_str()) {
                                return Ok(id.to_string());
                            }
                        }
                    }
                }
            }
        }

        let mut mkdir_args = vec![
            "mutate".to_string(),
            "mkdir".to_string(),
            name.clone(),
            "--json".to_string(),
        ];
        if let Some(ref p) = parent {
            mkdir_args.push("--parent".to_string());
            mkdir_args.push(p.clone());
        }
        let out = tokio::process::Command::new("gdrive")
            .args(&mkdir_args)
            .output()
            .await
            .map_err(|e| format!("gdrive mkdir fallito: {}", e))?;
        if let Ok(j) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
            if let Some(id) = j
                .get("id")
                .and_then(|s| s.as_str())
                .or_else(|| j.get("file").and_then(|f| f.get("id")).and_then(|s| s.as_str()))
            {
                return Ok(id.to_string());
            }
        }
        Err(format!("Impossibile creare cartella {} via gdrive CLI", name))
    };

    let root_id = find_or_mkdir(server_root.to_string(), None).await?;
    let inst_id = find_or_mkdir(inst_folder_name.to_string(), Some(root_id)).await?;

    // 2. Upload backup archive into inst_id
    let up_out = tokio::process::Command::new("gdrive")
        .args([
            "mutate",
            "upload",
            &local_file.to_string_lossy(),
            "--parent",
            &inst_id,
            "--json",
        ])
        .output()
        .await
        .map_err(|e| format!("gdrive upload fallito: {}", e))?;

    if !up_out.status.success() {
        return Err(String::from_utf8_lossy(&up_out.stderr).to_string());
    }

    // 3. List files in inst_id and trash oldest beyond max_keep
    if let Ok(ls_out) = tokio::process::Command::new("gdrive")
        .args(["readonly", "ls", "--parent", &inst_id, "--json"])
        .output()
        .await
    {
        if let Ok(j) = serde_json::from_slice::<serde_json::Value>(&ls_out.stdout) {
            if let Some(files) = j
                .get("files")
                .and_then(|f| f.as_array())
                .or_else(|| j.as_array())
            {
                for old in files.iter().skip(max_keep.max(1)) {
                    if let Some(old_id) = old.get("id").and_then(|s| s.as_str()) {
                        let _ = tokio::process::Command::new("gdrive")
                            .args(["mutate", "trash", old_id])
                            .output()
                            .await;
                    }
                }
            }
        }
    }

    Ok(format!("{}/{}/{}", server_root, inst_folder_name, filename))
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
    let cfg = load_config(db);
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

    // If connected via 1-Click gdrive CLI mode (or if access_token is empty and CLI is authenticated)
    if (cfg.auth_mode == "gdrive_cli" || cfg.access_token.is_empty())
        && is_gdrive_cli_authenticated()
    {
        return upload_via_gdrive_cli(
            server_root,
            &inst_folder_name,
            local_file,
            filename,
            cfg.max_backups_per_instance,
        )
        .await;
    }

    let (token, cfg) = ensure_valid_access_token(db, http).await?;

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
