use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McInstallResult {
    pub loader: String,
    pub mc_version: String,
    pub build_info: String,
    pub size_bytes: u64,
}

/// Automatically downloads `server.jar` into `work_dir` for the chosen loader (`paper`, `purpur`, `fabric`, `vanilla`)
/// and Minecraft version (`latest` or specific like `26.3`, `1.21.4`, etc.).
pub async fn download_minecraft_server_jar(
    http: &reqwest::Client,
    work_dir: &Path,
    loader: &str,
    requested_version: &str,
) -> Result<McInstallResult, String> {
    std::fs::create_dir_all(work_dir).map_err(|e| e.to_string())?;
    let target_jar = work_dir.join("server.jar");

    let clean_loader = loader.trim().to_lowercase();
    let clean_ver = requested_version.trim();

    let (bytes, resolved_ver, build_info) = match clean_loader.as_str() {
        "purpur" => download_purpur(http, clean_ver).await?,
        "fabric" => download_fabric(http, clean_ver).await?,
        "vanilla" => download_vanilla(http, clean_ver).await?,
        _ => download_paper(http, clean_ver).await?,
    };

    std::fs::write(&target_jar, &bytes)
        .map_err(|e| format!("Errore scrittura server.jar: {}", e))?;
    let _ = std::fs::write(work_dir.join("eula.txt"), "eula=true\n");

    Ok(McInstallResult {
        loader: clean_loader,
        mc_version: resolved_ver,
        build_info,
        size_bytes: bytes.len() as u64,
    })
}

async fn download_paper(
    http: &reqwest::Client,
    requested_version: &str,
) -> Result<(Vec<u8>, String, String), String> {
    // 1. Resolve version from PaperMC API v2
    let proj_json: serde_json::Value = http
        .get("https://api.papermc.io/v2/projects/paper")
        .header("User-Agent", "GOLDSMP/1.0")
        .send()
        .await
        .map_err(|e| format!("Errore connessione PaperMC API: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Errore risposta PaperMC: {}", e))?;

    let versions = proj_json
        .get("versions")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "Lista versioni PaperMC non trovata".to_string())?;

    let ver = if requested_version.is_empty()
        || requested_version.eq_ignore_ascii_case("latest")
        || !versions
            .iter()
            .any(|v| v.as_str() == Some(requested_version))
    {
        versions
            .last()
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Nessuna versione PaperMC disponibile".to_string())?
            .to_string()
    } else {
        requested_version.to_string()
    };

    // 2. Get latest build for resolved version
    let builds_url = format!(
        "https://api.papermc.io/v2/projects/paper/versions/{}/builds",
        ver
    );
    let builds_json: serde_json::Value = http
        .get(&builds_url)
        .header("User-Agent", "GOLDSMP/1.0")
        .send()
        .await
        .map_err(|e| format!("Errore fetch build PaperMC: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Errore parsing build PaperMC: {}", e))?;

    let latest_build = builds_json
        .get("builds")
        .and_then(|b| b.as_array())
        .and_then(|arr| arr.last())
        .ok_or_else(|| format!("Nessuna build Paper trovata per {}", ver))?;

    let build_num = latest_build
        .get("build")
        .and_then(|b| b.as_i64())
        .ok_or_else(|| "Numero build Paper non valido".to_string())?;

    let download_name = latest_build
        .get("downloads")
        .and_then(|d| d.get("application"))
        .and_then(|a| a.get("name"))
        .and_then(|n| n.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("paper-{}-{}.jar", ver, build_num));

    let jar_url = format!(
        "https://api.papermc.io/v2/projects/paper/versions/{}/builds/{}/downloads/{}",
        ver, build_num, download_name
    );

    let bytes = http
        .get(&jar_url)
        .header("User-Agent", "GOLDSMP/1.0")
        .send()
        .await
        .map_err(|e| format!("Errore download Paper jar: {}", e))?
        .bytes()
        .await
        .map_err(|e| format!("Errore stream Paper jar: {}", e))?
        .to_vec();

    Ok((bytes, ver, format!("Paper build #{}", build_num)))
}

async fn download_purpur(
    http: &reqwest::Client,
    requested_version: &str,
) -> Result<(Vec<u8>, String, String), String> {
    let proj_json: serde_json::Value = http
        .get("https://api.purpurmc.org/v2/purpur")
        .header("User-Agent", "GOLDSMP/1.0")
        .send()
        .await
        .map_err(|e| format!("Errore connessione Purpur API: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Errore risposta Purpur: {}", e))?;

    let versions = proj_json
        .get("versions")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "Lista versioni Purpur non trovata".to_string())?;

    let ver = if requested_version.is_empty()
        || requested_version.eq_ignore_ascii_case("latest")
        || !versions
            .iter()
            .any(|v| v.as_str() == Some(requested_version))
    {
        versions
            .last()
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Nessuna versione Purpur disponibile".to_string())?
            .to_string()
    } else {
        requested_version.to_string()
    };

    let jar_url = format!(
        "https://api.purpurmc.org/v2/purpur/{}/latest/download",
        ver
    );
    let bytes = http
        .get(&jar_url)
        .header("User-Agent", "GOLDSMP/1.0")
        .send()
        .await
        .map_err(|e| format!("Errore download Purpur jar: {}", e))?
        .bytes()
        .await
        .map_err(|e| format!("Errore stream Purpur jar: {}", e))?
        .to_vec();

    Ok((bytes, ver, "Purpur latest".to_string()))
}

