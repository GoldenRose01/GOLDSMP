use crate::analytics::get_host_system_info;
use crate::auth::AuthUser;
use crate::backups;
use crate::models::{
    AppTemplate, BackupEntry, CreateTaskReq, CreateWebhookReq, HostSystemInfo, PanelSettings,
    ScheduledTask, WebhookConfig,
};
use crate::modrinth::{self, ModrinthHit};
use crate::scheduler;
use crate::state::AppState;
use crate::templates::{builtin_templates, find_template};
use crate::webhooks;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use rusqlite::params;
use serde::Deserialize;
use std::path::PathBuf;
use uuid::Uuid;

// --- System & Settings ---

pub async fn system_info(_auth: AuthUser) -> Json<HostSystemInfo> {
    Json(get_host_system_info())
}

pub async fn list_templates(_auth: AuthUser) -> Json<Vec<AppTemplate>> {
    Json(builtin_templates())
}

pub async fn get_settings(State(state): State<AppState>) -> Json<PanelSettings> {
    // Publicly readable branding subset (mask secret if needed, or full for admin)
    Json(state.db.get_panel_settings())
}

pub async fn update_settings(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(settings): Json<PanelSettings>,
) -> Result<Json<PanelSettings>, (StatusCode, String)> {
    state
        .db
        .save_panel_settings(&settings)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    state.db.log_audit(
        &auth.0.username,
        "SETTINGS_UPDATE",
        "panel",
        &format!("Branding/OIDC aggiornati ({})", settings.panel_name),
        "local",
    );
    Ok(Json(settings))
}

// --- Scheduler ---

#[derive(Deserialize)]
pub struct TaskQuery {
    pub instance_id: Option<String>,
}

pub async fn list_tasks(
    State(state): State<AppState>,
    _auth: AuthUser,
    Query(q): Query<TaskQuery>,
) -> Json<Vec<ScheduledTask>> {
    Json(scheduler::list_tasks(&state.db, q.instance_id.as_deref()))
}

