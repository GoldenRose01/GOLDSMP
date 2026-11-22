use crate::backups;
use crate::db::Database;
use crate::models::ScheduledTask;
use crate::process_manager::ProcessManager;
use crate::webhooks;
use chrono::{DateTime, Utc};
use rusqlite::params;
use std::path::PathBuf;
use std::sync::Arc;

pub fn list_tasks(db: &Database, instance_id: Option<&str>) -> Vec<ScheduledTask> {
    let conn = db.conn();
    let sql = if instance_id.is_some() {
        "SELECT id, instance_id, name, trigger_type, interval_secs, event_name, action_type, action_payload, enabled, last_run, run_count, created_at
         FROM scheduled_tasks WHERE instance_id = ?1 ORDER BY created_at DESC"
    } else {
        "SELECT id, instance_id, name, trigger_type, interval_secs, event_name, action_type, action_payload, enabled, last_run, run_count, created_at
         FROM scheduled_tasks ORDER BY created_at DESC"
    };
    let mut stmt = conn.prepare(sql).unwrap();
    let map_row = |r: &rusqlite::Row| -> rusqlite::Result<ScheduledTask> {
        Ok(ScheduledTask {
            id: r.get(0)?,
            instance_id: r.get(1)?,
            name: r.get(2)?,
            trigger_type: r.get(3)?,
            interval_secs: r.get(4)?,
            event_name: r.get(5)?,
            action_type: r.get(6)?,
            action_payload: r.get(7)?,
            enabled: r.get::<_, i64>(8)? != 0,
            last_run: r.get(9)?,
            run_count: r.get(10)?,
            created_at: r.get(11)?,
        })
    };

    if let Some(iid) = instance_id {
        stmt.query_map(params![iid], map_row)
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    } else {
        stmt.query_map([], map_row)
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    }
}

pub fn spawn_scheduler(
    db: Database,
    pm: Arc<ProcessManager>,
    data_dir: PathBuf,
    http: reqwest::Client,
) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            let tasks = list_tasks(&db, None);
            let now = Utc::now();

            for task in tasks {
                if !task.enabled || task.trigger_type != "interval" {
                    continue;
                }
                let interval = task.interval_secs.unwrap_or(3600).max(30);
                let due = match &task.last_run {
                    None => true,
                    Some(ts) => DateTime::parse_from_rfc3339(ts)
                        .map(|dt| (now.timestamp() - dt.timestamp()) >= interval)
                        .unwrap_or(true),
                };

                if due {
                    execute_task(&db, &pm, &data_dir, &http, &task).await;
                }
            }
        }
    });
}

pub async fn execute_task(
    db: &Database,
    pm: &Arc<ProcessManager>,
    data_dir: &PathBuf,
    http: &reqwest::Client,
    task: &ScheduledTask,
) {
    match task.action_type.as_str() {
        "command" => {
            let _ = pm.send_command_sync(&task.instance_id, &task.action_payload);
        }
        "restart" => {
            let _ = pm.restart_instance(&task.instance_id).await;
        }
        "backup" => {
            if let Some(inst) = pm.load_instance(&task.instance_id) {
                let _ = backups::create_backup(
                    db,
                    data_dir,
                    &inst.id,
                    &PathBuf::from(&inst.work_dir),
                    &format!("Auto: {}", task.name),
                    "Backup programmato dallo Scheduler",
                );
            }
        }
        "webhook" => {
            webhooks::dispatch_event(
                db.clone(),
                http.clone(),
                "scheduled_task".into(),
                task.instance_id.clone(),
                task.name.clone(),
                task.action_payload.clone(),
            );
        }
        _ => {}
    }

    let conn = db.conn();
    let _ = conn.execute(
        "UPDATE scheduled_tasks SET last_run = ?1, run_count = run_count + 1 WHERE id = ?2",
        params![Utc::now().to_rfc3339(), task.id],
    );
}
