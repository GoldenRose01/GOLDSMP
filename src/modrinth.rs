use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModrinthHit {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub downloads: u64,
    pub icon_url: Option<String>,
    pub project_type: String,
    pub categories: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ModrinthSearchResponse {
    hits: Vec<ModrinthHit>,
}

#[derive(Debug, Deserialize)]
struct ModrinthVersion {
    project_id: Option<String>,
    version_number: String,
    files: Vec<ModrinthFile>,
}

#[derive(Debug, Deserialize)]
struct ModrinthFile {
    url: String,
    filename: String,
    primary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledModEntry {
    pub filename: String,
    pub folder: String,
    pub size_bytes: u64,
    pub enabled: bool,
    pub sha1: String,
    pub modrinth_project_id: Option<String>,
    pub current_version: Option<String>,
    pub latest_version: Option<String>,
    pub latest_filename: Option<String>,
    pub latest_url: Option<String>,
    pub update_available: bool,
}

pub async fn search_modrinth(
    http: &reqwest::Client,
    query: &str,
    project_type: &str,
) -> Result<Vec<ModrinthHit>, String> {
    let facets = format!("[[\"project_type:{}\"]]", project_type);
    let resp = http
        .get("https://api.modrinth.com/v2/search")
        .header("User-Agent", "GOLDSMP/1.0 (https://goldsmp.local)")
        .query(&[
            ("query", query),
            ("limit", "20"),
            ("facets", facets.as_str()),
        ])
        .send()
        .await
        .map_err(|e| format!("Errore connessione Modrinth: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Modrinth API ha risposto con status {}", resp.status()));
    }

    let parsed: ModrinthSearchResponse = resp
        .json()
        .await
        .map_err(|e| format!("Errore parsing Modrinth: {}", e))?;
    Ok(parsed.hits)
}

pub async fn install_modrinth_project(
    http: &reqwest::Client,
    project_id: &str,
    target_dir: &Path,
) -> Result<String, String> {
    let url = format!("https://api.modrinth.com/v2/project/{}/version", project_id);
    let resp = http
        .get(&url)
        .header("User-Agent", "GOLDSMP/1.0 (https://goldsmp.local)")
        .send()
        .await
        .map_err(|e| format!("Errore fetch versioni Modrinth: {}", e))?;

    let versions: Vec<ModrinthVersion> = resp
        .json()
        .await
        .map_err(|e| format!("Errore parsing versioni Modrinth: {}", e))?;

    let latest = versions
        .first()
        .ok_or_else(|| "Nessuna versione disponibile su Modrinth".to_string())?;

    let file = latest
        .files
        .iter()
        .find(|f| f.primary)
        .or_else(|| latest.files.first())
        .ok_or_else(|| "Nessun file scaricabile trovato".to_string())?;

    let bytes = http
        .get(&file.url)
        .header("User-Agent", "GOLDSMP/1.0 (https://goldsmp.local)")
        .send()
        .await
        .map_err(|e| format!("Errore download mod: {}", e))?
        .bytes()
        .await
        .map_err(|e| format!("Errore lettura stream mod: {}", e))?;

    std::fs::create_dir_all(target_dir).map_err(|e| e.to_string())?;
    let dest = target_dir.join(&file.filename);
    std::fs::write(&dest, &bytes).map_err(|e| e.to_string())?;

    Ok(format!("{} ({})", file.filename, latest.version_number))
}

fn sha1_hex(path: &Path) -> String {
    match std::fs::read(path) {
        Ok(bytes) => {
            let mut hasher = Sha1::new();
            hasher.update(&bytes);
            let digest = hasher.finalize();
            digest.iter().map(|b| format!("{:02x}", b)).collect()
        }
        Err(_) => String::new(),
    }
}

/// Scans `work_dir/<folder>` (e.g. `mods` or `plugins`) and lists all `.jar`, `.hmod`, and `.disabled` files.
/// When `check_modrinth` is true, queries Modrinth API v2 by SHA-1 hash to detect installed versions and available updates.
pub async fn scan_installed_mods(
    http: &reqwest::Client,
    work_dir: &Path,
    folder: &str,
    check_modrinth: bool,
) -> Result<Vec<InstalledModEntry>, String> {
    let dir = work_dir.join(folder);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let mut entries = Vec::new();
    let rd = std::fs::read_dir(&dir).map_err(|e| e.to_string())?;
    for item in rd.flatten() {
        let meta = match item.metadata() {
            Ok(m) if m.is_file() => m,
            _ => continue,
        };
        let filename = item.file_name().to_string_lossy().to_string();
        let lower = filename.to_lowercase();
        let is_mod = lower.ends_with(".jar")
            || lower.ends_with(".jar.disabled")
            || lower.ends_with(".hmod")
            || lower.ends_with(".hmod.disabled");
        if !is_mod {
            continue;
        }
        let enabled = !lower.ends_with(".disabled");
        let sha1 = if check_modrinth && lower.contains(".jar") {
            sha1_hex(&item.path())
        } else {
            String::new()
        };

        entries.push(InstalledModEntry {
            filename,
            folder: folder.to_string(),
            size_bytes: meta.len(),
            enabled,
            sha1,
            modrinth_project_id: None,
            current_version: None,
            latest_version: None,
            latest_filename: None,
            latest_url: None,
            update_available: false,
        });
    }

    entries.sort_by(|a, b| a.filename.to_lowercase().cmp(&b.filename.to_lowercase()));

    if check_modrinth {
        let hashes: Vec<String> = entries
            .iter()
            .filter(|e| !e.sha1.is_empty())
            .map(|e| e.sha1.clone())
            .collect();

        if !hashes.is_empty() {
            // 1. Batch lookup current versions by SHA-1 on Modrinth
            if let Ok(resp) = http
                .post("https://api.modrinth.com/v2/version_files")
                .header("User-Agent", "GOLDSMP/1.0 (https://goldsmp.local)")
                .json(&serde_json::json!({
                    "hashes": hashes,
                    "algorithm": "sha1"
                }))
                .send()
                .await
            {
                if let Ok(found_map) = resp.json::<HashMap<String, ModrinthVersion>>().await {
                    for entry in &mut entries {
                        if let Some(cur_ver) = found_map.get(&entry.sha1) {
                            entry.modrinth_project_id = cur_ver.project_id.clone();
                            entry.current_version = Some(cur_ver.version_number.clone());
                        }
                    }
                }
            }

            // 2. For each identified project, check latest version on Modrinth
            for entry in &mut entries {
                if let Some(ref pid) = entry.modrinth_project_id {
                    let url = format!("https://api.modrinth.com/v2/project/{}/version", pid);
                    if let Ok(resp) = http
                        .get(&url)
                        .header("User-Agent", "GOLDSMP/1.0 (https://goldsmp.local)")
                        .send()
                        .await
                    {
                        if let Ok(versions) = resp.json::<Vec<ModrinthVersion>>().await {
                            if let Some(latest) = versions.first() {
                                let file = latest
                                    .files
                                    .iter()
                                    .find(|f| f.primary)
                                    .or_else(|| latest.files.first());
                                if let Some(f) = file {
                                    entry.latest_version = Some(latest.version_number.clone());
                                    entry.latest_filename = Some(f.filename.clone());
                                    entry.latest_url = Some(f.url.clone());
                                    let cur = entry.current_version.as_deref().unwrap_or("");
                                    entry.update_available =
                                        !cur.is_empty() && cur != latest.version_number;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(entries)
}

/// Downloads the updated JAR from Modrinth and removes the old JAR file.
pub async fn update_installed_mod(
    http: &reqwest::Client,
    work_dir: &Path,
    folder: &str,
    old_filename: &str,
    download_url: &str,
    new_filename: &str,
) -> Result<String, String> {
    if old_filename.contains("..") || new_filename.contains("..") || folder.contains("..") {
        return Err("Nome file non valido".into());
    }
    let dir = work_dir.join(folder);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let bytes = http
        .get(download_url)
        .header("User-Agent", "GOLDSMP/1.0 (https://goldsmp.local)")
        .send()
        .await
        .map_err(|e| format!("Errore download aggiornamento: {}", e))?
        .bytes()
        .await
        .map_err(|e| format!("Errore lettura stream: {}", e))?;

    let new_path = dir.join(new_filename);
    std::fs::write(&new_path, &bytes).map_err(|e| e.to_string())?;

    if old_filename != new_filename {
        let old_path = dir.join(old_filename);
        if old_path.exists() {
            let _ = std::fs::remove_file(old_path);
        }
    }

    Ok(new_filename.to_string())
}
