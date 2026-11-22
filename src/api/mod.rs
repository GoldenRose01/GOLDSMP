pub mod analytics_routes;
pub mod auth_routes;
pub mod file_routes;
pub mod instance_routes;
pub mod ops_routes;

use crate::state::AppState;
use axum::{
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Router,
};
use rust_embed::RustEmbed;
use tower_http::cors::{Any, CorsLayer};

#[derive(RustEmbed)]
#[folder = "frontend/dist"]
struct FrontendAssets;

async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let file_path = if path.is_empty() { "index.html" } else { path };

    if let Some(content) = FrontendAssets::get(file_path) {
        let mime = mime_guess::from_path(file_path).first_or_octet_stream();
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, mime.as_ref().to_string())],
            content.data.into_owned(),
        )
            .into_response();
    }

    // SPA fallback to index.html
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
        // Modrinth Store
        .route("/api/mods/search", get(ops_routes::search_mods))
        .route("/api/instances/:id/mods/install", post(ops_routes::install_mod))
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
        // Embedded SPA Frontend
        .fallback(static_handler)
        .layer(cors)
        .with_state(state)
}
