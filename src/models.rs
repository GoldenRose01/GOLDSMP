use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub email: String,
    pub role: String, // "superadmin" | "admin" | "operator" | "viewer"
    pub permissions: Vec<String>,
    pub oidc_subject: Option<String>,
    pub created_at: String,
    pub last_login: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUserReq {
    pub username: String,
    pub email: String,
    pub password: String,
    pub role: String,
    pub permissions: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginReq {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRes {
    pub token: String,
    pub user: User,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateSettingField {
    pub key: String,
    pub label: String,
    pub description: String,
    pub field_type: String, // "string" | "number" | "boolean" | "select"
    pub default_value: String,
    pub options: Option<Vec<String>>,
    pub category: String,
    pub config_file: Option<String>, // e.g. "server.properties"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppTemplate {
    pub id: String,
    pub name: String,
    pub category: String, // "minecraft" | "steamcmd" | "generic"
    pub description: String,
    pub icon: String,
    pub default_port: u16,
    pub default_executable: String,
    pub default_args: Vec<String>,
    pub stop_command: String,
    pub steam_app_id: Option<u32>,
    pub supports_modrinth: bool,
    pub mod_folder: Option<String>,
    pub log_patterns: LogPatterns,
    pub settings_schema: Vec<TemplateSettingField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogPatterns {
    pub player_join: Option<String>,
    pub player_leave: Option<String>,
    pub server_ready: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub template_id: String,
    pub category: String,
    pub status: String, // "stopped" | "starting" | "running" | "stopping" | "crashed"
    pub port: u16,
    pub work_dir: String,
    pub executable: String,
    pub args: Vec<String>,
    pub env_vars: HashMap<String, String>,
    pub settings: HashMap<String, String>,
    pub auto_start: bool,
    pub auto_restart: bool,
    pub max_memory_mb: u64,
    pub max_players: u32,
    pub pid: Option<u32>,
    pub cpu_percent: f32,
    pub memory_mb: f64,
    pub disk_mb: f64,
    pub active_players: Vec<String>,
    pub uptime_secs: u64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateInstanceReq {
    pub name: String,
    pub template_id: String,
    pub port: Option<u16>,
    pub max_memory_mb: Option<u64>,
    pub max_players: Option<u32>,
    pub auto_start: Option<bool>,
    pub auto_restart: Option<bool>,
    pub executable: Option<String>,
    pub args: Option<Vec<String>>,
    pub settings: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInstanceReq {
    pub name: Option<String>,
    pub port: Option<u16>,
    pub executable: Option<String>,
    pub args: Option<Vec<String>>,
    pub env_vars: Option<HashMap<String, String>>,
    pub settings: Option<HashMap<String, String>>,
    pub auto_start: Option<bool>,
    pub auto_restart: Option<bool>,
    pub max_memory_mb: Option<u64>,
    pub max_players: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsoleLine {
    pub timestamp: String,
    pub stream: String, // "stdout" | "stderr" | "system" | "stdin"
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricPoint {
    pub timestamp: i64,
    pub instance_id: String, // "host" for host-level metrics
    pub cpu_percent: f64,
    pub memory_mb: f64,
    pub disk_mb: f64,
    pub net_rx_kb: f64,
    pub net_tx_kb: f64,
    pub players_online: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerSession {
    pub id: String,
    pub instance_id: String,
    pub instance_name: String,
    pub player_name: String,
    pub player_uuid: Option<String>,
    pub ip_address: String,
    pub country_code: String,
    pub country_name: String,
    pub joined_at: String,
    pub left_at: Option<String>,
    pub duration_secs: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerLeaderboardEntry {
    pub player_name: String,
    pub total_sessions: i64,
    pub total_playtime_secs: i64,
    pub last_seen: String,
    pub country_code: String,
    pub country_name: String,
    pub favorite_instance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoStat {
    pub country_code: String,
    pub country_name: String,
    pub unique_players: i64,
    pub total_sessions: i64,
    pub total_playtime_secs: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoFilterRule {
    pub id: String,
    pub instance_id: String, // "*" for global
    pub country_code: String,
    pub country_name: String,
    pub action: String, // "allow" | "block"
    pub enabled: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateGeoRuleReq {
    pub instance_id: String,
    pub country_code: String,
    pub country_name: String,
    pub action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTask {
    pub id: String,
    pub instance_id: String,
    pub name: String,
    pub trigger_type: String, // "interval" | "event"
    pub interval_secs: Option<i64>,
    pub event_name: Option<String>, // "player_join" | "player_leave" | "server_start" | "server_stop" | "server_crash"
    pub action_type: String, // "command" | "restart" | "backup" | "webhook"
    pub action_payload: String,
    pub enabled: bool,
    pub last_run: Option<String>,
    pub run_count: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTaskReq {
    pub instance_id: String,
    pub name: String,
    pub trigger_type: String,
    pub interval_secs: Option<i64>,
    pub event_name: Option<String>,
    pub action_type: String,
    pub action_payload: String,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupEntry {
    pub id: String,
    pub instance_id: String,
    pub name: String,
    pub filename: String,
    pub size_bytes: u64,
    pub created_at: String,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    pub id: String,
    pub name: String,
    pub url: String,
    pub events: Vec<String>,
    pub enabled: bool,
    pub secret: String,
    pub last_triggered: Option<String>,
    pub last_status: Option<i32>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWebhookReq {
    pub name: String,
    pub url: String,
    pub events: Vec<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub id: i64,
    pub timestamp: String,
    pub username: String,
    pub action: String,
    pub target: String,
    pub details: String,
    pub ip_address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostSystemInfo {
    pub hostname: String,
    pub os_name: String,
    pub os_version: String,
    pub kernel_version: String,
    pub cpu_brand: String,
    pub cpu_cores: usize,
    pub cpu_usage_percent: f32,
    pub total_memory_mb: u64,
    pub used_memory_mb: u64,
    pub total_disk_gb: f64,
    pub used_disk_gb: f64,
    pub uptime_secs: u64,
    pub panel_memory_mb: f64,
    pub panel_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelSettings {
    pub panel_name: String,
    pub accent_color: String,
    pub logo_url: String,
    pub oidc_enabled: bool,
    pub oidc_issuer_url: String,
    pub oidc_client_id: String,
    pub oidc_client_secret: String,
    pub metrics_retention_days: i64,
    pub incoming_webhook_token: String,
}