pub async fn create_task(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CreateTaskReq>,
) -> Result<Json<ScheduledTask>, (StatusCode, String)> {
    let task = ScheduledTask {
        id: Uuid::new_v4().to_string(),
        instance_id: req.instance_id,
        name: req.name,
        trigger_type: req.trigger_type,
        interval_secs: req.interval_secs,
        event_name: req.event_name,
        action_type: req.action_type,
        action_payload: req.action_payload,
        enabled: req.enabled.unwrap_or(true),
        last_run: None,
        run_count: 0,
        created_at: Utc::now().to_rfc3339(),
    };
    {
        let conn = state.db.conn();
        conn.execute(
            "INSERT INTO scheduled_tasks (id, instance_id, name, trigger_type, interval_secs, event_name, action_type, action_payload, enabled, run_count, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0, ?10)",
            params![
                task.id,
                task.instance_id,
                task.name,
                task.trigger_type,
                task.interval_secs,
                task.event_name,
                task.action_type,
                task.action_payload,
                if task.enabled { 1 } else { 0 },
                task.created_at
            ],
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    state.db.log_audit(
        &auth.0.username,
        "TASK_CREATE",
        &task.name,
        &format!("Azione: {}", task.action_type),
        "local",
    );
    Ok(Json(task))
}

pub async fn delete_task(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    {
        let conn = state.db.conn();
        conn.execute("DELETE FROM scheduled_tasks WHERE id = ?1", params![id])
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    state
        .db
        .log_audit(&auth.0.username, "TASK_DELETE", &id, "Task eliminato", "local");
    Ok(StatusCode::NO_CONTENT)
}

pub async fn run_task_now(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let tasks = scheduler::list_tasks(&state.db, None);
    let task = tasks
        .into_iter()
        .find(|t| t.id == id)
        .ok_or((StatusCode::NOT_FOUND, "Task non trovato".into()))?;
    scheduler::execute_task(&state.db, &state.pm, &state.data_dir, &state.http, &task).await;
    state.db.log_audit(
        &auth.0.username,
        "TASK_RUN",
        &task.name,
        "Esecuzione manuale",
        "local",
    );
    Ok(StatusCode::OK)
}

// --- Backups ---

#[derive(Deserialize)]
pub struct CreateBackupReq {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub notes: String,
}

pub async fn list_backups(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<String>,
) -> Json<Vec<BackupEntry>> {
    Json(backups::list_backups(&state.db, &id))
}

pub async fn create_backup(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(req): Json<CreateBackupReq>,
) -> Result<Json<BackupEntry>, (StatusCode, String)> {
    let inst = state
        .pm
        .load_instance(&id)
        .ok_or((StatusCode::NOT_FOUND, "Istanza non trovata".into()))?;
    let entry = backups::create_backup(
        &state.db,
        &state.data_dir,
        &inst.id,
        &PathBuf::from(&inst.work_dir),
        &req.name,
        &req.notes,
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let gcfg = crate::gdrive_backup::load_config(&state.db);
    let max_keep = gcfg.max_backups_per_instance.max(1);
    let pruned = backups::prune_local_backups(&state.db, &state.data_dir, &inst.id, max_keep);

    // Auto-upload to Google Drive (<nome-server>/<nome_istanza>_backup/) and prune oldest on Drive
    if gcfg.enabled && gcfg.connected {
        let db_c = state.db.clone();
        let http_c = state.http.clone();
        let inst_name = inst.name.clone();
        let local_path = backups::backup_dir(&state.data_dir, &inst.id).join(&entry.filename);
        let fname = entry.filename.clone();
        tokio::spawn(async move {
            match crate::gdrive_backup::upload_backup_and_prune(
                &db_c,
                &http_c,
                &inst_name,
                &local_path,
                &fname,
            )
            .await
            {
                Ok(remote_path) => {
                    db_c.log_audit(
                        "gdrive-sync",
                        "BACKUP_GDRIVE_UPLOAD",
                        &inst_name,
                        &format!("Caricato su Google Drive: {} (max {} mantenuti)", remote_path, max_keep),
                        "cloud",
                    );
                }
                Err(e) => {
                    db_c.log_audit(
                        "gdrive-sync",
                        "BACKUP_GDRIVE_ERROR",
                        &inst_name,
                        &e,
                        "cloud",
                    );
                }
            }
        });
    }

    state.db.log_audit(
        &auth.0.username,
        "BACKUP_CREATE",
        &inst.name,
        &format!(
            "{} ({} bytes) | Retention max {}: {} vecchi rimossi",
            entry.filename, entry.size_bytes, max_keep, pruned
        ),
        "local",
    );
    Ok(Json(entry))
}

pub async fn restore_backup(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((id, backup_id)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, String)> {
    let inst = state
        .pm
        .load_instance(&id)
        .ok_or((StatusCode::NOT_FOUND, "Istanza non trovata".into()))?;
    let list = backups::list_backups(&state.db, &id);
    let b = list
        .into_iter()
        .find(|x| x.id == backup_id)
        .ok_or((StatusCode::NOT_FOUND, "Backup non trovato".into()))?;

    backups::restore_backup(
        &state.data_dir,
        &id,
        &b.filename,
        &PathBuf::from(&inst.work_dir),
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    state.db.log_audit(
        &auth.0.username,
        "BACKUP_RESTORE",
        &inst.name,
        &format!("Ripristinato {}", b.filename),
        "local",
    );
    Ok(StatusCode::OK)
}

pub async fn delete_backup(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((id, backup_id)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, String)> {
    backups::delete_backup(&state.db, &state.data_dir, &id, &backup_id)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    state.db.log_audit(
        &auth.0.username,
        "BACKUP_DELETE",
        &id,
        &format!("Backup {} eliminato", backup_id),
        "local",
    );
    Ok(StatusCode::NO_CONTENT)
}

// --- Webhooks ---

pub async fn list_webhooks(
    State(state): State<AppState>,
    _auth: AuthUser,
) -> Json<Vec<WebhookConfig>> {
    Json(webhooks::list_webhooks(&state.db))
}

pub async fn create_webhook(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CreateWebhookReq>,
) -> Result<Json<WebhookConfig>, (StatusCode, String)> {
    let hook = WebhookConfig {
        id: Uuid::new_v4().to_string(),
        name: req.name,
        url: req.url,
        events: req.events,
        enabled: req.enabled.unwrap_or(true),
        secret: Uuid::new_v4().to_string().replace('-', ""),
        last_triggered: None,
        last_status: None,
        created_at: Utc::now().to_rfc3339(),
    };
    {
        let conn = state.db.conn();
        conn.execute(
            "INSERT INTO webhooks (id, name, url, events_json, enabled, secret, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                hook.id,
                hook.name,
                hook.url,
                serde_json::to_string(&hook.events).unwrap(),
                if hook.enabled { 1 } else { 0 },
                hook.secret,
                hook.created_at
            ],
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    state
        .db
        .log_audit(&auth.0.username, "WEBHOOK_CREATE", &hook.name, &hook.url, "local");
    Ok(Json(hook))
}

pub async fn delete_webhook(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    {
        let conn = state.db.conn();
        conn.execute("DELETE FROM webhooks WHERE id = ?1", params![id])
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    state
        .db
        .log_audit(&auth.0.username, "WEBHOOK_DELETE", &id, "Webhook rimosso", "local");
    Ok(StatusCode::NO_CONTENT)
}

pub async fn test_webhook(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let hooks = webhooks::list_webhooks(&state.db);
    let _hook = hooks
        .into_iter()
        .find(|h| h.id == id)
        .ok_or((StatusCode::NOT_FOUND, "Webhook non trovato".into()))?;
    webhooks::dispatch_event(
        state.db.clone(),
        state.http.clone(),
        "test_ping".into(),
        "system".into(),
        "GOLDSMP".into(),
        "Messaggio di test inviato da GOLDSMP!".into(),
    );
    Ok(StatusCode::OK)
}

#[derive(Deserialize)]
pub struct IncomingWebhookReq {
    pub instance_id: String,
    pub action: String, // "start" | "stop" | "restart" | "command" | "backup"
    #[serde(default)]
    pub payload: String,
}

pub async fn incoming_webhook(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Json(req): Json<IncomingWebhookReq>,
) -> Result<StatusCode, (StatusCode, String)> {
    let settings = state.db.get_panel_settings();
    if token != settings.incoming_webhook_token {
        return Err((StatusCode::UNAUTHORIZED, "Token webhook non valido".into()));
    }
    match req.action.as_str() {
        "start" => {
            let _ = state.pm.start_instance(&req.instance_id).await;
        }
        "stop" => {
            let _ = state.pm.stop_instance(&req.instance_id).await;
        }
        "restart" => {
            let _ = state.pm.restart_instance(&req.instance_id).await;
        }
        "command" => {
            let _ = state.pm.send_command_sync(&req.instance_id, &req.payload);
        }
        "backup" => {
            if let Some(inst) = state.pm.load_instance(&req.instance_id) {
                let _ = backups::create_backup(
                    &state.db,
                    &state.data_dir,
                    &inst.id,
                    &PathBuf::from(&inst.work_dir),
                    "Webhook Backup",
                    "Creato via Incoming Webhook",
                );
            }
        }
        _ => return Err((StatusCode::BAD_REQUEST, "Azione non supportata".into())),
    }
    state.db.log_audit(
        "webhook",
        "INCOMING_WEBHOOK",
        &req.instance_id,
        &format!("Azione: {} {}", req.action, req.payload),
        "webhook",
    );
    Ok(StatusCode::OK)
}

// --- Modrinth Store ---

#[derive(Deserialize)]
pub struct ModSearchQuery {
    #[serde(default)]
    pub q: String,
    #[serde(default = "default_mod_type")]
    pub project_type: String,
}
fn default_mod_type() -> String {
    "plugin".into()
}

pub async fn search_mods(
    State(state): State<AppState>,
    _auth: AuthUser,
    Query(q): Query<ModSearchQuery>,
) -> Result<Json<Vec<ModrinthHit>>, (StatusCode, String)> {
    let hits = modrinth::search_modrinth(&state.http, &q.q, &q.project_type)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e))?;
    Ok(Json(hits))
}

#[derive(Deserialize)]
pub struct InstallModReq {
    pub project_id: String,
    pub folder: Option<String>,
}

pub async fn install_mod(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(req): Json<InstallModReq>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let inst = state
        .pm
        .load_instance(&id)
        .ok_or((StatusCode::NOT_FOUND, "Istanza non trovata".into()))?;
    let default_folder = find_template(&inst.template_id)
        .and_then(|t| t.mod_folder)
        .unwrap_or_else(|| "plugins".into());
    let folder = req.folder.unwrap_or(default_folder);
    let target_dir = PathBuf::from(&inst.work_dir).join(&folder);

    let installed = modrinth::install_modrinth_project(&state.http, &req.project_id, &target_dir)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e))?;

    state.db.log_audit(
        &auth.0.username,
        "MOD_INSTALL",
        &inst.name,
        &format!("Installato {} in {}", installed, folder),
        "local",
    );

    Ok(Json(serde_json::json!({
        "installed": installed,
        "folder": folder
    })))
}

// --- Zero-Downtime Auto-Updater ---

pub async fn updater_status(
    State(state): State<AppState>,
    _auth: AuthUser,
) -> Json<crate::updater::UpdateStatus> {
    Json(state.updater.get_status())
}

pub async fn updater_check(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Json<crate::updater::UpdateStatus> {
    let auto_dl = state.updater.get_status().config.auto_download;
    let st = state.updater.check_and_maybe_download(auto_dl).await;
    state.db.log_audit(
        &auth.0.username,
        "UPDATER_CHECK",
        &st.config.repo,
        &format!("Stato: {} (latest: {:?})", st.state, st.latest_version),
        "local",
    );
    Json(st)
}

pub async fn updater_apply(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Json<crate::updater::UpdateStatus> {
    let st = state.updater.check_and_maybe_download(true).await;
    state.db.log_audit(
        &auth.0.username,
        "UPDATER_APPLY",
        &st.config.repo,
        &format!("UI hot-swap: {}, binary staged: {}", st.ui_hot_updated, st.binary_staged),
        "local",
    );
    Json(st)
}

pub async fn updater_config(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(cfg): Json<crate::updater::UpdaterConfig>,
) -> Result<Json<crate::updater::UpdateStatus>, (StatusCode, String)> {
    let st = state
        .updater
        .save_config(cfg)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    state.db.log_audit(
        &auth.0.username,
        "UPDATER_CONFIG",
        &st.config.repo,
        &format!(
            "auto_check={}, auto_download={}, interval={}m",
            st.config.auto_check, st.config.auto_download, st.config.check_interval_mins
        ),
        "local",
    );
    Ok(Json(st))
}

// --- Installed Mods Folder Manager & Modrinth Auto-Updater ---

#[derive(Deserialize)]
pub struct InstalledModsQuery {
    #[serde(default = "default_mods_folder")]
    pub folder: String,
    #[serde(default)]
    pub check_updates: bool,
}
fn default_mods_folder() -> String {
    "mods".into()
}

fn sanitize_mod_folder(folder: &str) -> String {
    match folder {
        "plugins" => "plugins".into(),
        "behavior_packs" => "behavior_packs".into(),
        _ => "mods".into(),
    }
}

pub async fn list_installed_mods(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Query(q): Query<InstalledModsQuery>,
) -> Result<Json<Vec<modrinth::InstalledModEntry>>, (StatusCode, String)> {
    if !auth.can_access_instance(&id) {
        return Err((StatusCode::FORBIDDEN, "Accesso negato".into()));
    }
    let inst = state
        .pm
        .load_instance(&id)
        .ok_or((StatusCode::NOT_FOUND, "Istanza non trovata".into()))?;
    let folder = sanitize_mod_folder(&q.folder);
    let list = modrinth::scan_installed_mods(
        &state.http,
        &PathBuf::from(&inst.work_dir),
        &folder,
        q.check_updates,
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(list))
}

#[derive(Deserialize)]
pub struct ToggleModReq {
    pub folder: String,
    pub filename: String,
}

pub async fn toggle_installed_mod(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(req): Json<ToggleModReq>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if !auth.can_access_instance(&id) {
        return Err((StatusCode::FORBIDDEN, "Accesso negato".into()));
    }
    if req.filename.contains("..") || req.filename.contains('/') || req.filename.contains('\\') {
        return Err((StatusCode::BAD_REQUEST, "Nome file non valido".into()));
    }
    let inst = state
        .pm
        .load_instance(&id)
        .ok_or((StatusCode::NOT_FOUND, "Istanza non trovata".into()))?;
    let folder = sanitize_mod_folder(&req.folder);
    let dir = PathBuf::from(&inst.work_dir).join(&folder);
    let src = dir.join(&req.filename);
    if !src.exists() {
        return Err((StatusCode::NOT_FOUND, "Mod non trovata".into()));
    }
    let new_name = if let Some(stripped) = req.filename.strip_suffix(".disabled") {
        stripped.to_string()
    } else {
        format!("{}.disabled", req.filename)
    };
    let dst = dir.join(&new_name);
    std::fs::rename(&src, &dst).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    state.db.log_audit(
        &auth.0.username,
        "MOD_TOGGLE",
        &inst.name,
        &format!("{} -> {}", req.filename, new_name),
        "local",
    );
    Ok(Json(serde_json::json!({ "filename": new_name })))
}

#[derive(Deserialize)]
pub struct DeleteModQuery {
    #[serde(default = "default_mods_folder")]
    pub folder: String,
    pub filename: String,
}

pub async fn delete_installed_mod(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Query(q): Query<DeleteModQuery>,
) -> Result<StatusCode, (StatusCode, String)> {
    if !auth.can_access_instance(&id) {
        return Err((StatusCode::FORBIDDEN, "Accesso negato".into()));
    }
    if q.filename.contains("..") || q.filename.contains('/') || q.filename.contains('\\') {
        return Err((StatusCode::BAD_REQUEST, "Nome file non valido".into()));
    }
    let inst = state
        .pm
        .load_instance(&id)
        .ok_or((StatusCode::NOT_FOUND, "Istanza non trovata".into()))?;
    let folder = sanitize_mod_folder(&q.folder);
    let target = PathBuf::from(&inst.work_dir).join(&folder).join(&q.filename);
    if target.exists() {
        std::fs::remove_file(&target)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    state.db.log_audit(
        &auth.0.username,
        "MOD_DELETE",
        &inst.name,
        &format!("Rimossa {}/{}", folder, q.filename),
        "local",
    );
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct UpdateModReq {
    #[serde(default = "default_mods_folder")]
    pub folder: String,
    pub filename: Option<String>,
    pub download_url: Option<String>,
    pub new_filename: Option<String>,
    #[serde(default)]
    pub update_all: bool,
}

pub async fn update_installed_mods(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(req): Json<UpdateModReq>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if !auth.can_access_instance(&id) {
        return Err((StatusCode::FORBIDDEN, "Accesso negato".into()));
    }
    let inst = state
        .pm
        .load_instance(&id)
        .ok_or((StatusCode::NOT_FOUND, "Istanza non trovata".into()))?;
    let folder = sanitize_mod_folder(&req.folder);
    let work_dir = PathBuf::from(&inst.work_dir);

    let mut updated_files: Vec<String> = Vec::new();

    if req.update_all {
        let scanned = modrinth::scan_installed_mods(&state.http, &work_dir, &folder, true)
            .await
            .map_err(|e| (StatusCode::BAD_GATEWAY, e))?;
        for m in scanned {
            if m.update_available {
                if let (Some(url), Some(new_fn)) = (m.latest_url, m.latest_filename) {
                    if let Ok(done) = modrinth::update_installed_mod(
                        &state.http,
                        &work_dir,
                        &folder,
                        &m.filename,
                        &url,
                        &new_fn,
                    )
                    .await
                    {
                        updated_files.push(done);
                    }
                }
            }
        }
    } else if let (Some(old_fn), Some(url), Some(new_fn)) =
        (req.filename, req.download_url, req.new_filename)
    {
        let done = modrinth::update_installed_mod(
            &state.http,
            &work_dir,
            &folder,
            &old_fn,
            &url,
            &new_fn,
        )
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e))?;
        updated_files.push(done);
    } else {
        return Err((StatusCode::BAD_REQUEST, "Parametri aggiornamento mancanti".into()));
    }

    state.db.log_audit(
        &auth.0.username,
        "MOD_AUTO_UPDATE",
        &inst.name,
        &format!("Aggiornate {} mod in {}/", updated_files.len(), folder),
        "local",
    );

    Ok(Json(serde_json::json!({
        "updated_count": updated_files.len(),
        "updated_files": updated_files
    })))
}

// --- Google Drive Cloud Backup & OAuth2 Login ---

pub async fn gdrive_status(
    State(state): State<AppState>,
    _auth: AuthUser,
) -> Json<crate::gdrive_backup::GDrivePublicStatus> {
    Json(crate::gdrive_backup::load_config(&state.db).into())
}

#[derive(Deserialize)]
pub struct UpdateGDriveConfigReq {
    pub enabled: bool,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub redirect_uri: String,
    pub server_folder_name: String,
    pub max_backups_per_instance: usize,
}

pub async fn gdrive_update_config(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<UpdateGDriveConfigReq>,
) -> Result<Json<crate::gdrive_backup::GDrivePublicStatus>, (StatusCode, String)> {
    let mut cfg = crate::gdrive_backup::load_config(&state.db);
    cfg.enabled = req.enabled;
    cfg.client_id = req.client_id.trim().to_string();
    if let Some(sec) = req.client_secret {
        if !sec.trim().is_empty() {
            cfg.client_secret = sec.trim().to_string();
        }
    }
    cfg.redirect_uri = req.redirect_uri.trim().to_string();
    cfg.server_folder_name = if req.server_folder_name.trim().is_empty() {
        "GOLDSMP-Server".into()
    } else {
        req.server_folder_name.trim().to_string()
    };
    cfg.max_backups_per_instance = req.max_backups_per_instance.max(1);

    crate::gdrive_backup::save_config(&state.db, &cfg)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    state.db.log_audit(
        &auth.0.username,
        "GDRIVE_CONFIG_UPDATE",
        &cfg.server_folder_name,
        &format!("Retention max {} backup/istanza", cfg.max_backups_per_instance),
        "local",
    );
    Ok(Json(cfg.into()))
}

pub async fn gdrive_auth_url(
    State(state): State<AppState>,
    _auth: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let cfg = crate::gdrive_backup::load_config(&state.db);
    let url = crate::gdrive_backup::build_oauth_url(&cfg)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    Ok(Json(serde_json::json!({ "url": url })))
}

#[derive(Deserialize)]
pub struct GDriveCallbackQuery {
    pub code: Option<String>,
    pub error: Option<String>,
}

pub async fn gdrive_callback(
    State(state): State<AppState>,
    Query(q): Query<GDriveCallbackQuery>,
) -> axum::response::Html<String> {
    if let Some(err) = q.error {
        return axum::response::Html(format!(
            "<html><body style='background:#0d0d12;color:#f87171;font-family:sans-serif;padding:40px'><h2>Accesso Google Drive annullato</h2><p>{}</p><a href='/' style='color:#f5b932'>Torna a GOLDSMP</a></body></html>",
            err
        ));
    }
    let Some(code) = q.code else {
        return axum::response::Html(
            "<html><body style='background:#0d0d12;color:#f87171;font-family:sans-serif;padding:40px'><h2>Codice OAuth mancante</h2><a href='/' style='color:#f5b932'>Torna a GOLDSMP</a></body></html>".into()
        );
    };

    match crate::gdrive_backup::exchange_code(&state.db, &state.http, &code).await {
        Ok(cfg) => {
            let email = cfg.connected_email.unwrap_or_else(|| "Account Google".into());
            state.db.log_audit(
                "oauth2",
                "GDRIVE_CONNECTED",
                &email,
                "Google Drive connesso per backup automatici",
                "cloud",
            );
            axum::response::Html(format!(
                "<html><body style='background:#0d0d12;color:#fff4d2;font-family:sans-serif;padding:40px;text-align:center'><h1 style='color:#f5b932'>Google Drive Connesso!</h1><p>Account collegato: <strong>{}</strong></p><p>I backup verranno salvati in <code>{}/&lt;nome_istanza&gt;_backup</code>.</p><script>setTimeout(()=>window.location.href='/', 2200);</script><a href='/' style='color:#38bdf8'>Torna subito a GOLDSMP</a></body></html>",
                email, cfg.server_folder_name
            ))
        }
        Err(e) => axum::response::Html(format!(
            "<html><body style='background:#0d0d12;color:#f87171;font-family:sans-serif;padding:40px'><h2>Errore connessione Google Drive</h2><pre>{}</pre><a href='/' style='color:#f5b932'>Torna a GOLDSMP</a></body></html>",
            e
        )),
    }
}

pub async fn gdrive_disconnect(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<crate::gdrive_backup::GDrivePublicStatus>, (StatusCode, String)> {
    let mut cfg = crate::gdrive_backup::load_config(&state.db);
    cfg.connected = false;
    cfg.connected_email = None;
    cfg.access_token.clear();
    cfg.refresh_token.clear();
    cfg.token_expires_at = 0;
    crate::gdrive_backup::save_config(&state.db, &cfg)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    state.db.log_audit(
        &auth.0.username,
        "GDRIVE_DISCONNECTED",
        "gdrive",
        "Account Google Drive scollegato",
        "local",
    );
    Ok(Json(cfg.into()))
}

pub async fn upload_backup_to_gdrive(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((id, backup_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if !auth.can_access_instance(&id) {
        return Err((StatusCode::FORBIDDEN, "Accesso negato".into()));
    }
    let inst = state
        .pm
        .load_instance(&id)
        .ok_or((StatusCode::NOT_FOUND, "Istanza non trovata".into()))?;
    let list = backups::list_backups(&state.db, &id);
    let b = list
        .into_iter()
        .find(|x| x.id == backup_id)
        .ok_or((StatusCode::NOT_FOUND, "Backup non trovato".into()))?;

    let local_path = backups::backup_dir(&state.data_dir, &id).join(&b.filename);
    let remote = crate::gdrive_backup::upload_backup_and_prune(
        &state.db,
        &state.http,
        &inst.name,
        &local_path,
        &b.filename,
    )
    .await
    .map_err(|e| (StatusCode::BAD_GATEWAY, e))?;

    Ok(Json(serde_json::json!({ "remote_path": remote })))
}



