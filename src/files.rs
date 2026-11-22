use crate::models::FileEntry;
use chrono::{DateTime, Utc};
use std::path::{Component, Path, PathBuf};

/// Safely resolves a relative path inside `root`, preventing directory traversal.
pub fn resolve_safe_path(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let mut clean = PathBuf::new();
    let trimmed = rel.trim_start_matches('/');
    for comp in Path::new(trimmed).components() {
        match comp {
            Component::Normal(c) => clean.push(c),
            Component::CurDir => {}
            Component::ParentDir => {
                if !clean.pop() {
                    return Err("Accesso fuori dalla directory dell'istanza non consentito".into());
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err("Percorso assoluto non consentito".into());
            }
        }
    }
    Ok(root.join(clean))
}

pub fn list_dir(root: &Path, rel: &str) -> Result<Vec<FileEntry>, String> {
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let target = resolve_safe_path(root, rel)?;
    if !target.exists() {
        return Err("Directory non trovata".into());
    }
    let mut entries = Vec::new();
    let read_dir = std::fs::read_dir(&target).map_err(|e| e.to_string())?;
    for entry in read_dir.flatten() {
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let name = entry.file_name().to_string_lossy().to_string();
        let entry_path = entry.path();
        let rel_path = entry_path
            .strip_prefix(root)
            .unwrap_or(&entry_path)
            .to_string_lossy()
            .replace('\\', "/");
        let modified = meta
            .modified()
            .ok()
            .map(|t| DateTime::<Utc>::from(t).to_rfc3339())
            .unwrap_or_default();
        entries.push(FileEntry {
            name,
            path: rel_path,
            is_dir: meta.is_dir(),
            size: if meta.is_dir() { 0 } else { meta.len() },
            modified,
        });
    }
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

pub fn read_file_text(root: &Path, rel: &str) -> Result<String, String> {
    let target = resolve_safe_path(root, rel)?;
    let meta = std::fs::metadata(&target).map_err(|e| e.to_string())?;
    if meta.len() > 5 * 1024 * 1024 {
        return Err("File troppo grande per l'editor web (> 5 MB)".into());
    }
    std::fs::read_to_string(&target).map_err(|e| format!("Impossibile leggere il file come testo: {}", e))
}

pub fn write_file_text(root: &Path, rel: &str, content: &str) -> Result<(), String> {
    let target = resolve_safe_path(root, rel)?;
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&target, content).map_err(|e| e.to_string())
}

pub fn create_directory(root: &Path, rel: &str) -> Result<(), String> {
    let target = resolve_safe_path(root, rel)?;
    std::fs::create_dir_all(&target).map_err(|e| e.to_string())
}

pub fn delete_path(root: &Path, rel: &str) -> Result<(), String> {
    if rel.trim().is_empty() || rel.trim() == "/" {
        return Err("Impossibile eliminare la directory radice dell'istanza".into());
    }
    let target = resolve_safe_path(root, rel)?;
    if !target.exists() {
        return Err("Percorso non trovato".into());
    }
    if target.is_dir() {
        std::fs::remove_dir_all(&target).map_err(|e| e.to_string())
    } else {
        std::fs::remove_file(&target).map_err(|e| e.to_string())
    }
}

pub fn rename_path(root: &Path, from: &str, to: &str) -> Result<(), String> {
    let src = resolve_safe_path(root, from)?;
    let dst = resolve_safe_path(root, to)?;
    std::fs::rename(&src, &dst).map_err(|e| e.to_string())
}

pub fn dir_size_mb(root: &Path) -> f64 {
    if !root.exists() {
        return 0.0;
    }
    let total: u64 = walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum();
    (total as f64) / (1024.0 * 1024.0)
}
