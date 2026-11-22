use crate::auth::AuthUser;
use crate::models::{
    AuditLogEntry, CreateGeoRuleReq, GeoFilterRule, GeoStat, MetricPoint, PlayerLeaderboardEntry,
    PlayerSession,
};
use crate::state::AppState;
use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct MetricsQuery {
    #[serde(default = "default_target")]
    pub instance_id: String,
    #[serde(default = "default_range")]
    pub range: String, // "15m" | "1h" | "6h" | "24h" | "7d"
}

fn default_target() -> String {
    "host".into()
}
fn default_range() -> String {
    "1h".into()
}

pub async fn get_metrics(
    State(state): State<AppState>,
    _auth: AuthUser,
    Query(q): Query<MetricsQuery>,
) -> Json<Vec<MetricPoint>> {
    let secs: i64 = match q.range.as_str() {
        "15m" => 900,
        "1h" => 3600,
        "6h" => 21600,
        "24h" => 86400,
        "7d" => 604800,
        _ => 3600,
    };
    let since = Utc::now().timestamp() - secs;
    let conn = state.db.conn();
    let mut stmt = conn
        .prepare(
            "SELECT timestamp, instance_id, cpu_percent, memory_mb, disk_mb, net_rx_kb, net_tx_kb, players_online
             FROM metrics_history
             WHERE instance_id = ?1 AND timestamp >= ?2
             ORDER BY timestamp ASC
             LIMIT 600",
        )
        .unwrap();
    let points = stmt
        .query_map(params![q.instance_id, since], |r| {
            Ok(MetricPoint {
                timestamp: r.get(0)?,
                instance_id: r.get(1)?,
                cpu_percent: r.get(2)?,
                memory_mb: r.get(3)?,
                disk_mb: r.get(4)?,
                net_rx_kb: r.get(5)?,
                net_tx_kb: r.get(6)?,
                players_online: r.get(7)?,
            })
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    Json(points)
}

#[derive(Serialize)]
pub struct AnalyticsOverview {
    pub unique_players: i64,
    pub total_sessions: i64,
    pub active_sessions: i64,
    pub total_playtime_secs: i64,
    pub avg_session_secs: i64,
    pub countries_count: i64,
    pub leaderboard: Vec<PlayerLeaderboardEntry>,
    pub geo_stats: Vec<GeoStat>,
    pub recent_sessions: Vec<PlayerSession>,
}

pub async fn get_analytics_overview(
    State(state): State<AppState>,
    _auth: AuthUser,
) -> Json<AnalyticsOverview> {
    let conn = state.db.conn();

    let unique_players: i64 = conn
        .query_row("SELECT COUNT(DISTINCT player_name) FROM player_sessions", [], |r| r.get(0))
        .unwrap_or(0);
    let total_sessions: i64 = conn
        .query_row("SELECT COUNT(*) FROM player_sessions", [], |r| r.get(0))
        .unwrap_or(0);
    let active_sessions: i64 = conn
        .query_row("SELECT COUNT(*) FROM player_sessions WHERE left_at IS NULL", [], |r| r.get(0))
        .unwrap_or(0);
    let total_playtime_secs: i64 = conn
        .query_row("SELECT COALESCE(SUM(duration_secs), 0) FROM player_sessions", [], |r| r.get(0))
        .unwrap_or(0);
    let avg_session_secs = if total_sessions > 0 {
        total_playtime_secs / total_sessions
    } else {
        0
    };
    let countries_count: i64 = conn
        .query_row("SELECT COUNT(DISTINCT country_code) FROM player_sessions", [], |r| r.get(0))
        .unwrap_or(0);

    let mut lb_stmt = conn
        .prepare(
            "SELECT player_name, COUNT(*) as sess, COALESCE(SUM(duration_secs), 0) as playtime,
                    MAX(joined_at) as last_seen, MAX(country_code), MAX(country_name), MAX(instance_name)
             FROM player_sessions
             GROUP BY player_name
             ORDER BY playtime DESC
             LIMIT 50",
        )
        .unwrap();
    let leaderboard = lb_stmt
        .query_map([], |r| {
            Ok(PlayerLeaderboardEntry {
                player_name: r.get(0)?,
                total_sessions: r.get(1)?,
                total_playtime_secs: r.get(2)?,
                last_seen: r.get(3)?,
                country_code: r.get(4)?,
                country_name: r.get(5)?,
                favorite_instance: r.get(6)?,
            })
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    let mut geo_stmt = conn
        .prepare(
            "SELECT country_code, MAX(country_name), COUNT(DISTINCT player_name), COUNT(*), COALESCE(SUM(duration_secs), 0)
             FROM player_sessions
             GROUP BY country_code
             ORDER BY COUNT(*) DESC",
        )
        .unwrap();
    let geo_stats = geo_stmt
        .query_map([], |r| {
            Ok(GeoStat {
                country_code: r.get(0)?,
                country_name: r.get(1)?,
                unique_players: r.get(2)?,
                total_sessions: r.get(3)?,
                total_playtime_secs: r.get(4)?,
            })
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    let mut sess_stmt = conn
        .prepare(
            "SELECT id, instance_id, instance_name, player_name, player_uuid, ip_address, country_code, country_name, joined_at, left_at, duration_secs
             FROM player_sessions
             ORDER BY joined_at DESC
             LIMIT 100",
        )
        .unwrap();
    let recent_sessions = sess_stmt
        .query_map([], |r| {
            Ok(PlayerSession {
                id: r.get(0)?,
                instance_id: r.get(1)?,
                instance_name: r.get(2)?,
                player_name: r.get(3)?,
                player_uuid: r.get(4)?,
                ip_address: r.get(5)?,
                country_code: r.get(6)?,
                country_name: r.get(7)?,
                joined_at: r.get(8)?,
                left_at: r.get(9)?,
                duration_secs: r.get(10)?,
            })
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    Json(AnalyticsOverview {
        unique_players,
        total_sessions,
        active_sessions,
        total_playtime_secs,
        avg_session_secs,
        countries_count,
        leaderboard,
        geo_stats,
        recent_sessions,
    })
}

pub async fn export_sessions_csv(
    State(state): State<AppState>,
    _auth: AuthUser,
) -> impl IntoResponse {
    let conn = state.db.conn();
    let mut stmt = conn
        .prepare(
            "SELECT player_name, instance_name, ip_address, country_code, country_name, joined_at, COALESCE(left_at, 'ONLINE'), duration_secs
             FROM player_sessions ORDER BY joined_at DESC",
        )
        .unwrap();
    let mut csv = String::from("player_name,instance_name,ip_address,country_code,country_name,joined_at,left_at,duration_secs\n");
    let rows = stmt
        .query_map([], |r| {
            Ok(format!(
                "{},{},{},{},{},{},{},{}\n",
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, i64>(7)?,
            ))
        })
        .unwrap();
    for line in rows.flatten() {
        csv.push_str(&line);
    }
    (
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"goldsmp-player-analytics.csv\"",
            ),
        ],
        csv,
    )
}

pub async fn list_geo_rules(
    State(state): State<AppState>,
    _auth: AuthUser,
) -> Json<Vec<GeoFilterRule>> {
    let conn = state.db.conn();
    let mut stmt = conn
        .prepare(
            "SELECT id, instance_id, country_code, country_name, action, enabled, created_at
             FROM geo_rules ORDER BY created_at DESC",
        )
        .unwrap();
    let rules = stmt
        .query_map([], |r| {
            Ok(GeoFilterRule {
                id: r.get(0)?,
                instance_id: r.get(1)?,
                country_code: r.get(2)?,
                country_name: r.get(3)?,
                action: r.get(4)?,
                enabled: r.get::<_, i64>(5)? != 0,
                created_at: r.get(6)?,
            })
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    Json(rules)
}

pub async fn create_geo_rule(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CreateGeoRuleReq>,
) -> Result<Json<GeoFilterRule>, (StatusCode, String)> {
    let rule = GeoFilterRule {
        id: Uuid::new_v4().to_string(),
        instance_id: req.instance_id,
        country_code: req.country_code.to_uppercase(),
        country_name: req.country_name,
        action: req.action,
        enabled: true,
        created_at: Utc::now().to_rfc3339(),
    };
    {
        let conn = state.db.conn();
        conn.execute(
            "INSERT INTO geo_rules (id, instance_id, country_code, country_name, action, enabled, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6)",
            params![
                rule.id,
                rule.instance_id,
                rule.country_code,
                rule.country_name,
                rule.action,
                rule.created_at
            ],
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    state.db.log_audit(
        &auth.0.username,
        "GEOFILTER_CREATE",
        &rule.country_code,
        &format!("Azione: {} su {}", rule.action, rule.instance_id),
        "local",
    );
    Ok(Json(rule))
}

pub async fn delete_geo_rule(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    {
        let conn = state.db.conn();
        conn.execute("DELETE FROM geo_rules WHERE id = ?1", params![id])
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    state
        .db
        .log_audit(&auth.0.username, "GEOFILTER_DELETE", &id, "Regola geo rimossa", "local");
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_audit_logs(
    State(state): State<AppState>,
    _auth: AuthUser,
) -> Json<Vec<AuditLogEntry>> {
    let conn = state.db.conn();
    let mut stmt = conn
        .prepare(
            "SELECT id, timestamp, username, action, target, details, ip_address
             FROM audit_logs ORDER BY id DESC LIMIT 200",
        )
        .unwrap();
    let logs = stmt
        .query_map([], |r| {
            Ok(AuditLogEntry {
                id: r.get(0)?,
                timestamp: r.get(1)?,
                username: r.get(2)?,
                action: r.get(3)?,
                target: r.get(4)?,
                details: r.get(5)?,
                ip_address: r.get(6)?,
            })
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    Json(logs)
}