async fn download_fabric(
    http: &reqwest::Client,
    requested_version: &str,
) -> Result<(Vec<u8>, String, String), String> {
    // Resolve stable game version, loader version, and installer version from Fabric Meta API
    let game_versions: Vec<serde_json::Value> = http
        .get("https://meta.fabricmc.net/v2/versions/game")
        .header("User-Agent", "GOLDSMP/1.0")
        .send()
        .await
        .map_err(|e| format!("Errore connessione Fabric Meta: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Errore parsing Fabric game versions: {}", e))?;

    let ver = if !requested_version.is_empty()
        && !requested_version.eq_ignore_ascii_case("latest")
        && game_versions
            .iter()
            .any(|v| v.get("version").and_then(|s| s.as_str()) == Some(requested_version))
    {
        requested_version.to_string()
    } else {
        game_versions
            .iter()
            .find(|v| v.get("stable").and_then(|b| b.as_bool()) == Some(true))
            .or_else(|| game_versions.first())
            .and_then(|v| v.get("version").and_then(|s| s.as_str()))
            .ok_or_else(|| "Nessuna versione Fabric disponibile".to_string())?
            .to_string()
    };

    let loaders: Vec<serde_json::Value> = http
        .get("https://meta.fabricmc.net/v2/versions/loader")
        .header("User-Agent", "GOLDSMP/1.0")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let loader_ver = loaders
        .first()
        .and_then(|l| l.get("version").and_then(|s| s.as_str()))
        .ok_or_else(|| "Loader Fabric non trovato".to_string())?;

    let installers: Vec<serde_json::Value> = http
        .get("https://meta.fabricmc.net/v2/versions/installer")
        .header("User-Agent", "GOLDSMP/1.0")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let installer_ver = installers
        .first()
        .and_then(|i| i.get("version").and_then(|s| s.as_str()))
        .ok_or_else(|| "Installer Fabric non trovato".to_string())?;

    let jar_url = format!(
        "https://meta.fabricmc.net/v2/versions/loader/{}/{}/{}/server/jar",
        ver, loader_ver, installer_ver
    );

    let bytes = http
        .get(&jar_url)
        .header("User-Agent", "GOLDSMP/1.0")
        .send()
        .await
        .map_err(|e| format!("Errore download Fabric server.jar: {}", e))?
        .bytes()
        .await
        .map_err(|e| format!("Errore stream Fabric server.jar: {}", e))?
        .to_vec();

    Ok((
        bytes,
        ver,
        format!("Fabric Loader {}", loader_ver),
    ))
}

async fn download_vanilla(
    http: &reqwest::Client,
    requested_version: &str,
) -> Result<(Vec<u8>, String, String), String> {
    let manifest: serde_json::Value = http
        .get("https://piston-meta.mojang.com/mc/game/version_manifest_v2.json")
        .header("User-Agent", "GOLDSMP/1.0")
        .send()
        .await
        .map_err(|e| format!("Errore connessione Mojang Piston Meta: {}", e))?
        .json()
        .await
        .map_err(|e| format!("Errore parsing manifest Mojang: {}", e))?;

    let latest_release = manifest
        .get("latest")
        .and_then(|l| l.get("release"))
        .and_then(|r| r.as_str())
        .unwrap_or("1.21.4")
        .to_string();

    let versions = manifest
        .get("versions")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "Lista versioni Mojang non trovata".to_string())?;

    let chosen_entry = if !requested_version.is_empty()
        && !requested_version.eq_ignore_ascii_case("latest")
    {
        versions
            .iter()
            .find(|v| v.get("id").and_then(|s| s.as_str()) == Some(requested_version))
            .or_else(|| {
                versions
                    .iter()
                    .find(|v| v.get("id").and_then(|s| s.as_str()) == Some(latest_release.as_str()))
            })
    } else {
        versions
            .iter()
            .find(|v| v.get("id").and_then(|s| s.as_str()) == Some(latest_release.as_str()))
    }
    .or_else(|| versions.first())
    .ok_or_else(|| "Versione Mojang non trovata".to_string())?;

    let ver_id = chosen_entry
        .get("id")
        .and_then(|s| s.as_str())
        .unwrap_or(&latest_release)
        .to_string();
    let pkg_url = chosen_entry
        .get("url")
        .and_then(|s| s.as_str())
        .ok_or_else(|| "URL pacchetto Mojang mancante".to_string())?;

    let pkg_json: serde_json::Value = http
        .get(pkg_url)
        .header("User-Agent", "GOLDSMP/1.0")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let server_jar_url = pkg_json
        .get("downloads")
        .and_then(|d| d.get("server"))
        .and_then(|s| s.get("url"))
        .and_then(|u| u.as_str())
        .ok_or_else(|| "Download server.jar Mojang non disponibile per questa versione".to_string())?;

    let bytes = http
        .get(server_jar_url)
        .header("User-Agent", "GOLDSMP/1.0")
        .send()
        .await
        .map_err(|e| format!("Errore download Vanilla server.jar: {}", e))?
        .bytes()
        .await
        .map_err(|e| e.to_string())?
        .to_vec();

    Ok((bytes, ver_id, "Mojang Official Vanilla".to_string()))
}
