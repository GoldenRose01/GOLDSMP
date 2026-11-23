pub mod analytics_routes;
pub mod auth_routes;
pub mod file_routes;
pub mod instance_routes;
pub mod ops_routes;

use crate::state::AppState;
use axum::{
    extract::State,
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Router,
};
use rust_embed::RustEmbed;
use tower_http::cors::{Any, CorsLayer};

#[derive(RustEmbed)]
#[folder = "frontend/dist"]
struct FrontendAssets;

async fn static_handler(State(state): State<AppState>, uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let file_path = if path.is_empty() { "index.html" } else { path };

    // 1. Check zero-downtime hot-swapped UI directory first (downloaded by AutoUpdater)
    let override_dir = state.data_dir.join("ui-override");
    if override_dir.exists() && !file_path.contains("..") {
        let candidate = override_dir.join(file_path);
        if candidate.is_file() {
            if let Ok(bytes) = std::fs::read(&candidate) {
                let mime = mime_guess::from_path(file_path).first_or_octet_stream();
                return (
                    StatusCode::OK,
                    [
                        (header::CONTENT_TYPE, mime.as_ref().to_string()),
                        (header::CACHE_CONTROL, "no-cache".to_string()),
                    ],
                    bytes,
                )
                    .into_response();
            }
        }
        // SPA fallback from hot-swapped UI
        let override_index = override_dir.join("index.html");
        if override_index.is_file() {
            if let Ok(bytes) = std::fs::read(&override_index) {
                return (
                    StatusCode::OK,
                    [
                        (header::CONTENT_TYPE, "text/html; charset=utf-8".to_string()),
                        (header::CACHE_CONTROL, "no-cache".to_string()),
                    ],
                    bytes,
                )
                    .into_response();
            }
        }
    }

    // 2. Fallback to embedded frontend compiled into the Rust binary
    if let Some(content) = FrontendAssets::get(file_path) {
        let mime = mime_guess::from_path(file_path).first_or_octet_stream();
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, mime.as_ref().to_string())],
            content.data.into_owned(),
        )
            .into_response();
    }

    // SPA fallback to embedded index.html
    if let Some(index) = FrontendAssets::get("index.html") {
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/html; charset=utf-8".to_string())],
            index.data.into_owned(),
        )
            .into_response();
    }

    (
        StatusCode::NOT_FOUND,
        "Frontend non compilato. Esegui 'npm run build' nella cartella frontend/.",
    )
        .into_response()
}

