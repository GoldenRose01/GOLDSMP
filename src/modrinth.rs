use serde::{Deserialize, Serialize};
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
    id: String,
    name: String,
    version_number: String,
    files: Vec<ModrinthFile>,
}

#[derive(Debug, Deserialize)]
struct ModrinthFile {
    url: String,
    filename: String,
    primary: bool,
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
