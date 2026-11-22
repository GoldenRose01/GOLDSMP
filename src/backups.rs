use crate::db::Database;
use crate::models::BackupEntry;
use chrono::Utc;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use rusqlite::params;
use std::fs::File;
use std::path::{Path, PathBuf};
use tar::{Archive, Builder};
use uuid::Uuid;

pub fn backup_dir(data_dir: &Path, instance_id: &str) -> PathBuf {
    data_dir.join("backups").join(instance_id)
}

pub fn create_backup(
    db: &Database,
    data_dir: &Path,
    instance_id: &str,
    work_dir: &Path,
    name: &str,
    notes: &str,
) -> Result<BackupEntry, String> {
    let bdir = backup_dir(data_dir, instance_id);
    std::fs::create_dir_all(&bdir).map_err(|e| e.to_string())?;

    let id = Uuid::new_v4().to_string();
    let ts = Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("backup_{}_{}.tar.gz", ts, &id[..8]);
    let target_path = bdir.join(&filename);

    let tar_gz = File::create(&target_path).map_err(|e| e.to_string())?;
    let enc = GzEncoder::new(tar_gz, Compression::fast());
    let mut tar = Builder::new(enc);

    if work_dir.exists() {
        tar.append_dir_all(".", work_dir)
            .map_err(|e| format!("Errore archiviazione: {}", e))?;
    }
    let enc = tar.into_inner().map_err(|e| e.to_string())?;
    enc.finish().map_err(|e| e.to_string())?;

    let size_bytes = std::fs::metadata(&target_path)
        .map(|m| m.len())
        .unwrap_or(0);

    let entry = BackupEntry {
        id: id.clone(),
        instance_id: instance_id.to_string(),
        name: if name.trim().is_empty() {
            format!("Backup {}", Utc::now().format("%Y-%m-%d %H:%M"))
        } else {
            name.to_string()
        },
        filename,
        size_bytes,
        created_at: Utc::now().to_rfc3339(),
        notes: notes.to_string(),
    };

    let conn = db.conn();
    conn.execute(
        "INSERT INTO backups (id, instance_id, name, filename, size_bytes, created_at, notes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            entry.id,
            entry.instance_id,
            entry.name,
            entry.filename,
            entry.size_bytes as i64,
            entry.created_at,
            entry.notes
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(entry)
}

pub fn restore_backup(
    data_dir: &Path,
    instance_id: &str,
    filename: &str,
    work_dir: &Path,
) -> Result<(), String> {
    let target_path = backup_dir(data_dir, instance_id).join(filename);
    if !target_path.exists() {
        return Err("File di backup non trovato".into());
    }
    std::fs::create_dir_all(work_dir).map_err(|e| e.to_string())?;
    let tar_gz = File::open(&target_path).map_err(|e| e.to_string())?;
    let dec = GzDecoder::new(tar_gz);
    let mut archive = Archive::new(dec);
    archive
        .unpack(work_dir)
        .map_err(|e| format!("Errore ripristino: {}", e))?;
    Ok(())
}

pub fn delete_backup(
    db: &Database,
    data_dir: &Path,
    instance_id: &str,
    backup_id: &str,
) -> Result<(), String> {
    let conn = db.conn();
    let filename: String = conn
        .query_row(
            "SELECT filename FROM backups WHERE id = ?1 AND instance_id = ?2",
            params![backup_id, instance_id],
            |r| r.get(0),
        )
        .map_err(|_| "Backup non trovato".to_string())?;

    let path = backup_dir(data_dir, instance_id).join(filename);
    if path.exists() {
        let _ = std::fs::remove_file(path);
    }
    conn.execute("DELETE FROM backups WHERE id = ?1", params![backup_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn list_backups(db: &Database, instance_id: &str) -> Vec<BackupEntry> {
    let conn = db.conn();
    let mut stmt = conn
        .prepare(
            "SELECT id, instance_id, name, filename, size_bytes, created_at, notes
             FROM backups WHERE instance_id = ?1 ORDER BY created_at DESC",
        )
        .unwrap();
    stmt.query_map(params![instance_id], |r| {
        Ok(BackupEntry {
            id: r.get(0)?,
            instance_id: r.get(1)?,
            name: r.get(2)?,
            filename: r.get(3)?,
            size_bytes: r.get::<_, i64>(4)? as u64,
            created_at: r.get(5)?,
            notes: r.get(6)?,
        })
    })
    .unwrap()
    .filter_map(|r| r.ok())
    .collect()
}