pub fn build_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // Auth & Users
        .route("/api/auth/login", post(auth_routes::login))
        .route("/api/auth/me", get(auth_routes::me))
        .route("/api/users", get(auth_routes::list_users).post(auth_routes::create_user))
        .route("/api/users/:id", delete(auth_routes::delete_user))
        .route("/api/users/:id/instances", post(auth_routes::grant_user_instance))
        .route(
            "/api/users/:id/instances/:instance_id",
            delete(auth_routes::revoke_user_instance),
        )
        // System & Settings
        .route("/api/system/info", get(ops_routes::system_info))
        .route("/api/templates", get(ops_routes::list_templates))
        .route("/api/settings", get(ops_routes::get_settings).put(ops_routes::update_settings))
        // Instances
        .route(
            "/api/instances",
            get(instance_routes::list_instances).post(instance_routes::create_instance),
        )
        .route(
            "/api/instances/:id",
            get(instance_routes::get_instance)
                .put(instance_routes::update_instance)
                .delete(instance_routes::delete_instance),
        )
        .route("/api/instances/:id/start", post(instance_routes::start_instance))
        .route("/api/instances/:id/stop", post(instance_routes::stop_instance))
        .route("/api/instances/:id/restart", post(instance_routes::restart_instance))
        .route("/api/instances/:id/kill", post(instance_routes::kill_instance))
        .route("/api/instances/:id/command", post(instance_routes::send_command))
        .route("/api/instances/:id/console", get(instance_routes::console_ws))
        // File Manager
        .route("/api/instances/:id/files", get(file_routes::list_files).delete(file_routes::delete_file))
        .route(
            "/api/instances/:id/files/content",
            get(file_routes::read_file).put(file_routes::write_file),
        )
        .route("/api/instances/:id/files/mkdir", post(file_routes::create_dir))
        .route("/api/instances/:id/files/rename", post(file_routes::rename_file))
        .route("/api/instances/:id/files/download-url", post(file_routes::download_from_url))
        // Per-Instance FTP Access, Horizon MC 1-Click Installer & GitHub WebService Auto-Deploy
        .route("/api/instances/github-deploy", post(instance_routes::quick_deploy_github_repo))
        .route("/api/instances/:id/ftp", get(instance_routes::get_ftp_info))
        .route("/api/instances/:id/ftp/rotate", post(instance_routes::rotate_ftp_password))
        .route("/api/instances/:id/horizon/install", post(instance_routes::install_horizon_server))
        .route("/api/instances/:id/minecraft/install-jar", post(instance_routes::install_minecraft_jar))
        .route("/api/instances/:id/github/sync", post(instance_routes::sync_github_webservice))
        // Backups
        .route(
            "/api/instances/:id/backups",
            get(ops_routes::list_backups).post(ops_routes::create_backup),
        )
        .route(
            "/api/instances/:id/backups/:backup_id/restore",
            post(ops_routes::restore_backup),
        )
        .route(
            "/api/instances/:id/backups/:backup_id",
            delete(ops_routes::delete_backup),
        )
        .route(
            "/api/instances/:id/backups/:backup_id/upload-gdrive",
            post(ops_routes::upload_backup_to_gdrive),
        )
        // Google Drive Cloud Backup & OAuth2 Login
        .route("/api/gdrive/status", get(ops_routes::gdrive_status))
        .route("/api/gdrive/config", put(ops_routes::gdrive_update_config))
        .route("/api/gdrive/auth-url", get(ops_routes::gdrive_auth_url))
        .route("/api/gdrive/callback", get(ops_routes::gdrive_callback))
        .route("/api/gdrive/disconnect", post(ops_routes::gdrive_disconnect))
        // Modrinth Store & Installed Mods Folder Manager / Auto-Updater
        .route("/api/mods/search", get(ops_routes::search_mods))
        .route("/api/instances/:id/mods/install", post(ops_routes::install_mod))
        .route(
            "/api/instances/:id/mods/installed",
            get(ops_routes::list_installed_mods).delete(ops_routes::delete_installed_mod),
        )
        .route("/api/instances/:id/mods/toggle", post(ops_routes::toggle_installed_mod))
        .route("/api/instances/:id/mods/update", post(ops_routes::update_installed_mods))
        // Analytics, Telemetry, Geo-Filtering & Audit
        .route("/api/analytics/metrics", get(analytics_routes::get_metrics))
        .route("/api/analytics/overview", get(analytics_routes::get_analytics_overview))
        .route("/api/analytics/export.csv", get(analytics_routes::export_sessions_csv))
        .route(
            "/api/analytics/geo-rules",
            get(analytics_routes::list_geo_rules).post(analytics_routes::create_geo_rule),
        )
        .route("/api/analytics/geo-rules/:id", delete(analytics_routes::delete_geo_rule))
        .route("/api/audit-logs", get(analytics_routes::list_audit_logs))
        // Scheduler
        .route("/api/tasks", get(ops_routes::list_tasks).post(ops_routes::create_task))
        .route("/api/tasks/:id", delete(ops_routes::delete_task))
        .route("/api/tasks/:id/run", post(ops_routes::run_task_now))
        // Webhooks
        .route("/api/webhooks", get(ops_routes::list_webhooks).post(ops_routes::create_webhook))
        .route("/api/webhooks/:id", delete(ops_routes::delete_webhook))
        .route("/api/webhooks/:id/test", post(ops_routes::test_webhook))
        .route("/api/webhooks/incoming/:token", post(ops_routes::incoming_webhook))
        // Zero-Downtime Auto-Updater
        .route("/api/updater/status", get(ops_routes::updater_status))
        .route("/api/updater/check", post(ops_routes::updater_check))
        .route("/api/updater/apply", post(ops_routes::updater_apply))
        .route("/api/updater/config", put(ops_routes::updater_config))
        // Embedded SPA Frontend (with zero-downtime hot-swap override)
        .fallback(static_handler)
        .layer(axum::extract::DefaultBodyLimit::max(10 * 1024 * 1024))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            crate::security::security_middleware,
        ))
        .layer(cors)
        .with_state(state)
}
