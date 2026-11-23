use crate::db::Database;
use rusqlite::params;
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
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub downloads: u64,
    pub icon_url: Option<String>,
    pub project_type: String,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ModrinthSearchResponse {
    hits: Vec<ModrinthHit>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModrinthVersion {
    pub id: Option<String>,
    pub project_id: Option<String>,
    pub version_number: String,
    #[serde(default)]
    pub version_type: String, // "release" | "beta" | "alpha"
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub loaders: Vec<String>,
    pub files: Vec<ModrinthFile>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModrinthFile {
    pub url: String,
    pub filename: String,
    pub primary: bool,
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModrinthAuthConfig {
    pub token: String,
    pub user_id: String,
    pub username: String,
    pub avatar_url: Option<String>,
    pub connected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModrinthAuthPublicStatus {
    pub connected: bool,
    pub user_id: String,
    pub username: String,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ModrinthUserResp {
    id: String,
    username: String,
    avatar_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ModrinthOwnProject {
    id: String,
    slug: String,
    title: String,
    description: String,
    #[serde(default)]
    downloads: u64,
    icon_url: Option<String>,
    project_type: String,
    #[serde(default)]
    categories: Vec<String>,
    status: Option<String>,
}

pub fn load_auth_config(db: &Database) -> ModrinthAuthConfig {
    let conn = db.conn();
    conn.query_row(
        "SELECT value FROM kv_settings WHERE key = 'modrinth_auth'",
        [],
        |r| r.get::<_, String>(0),
    )
    .ok()
    .and_then(|s| serde_json::from_str(&s).ok())
    .unwrap_or_default()
}

pub fn save_auth_config(db: &Database, cfg: &ModrinthAuthConfig) -> Result<(), String> {
    let conn = db.conn();
    conn.execute(
        "INSERT OR REPLACE INTO kv_settings (key, value) VALUES ('modrinth_auth', ?1)",
        params![serde_json::to_string(cfg).unwrap()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_auth_public_status(db: &Database) -> ModrinthAuthPublicStatus {
    let cfg = load_auth_config(db);
    ModrinthAuthPublicStatus {
        connected: cfg.connected && !cfg.token.is_empty(),
        user_id: cfg.user_id,
        username: cfg.username,
        avatar_url: cfg.avatar_url,
    }
}

pub async fn login_with_token(
    db: &Database,
    http: &reqwest::Client,
    token: &str,
) -> Result<ModrinthAuthPublicStatus, String> {
    let clean = token.trim();
    if clean.is_empty() {
        return Err("Token Modrinth vuoto".into());
    }
    let resp = http
        .get("https://api.modrinth.com/v2/user")
        .header("User-Agent", "GOLDSMP/1.0 (https://goldsmp.local)")
        .header("Authorization", clean)
        .send()
        .await
        .map_err(|e| format!("Errore connessione Modrinth: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!(
            "Token Modrinth non valido (HTTP {}). Genera un Personal Access Token su modrinth.com/settings/pats",
            resp.status()
        ));
    }

    let u: ModrinthUserResp = resp
        .json()
        .await
        .map_err(|e| format!("Errore lettura profilo Modrinth: {}", e))?;

    let cfg = ModrinthAuthConfig {
        token: clean.to_string(),
        user_id: u.id.clone(),
        username: u.username.clone(),
        avatar_url: u.avatar_url.clone(),
        connected: true,
    };
    save_auth_config(db, &cfg)?;

    Ok(ModrinthAuthPublicStatus {
        connected: true,
        user_id: u.id,
        username: u.username,
        avatar_url: u.avatar_url,
    })
}

pub fn logout_modrinth(db: &Database) -> Result<ModrinthAuthPublicStatus, String> {
    save_auth_config(db, &ModrinthAuthConfig::default())?;
    Ok(get_auth_public_status(db))
}

/// Fetches the authenticated user's personal/private/draft/unlisted/public projects on Modrinth.
pub async fn list_personal_projects(
    db: &Database,
    http: &reqwest::Client,
) -> Result<Vec<ModrinthHit>, String> {
    let cfg = load_auth_config(db);
    if !cfg.connected || cfg.token.is_empty() || cfg.user_id.is_empty() {
        return Err("Non hai effettuato il login su Modrinth".into());
    }

    let url = format!("https://api.modrinth.com/v2/user/{}/projects", cfg.user_id);
    let resp = http
        .get(&url)
        .header("User-Agent", "GOLDSMP/1.0 (https://goldsmp.local)")
        .header("Authorization", &cfg.token)
        .send()
        .await
        .map_err(|e| format!("Errore fetch progetti personali Modrinth: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!(
            "Modrinth API ha risposto con status {}",
            resp.status()
        ));
    }

    let projects: Vec<ModrinthOwnProject> = resp
        .json()
        .await
        .map_err(|e| format!("Errore parsing progetti personali: {}", e))?;

    let hits = projects
        .into_iter()
        .map(|p| ModrinthHit {
            project_id: p.id,
            slug: p.slug,
            title: p.title,
            description: p.description,
            author: cfg.username.clone(),
            downloads: p.downloads,
            icon_url: p.icon_url,
            project_type: p.project_type,
            categories: p.categories,
            status: p.status,
        })
        .collect();

    Ok(hits)
}

fn is_snapshot_game_version(v: &str) -> bool {
    let lower = v.to_lowercase();
    lower.contains("snapshot")
        || lower.contains("-pre")
        || lower.contains("-rc")
        || lower.contains("experimental")
        || (lower.len() >= 5
            && lower.chars().take(2).all(|c| c.is_ascii_digit())
            && lower.chars().nth(2) == Some('w'))
}

fn expand_compatible_loaders(loader: &str, folder: &str) -> Vec<String> {
    let clean = loader.trim().to_lowercase();
    if folder == "plugins" {
        match clean.as_str() {
            "purpur" => vec![
                "purpur".into(),
                "paper".into(),
                "spigot".into(),
                "bukkit".into(),
            ],
            "paper" => vec!["paper".into(), "spigot".into(), "bukkit".into()],
            _ => vec![
                "paper".into(),
                "purpur".into(),
                "spigot".into(),
                "bukkit".into(),
            ],
        }
    } else {
        match clean.as_str() {
            "fabric" => vec!["fabric".into(), "quilt".into()],
            "neoforge" => vec!["neoforge".into()],
            "forge" => vec!["forge".into()],
            "purpur" | "paper" => vec![
                "paper".into(),
                "purpur".into(),
                "spigot".into(),
                "bukkit".into(),
                "fabric".into(),
            ],
            _ => vec![],
        }
    }
}

/// Picks the best Modrinth version matching the server's Minecraft version and loader,
/// strictly preferring `version_type == "release"` and avoiding snapshot-only builds!
pub fn pick_best_modrinth_version<'a>(
    versions: &'a [ModrinthVersion],
    mc_version: Option<&str>,
    loader: Option<&str>,
    folder: &str,
) -> Option<&'a ModrinthVersion> {
    if versions.is_empty() {
        return None;
    }

    let clean_mc = mc_version
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("latest"));

    let compat_loaders = loader
        .map(|l| expand_compatible_loaders(l, folder))
        .unwrap_or_default();

    let matches_mc = |v: &ModrinthVersion, target_mc: &str| -> bool {
        if v.game_versions.iter().any(|gv| gv == target_mc) {
            return true;
        }
        // Also match major.minor prefix (e.g., server on "1.21.4" matches "1.21.x" or "26.3" matches "26.3")
        let parts: Vec<&str> = target_mc.split('.').collect();
        if parts.len() >= 2 {
            let prefix = format!("{}.{}", parts[0], parts[1]);
            v.game_versions
                .iter()
                .any(|gv| gv == &prefix || gv.starts_with(&format!("{}.", prefix)))
        } else {
            false
        }
    };

    let matches_loader = |v: &ModrinthVersion| -> bool {
        if compat_loaders.is_empty() || v.loaders.is_empty() {
            return true;
        }
        v.loaders
            .iter()
            .any(|vl| compat_loaders.iter().any(|cl| vl.eq_ignore_ascii_case(cl)))
    };

    let is_stable_release = |v: &ModrinthVersion| -> bool {
        v.version_type.eq_ignore_ascii_case("release")
            && !v.version_number.to_lowercase().contains("snapshot")
            && (v.game_versions.is_empty()
                || v.game_versions.iter().any(|gv| !is_snapshot_game_version(gv)))
    };

    // Pass 1: Exact MC version + compatible loader + stable release
    if let Some(target_mc) = clean_mc {
        if let Some(found) = versions.iter().find(|v| {
            v.game_versions.iter().any(|gv| gv == target_mc)
                && matches_loader(v)
                && is_stable_release(v)
        }) {
            return Some(found);
        }
        // Pass 2: Exact MC version + compatible loader (even beta/alpha if no release exists for this MC version)
        if let Some(found) = versions
            .iter()
            .find(|v| v.game_versions.iter().any(|gv| gv == target_mc) && matches_loader(v))
        {
            return Some(found);
        }
        // Pass 3: Major.minor MC version + compatible loader + stable release
        if let Some(found) = versions
            .iter()
            .find(|v| matches_mc(v, target_mc) && matches_loader(v) && is_stable_release(v))
        {
            return Some(found);
        }
        // Pass 4: Exact MC version (any loader) + stable release
        if let Some(found) = versions
            .iter()
            .find(|v| matches_mc(v, target_mc) && is_stable_release(v))
        {
            return Some(found);
        }
    }

    // Pass 5: Compatible loader + stable release (never pick snapshot if a release exists!)
    if let Some(found) = versions
        .iter()
        .find(|v| matches_loader(v) && is_stable_release(v))
    {
        return Some(found);
    }

    // Pass 6: Any stable release
    if let Some(found) = versions.iter().find(|v| is_stable_release(v)) {
        return Some(found);
    }

    // Pass 7: Compatible loader (any)
    if let Some(found) = versions.iter().find(|v| matches_loader(v)) {
        return Some(found);
    }

    versions.first()
}

pub async fn search_modrinth(
    http: &reqwest::Client,
    query: &str,
    project_type: &str,
    mc_version: Option<&str>,
    auth_token: Option<&str>,
) -> Result<Vec<ModrinthHit>, String> {
    let mut facet_groups = vec![format!("[\"project_type:{}\"]", project_type)];
    if let Some(ver) = mc_version
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("latest") && !s.eq_ignore_ascii_case("all"))
    {
        facet_groups.push(format!("[\"versions:{}\"]", ver));
    }
    let facets = format!("[{}]", facet_groups.join(","));

    let mut req = http
        .get("https://api.modrinth.com/v2/search")
        .header("User-Agent", "GOLDSMP/1.0 (https://goldsmp.local)")
        .query(&[
            ("query", query),
            ("limit", "24"),
            ("facets", facets.as_str()),
        ]);
    if let Some(tok) = auth_token.filter(|t| !t.is_empty()) {
        req = req.header("Authorization", tok);
    }

    let resp = req
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

    // If version filter yielded 0 results (e.g. brand new MC version like 26.3), fallback to unfiltered search
    if parsed.hits.is_empty() && facet_groups.len() > 1 {
        return Box::pin(search_modrinth(http, query, project_type, None, auth_token)).await;
    }

    Ok(parsed.hits)
}

pub async fn list_project_versions(
    http: &reqwest::Client,
    project_id: &str,
    auth_token: Option<&str>,
) -> Result<Vec<ModrinthVersion>, String> {
    let url = format!("https://api.modrinth.com/v2/project/{}/version", project_id);
    let mut req = http
        .get(&url)
        .header("User-Agent", "GOLDSMP/1.0 (https://goldsmp.local)");
    if let Some(tok) = auth_token.filter(|t| !t.is_empty()) {
        req = req.header("Authorization", tok);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| format!("Errore fetch versioni Modrinth: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!(
            "Modrinth ha risposto con status {} per il progetto {}",
            resp.status(),
            project_id
        ));
    }
    resp.json::<Vec<ModrinthVersion>>()
        .await
        .map_err(|e| format!("Errore parsing versioni Modrinth: {}", e))
}

pub async fn install_modrinth_project(
    http: &reqwest::Client,
    project_id: &str,
    target_dir: &Path,
    mc_version: Option<&str>,
    loader: Option<&str>,
    folder: &str,
    version_id: Option<&str>,
    auth_token: Option<&str>,
) -> Result<String, String> {
    let versions = list_project_versions(http, project_id, auth_token).await?;

    let chosen = if let Some(vid) = version_id.filter(|s| !s.trim().is_empty()) {
        versions
            .iter()
            .find(|v| v.id.as_deref() == Some(vid) || v.version_number == vid)
            .or_else(|| pick_best_modrinth_version(&versions, mc_version, loader, folder))
    } else {
        pick_best_modrinth_version(&versions, mc_version, loader, folder)
    }
    .ok_or_else(|| "Nessuna versione compatibile trovata su Modrinth".to_string())?;

    let file = chosen
        .files
        .iter()
        .find(|f| f.primary)
        .or_else(|| chosen.files.first())
        .ok_or_else(|| "Nessun file scaricabile trovato".to_string())?;

    let mut dl_req = http
        .get(&file.url)
        .header("User-Agent", "GOLDSMP/1.0 (https://goldsmp.local)");
    if let Some(tok) = auth_token.filter(|t| !t.is_empty()) {
        dl_req = dl_req.header("Authorization", tok);
    }

    let bytes = dl_req
        .send()
        .await
        .map_err(|e| format!("Errore download mod: {}", e))?
        .bytes()
        .await
        .map_err(|e| format!("Errore lettura stream mod: {}", e))?;

    std::fs::create_dir_all(target_dir).map_err(|e| e.to_string())?;
    let dest = target_dir.join(&file.filename);
    std::fs::write(&dest, &bytes).map_err(|e| e.to_string())?;

    let mc_tag = chosen
        .game_versions
        .first()
        .cloned()
        .unwrap_or_else(|| "any".into());
    Ok(format!(
        "{} (v{} • MC {} • {})",
        file.filename, chosen.version_number, mc_tag, chosen.version_type
    ))
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
/// When `check_modrinth` is true, queries Modrinth API v2 by SHA-1 hash to detect installed versions and available updates
/// filtered specifically for the instance's Minecraft version (`mc_version`) and loader (`loader`), ignoring snapshot builds!
pub async fn scan_installed_mods(
    http: &reqwest::Client,
    work_dir: &Path,
    folder: &str,
    check_modrinth: bool,
    mc_version: Option<&str>,
    loader: Option<&str>,
    auth_token: Option<&str>,
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
            let mut batch_req = http
                .post("https://api.modrinth.com/v2/version_files")
                .header("User-Agent", "GOLDSMP/1.0 (https://goldsmp.local)")
                .json(&serde_json::json!({
                    "hashes": hashes,
                    "algorithm": "sha1"
                }));
            if let Some(tok) = auth_token.filter(|t| !t.is_empty()) {
                batch_req = batch_req.header("Authorization", tok);
            }

            if let Ok(resp) = batch_req.send().await {
                if let Ok(found_map) = resp.json::<HashMap<String, ModrinthVersion>>().await {
                    for entry in &mut entries {
                        if let Some(cur_ver) = found_map.get(&entry.sha1) {
                            entry.modrinth_project_id = cur_ver.project_id.clone();
                            entry.current_version = Some(cur_ver.version_number.clone());
                        }
                    }
                }
            }

            // 2. For each identified project, check latest stable version matching the server's MC version & loader
            for entry in &mut entries {
                if let Some(ref pid) = entry.modrinth_project_id {
                    if let Ok(versions) = list_project_versions(http, pid, auth_token).await {
                        if let Some(best) =
                            pick_best_modrinth_version(&versions, mc_version, loader, folder)
                        {
                            let file = best
                                .files
                                .iter()
                                .find(|f| f.primary)
                                .or_else(|| best.files.first());
                            if let Some(f) = file {
                                entry.latest_version = Some(best.version_number.clone());
                                entry.latest_filename = Some(f.filename.clone());
                                entry.latest_url = Some(f.url.clone());
                                let cur = entry.current_version.as_deref().unwrap_or("");
                                entry.update_available =
                                    !cur.is_empty() && cur != best.version_number;
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
    auth_token: Option<&str>,
) -> Result<String, String> {
    if old_filename.contains("..") || new_filename.contains("..") || folder.contains("..") {
        return Err("Nome file non valido".into());
    }
    let dir = work_dir.join(folder);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let mut req = http
        .get(download_url)
        .header("User-Agent", "GOLDSMP/1.0 (https://goldsmp.local)");
    if let Some(tok) = auth_token.filter(|t| !t.is_empty()) {
        req = req.header("Authorization", tok);
    }

    let bytes = req
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
