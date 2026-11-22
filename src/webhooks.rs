use crate::db::Database;
use crate::models::WebhookConfig;
use chrono::Utc;
use rusqlite::params;
use serde_json::json;

pub fn list_webhooks(db: &Database) -> Vec<WebhookConfig> {
    let conn = db.conn();
    let mut stmt = conn
        .prepare(
            "SELECT id, name, url, events_json, enabled, secret, last_triggered, last_status, created_at
             FROM webhooks ORDER BY created_at DESC",
        )
        .unwrap();
    stmt.query_map([], |row| {
        let ev_str: String = row.get(3)?;
        Ok(WebhookConfig {
            id: row.get(0)?,
            name: row.get(1)?,
            url: row.get(2)?,
            events: serde_json::from_str(&ev_str).unwrap_or_default(),
            enabled: row.get::<_, i64>(4)? != 0,
            secret: row.get(5)?,
            last_triggered: row.get(6)?,
            last_status: row.get(7)?,
            created_at: row.get(8)?,
        })
    })
    .unwrap()
    .filter_map(|r| r.ok())
    .collect()
}

pub fn dispatch_event(
    db: Database,
    http: reqwest::Client,
    event: String,
    instance_id: String,
    instance_name: String,
    details: String,
) {
    tokio::spawn(async move {
        let hooks = list_webhooks(&db);
        for hook in hooks {
            if !hook.enabled {
                continue;
            }
            if !hook.events.iter().any(|e| e == "*" || e == &event) {
                continue;
            }

            let is_discord = hook.url.contains("discord.com/api/webhooks")
                || hook.url.contains("discordapp.com/api/webhooks");

            let payload = if is_discord {
                json!({
                    "username": "GOLDSMP",
                    "embeds": [{
                        "title": format!("Evento: {}", event),
                        "description": details,
                        "color": 439972,
                        "fields": [
                            { "name": "Istanza", "value": instance_name, "inline": true },
                            { "name": "ID", "value": instance_id, "inline": true }
                        ],
                        "timestamp": Utc::now().to_rfc3339()
                    }]
                })
            } else {
                json!({
                    "event": event,
                    "instance_id": instance_id,
                    "instance_name": instance_name,
                    "details": details,
                    "timestamp": Utc::now().to_rfc3339()
                })
            };

            let status = match http
                .post(&hook.url)
                .header("X-GOLDSMP-Secret", &hook.secret)
                .json(&payload)
                .send()
                .await
            {
                Ok(resp) => resp.status().as_u16() as i32,
                Err(_) => 0,
            };

            let conn = db.conn();
            let _ = conn.execute(
                "UPDATE webhooks SET last_triggered = ?1, last_status = ?2 WHERE id = ?3",
                params![Utc::now().to_rfc3339(), status, hook.id],
            );
        }
    });
}
