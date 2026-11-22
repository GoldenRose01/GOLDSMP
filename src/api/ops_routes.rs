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

    state.db.log_audit(
        &auth.0.username,
        "BACKUP_CREATE",
        &inst.name,
        &format!("{} ({} bytes)", entry.filename, entry.size_bytes),
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

