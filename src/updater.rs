use crate::db::Database;
use chrono::Utc;
use flate2::read::GzDecoder;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tar::Archive;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdaterConfig {
    pub repo: String,
    pub auto_check: bool,
    pub auto_download: bool,
    pub check_interval_mins: u64,
}

impl Default for UpdaterConfig {
    fn default() -> Self {
        Self {
            repo: "GoldenRose01/GOLDSMP".into(),
            auto_check: true,
            auto_download: true,
            check_interval_mins: 15,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateStatus {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub active_ui_version: String,
    pub update_available: bool,
    pub ui_hot_updated: bool,
    pub binary_staged: bool,
    pub state: String, // "idle" | "checking" | "downloading" | "ready_refresh" | "up_to_date" | "error"
    pub release_notes: String,
    pub release_url: String,
    pub last_checked: Option<String>,
    pub error: Option<String>,
    pub config: UpdaterConfig,
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
    body: Option<String>,
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

pub struct AutoUpdater {
    db: Database,
    data_dir: PathBuf,
    http: reqwest::Client,
    status: Arc<Mutex<UpdateStatus>>,
}

impl AutoUpdater {
    pub fn new(db: Database, data_dir: PathBuf, http: reqwest::Client) -> Self {
        let cfg = Self::load_config(&db);
        let cur = env!("CARGO_PKG_VERSION").to_string();
        let active_ui = Self::read_ui_override_version(&data_dir).unwrap_or_else(|| cur.clone());
        let status = UpdateStatus {
            current_version: cur,
            latest_version: None,
            active_ui_version: active_ui,
            update_available: false,
            ui_hot_updated: false,
            binary_staged: false,
            state: "idle".into(),
            release_notes: String::new(),
            release_url: String::new(),
            last_checked: None,
            error: None,
            config: cfg,
        };
        Self {
            db,
            data_dir,
            http,
            status: Arc::new(Mutex::new(status)),
        }
    }

    fn load_config(db: &Database) -> UpdaterConfig {
        let conn = db.conn();
        conn.query_row(
            "SELECT value FROM kv_settings WHERE key = 'updater_config'",
            [],
            |r| r.get::<_, String>(0),
        )
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
    }

    pub fn save_config(&self, cfg: UpdaterConfig) -> Result<UpdateStatus, String> {
        {
            let conn = self.db.conn();
            conn.execute(
                "INSERT OR REPLACE INTO kv_settings (key, value) VALUES ('updater_config', ?1)",
                params![serde_json::to_string(&cfg).unwrap()],
            )
            .map_err(|e| e.to_string())?;
        }
        let mut st = self.status.lock().unwrap();
        st.config = cfg;
        Ok(st.clone())
    }

    pub fn get_status(&self) -> UpdateStatus {
        self.status.lock().unwrap().clone()
    }

    fn read_ui_override_version(data_dir: &Path) -> Option<String> {
        std::fs::read_to_string(data_dir.join("ui-override").join(".version"))
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    pub fn spawn_background_loop(self: &Arc<Self>) {
        let updater = Arc::clone(self);
        tokio::spawn(async move {
            // Initial check 12 seconds after boot so startup stays instant
            tokio::time::sleep(std::time::Duration::from_secs(12)).await;
            loop {
                let cfg = updater.get_status().config;
                if cfg.auto_check {
                    let _ = updater.check_and_maybe_download(cfg.auto_download).await;
                }
                let mins = cfg.check_interval_mins.max(5);
                tokio::time::sleep(std::time::Duration::from_secs(mins * 60)).await;
            }
        });
    }

    pub async fn check_and_maybe_download(&self, auto_download: bool) -> UpdateStatus {
        let repo = {
            let mut st = self.status.lock().unwrap();
            st.state = "checking".into();
            st.error = None;
            st.config.repo.clone()
        };

        let url = format!("https://api.github.com/repos/{}/releases/latest", repo);
        let resp = match self
            .http
            .get(&url)
            .header("User-Agent", "GOLDSMP-Updater/1.0")
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                let mut st = self.status.lock().unwrap();
                st.state = "error".into();
                st.last_checked = Some(Utc::now().to_rfc3339());
                st.error = Some(format!("Errore rete GitHub: {}", e));
                return st.clone();
            }
        };

        if resp.status().as_u16() == 404 {
            // Repository has no published GitHub Release yet — completely normal on fresh repo
            let mut st = self.status.lock().unwrap();
            st.state = "up_to_date".into();
            st.update_available = false;
            st.latest_version = Some(st.current_version.clone());
            st.last_checked = Some(Utc::now().to_rfc3339());
            st.error = None;
            return st.clone();
        }

        if !resp.status().is_success() {
            let code = resp.status();
            let mut st = self.status.lock().unwrap();
            st.state = "error".into();
            st.last_checked = Some(Utc::now().to_rfc3339());
            st.error = Some(format!("GitHub API ha risposto con status {}", code));
            return st.clone();
        }

        let release: GithubRelease = match resp.json().await {
            Ok(r) => r,
            Err(e) => {
                let mut st = self.status.lock().unwrap();
                st.state = "error".into();
                st.last_checked = Some(Utc::now().to_rfc3339());
                st.error = Some(format!("Errore parsing release: {}", e));
                return st.clone();
            }
        };

        let latest_ver = release.tag_name.trim_start_matches('v').to_string();
        let current_ver = {
            let st = self.status.lock().unwrap();
            st.active_ui_version.clone()
        };

        let is_newer = compare_semver(&latest_ver, &current_ver) > 0;

        {
            let mut st = self.status.lock().unwrap();
            st.latest_version = Some(latest_ver.clone());
            st.update_available = is_newer;
            st.release_notes = release.body.clone().unwrap_or_default();
            st.release_url = release.html_url.clone();
            st.last_checked = Some(Utc::now().to_rfc3339());
            if !is_newer {
                st.state = "up_to_date".into();
                return st.clone();
            }
        }

        if auto_download {
            return self.download_and_stage_release(&release, &latest_ver).await;
        }

        self.get_status()
    }

    async fn download_and_stage_release(
        &self,
        release: &GithubRelease,
        latest_ver: &str,
    ) -> UpdateStatus {
        {
            let mut st = self.status.lock().unwrap();
            st.state = "downloading".into();
            st.error = None;
        }

        let mut ui_updated = false;
        let mut bin_staged = false;

        // 1. Zero-Downtime UI Hot-Swap (frontend-dist.tar.gz) -> user only needs to press F5!
        if let Some(ui_asset) = release
            .assets
            .iter()
            .find(|a| a.name == "frontend-dist.tar.gz" || a.name.contains("frontend"))
        {
            match self.download_and_extract_ui(&ui_asset.browser_download_url, latest_ver).await {
                Ok(_) => {
                    ui_updated = true;
                    tracing::info!(
                        "GOLDSMP UI v{} scaricata e attivata a caldo! Nessun server interrotto.",
                        latest_ver
                    );
                }
                Err(e) => {
                    let mut st = self.status.lock().unwrap();
                    st.state = "error".into();
                    st.error = Some(format!("Errore aggiornamento UI: {}", e));
                    return st.clone();
                }
            }
        }

        // 2. Atomic Binary Self-Update (stages new binary without killing running game servers)
        let arch_asset_name = if cfg!(target_arch = "aarch64") {
            "goldsmp-linux-aarch64"
        } else {
            "goldsmp-linux-x86_64"
        };
        if let Some(bin_asset) = release
            .assets
            .iter()
            .find(|a| a.name == arch_asset_name || a.name == "goldsmp")
        {
            if self
                .download_and_stage_binary(&bin_asset.browser_download_url)
                .await
                .is_ok()
            {
                bin_staged = true;
                tracing::info!(
                    "GOLDSMP binario v{} scaricato e sostituito atomicamente su disco.",
                    latest_ver
                );
            }
        }

        let mut st = self.status.lock().unwrap();
        if ui_updated {
            st.active_ui_version = latest_ver.to_string();
            st.ui_hot_updated = true;
        }
        if bin_staged {
            st.binary_staged = true;
        }
        if ui_updated || bin_staged {
            st.state = "ready_refresh".into();
            st.update_available = false;
        } else {
            st.state = "idle".into();
        }
        st.clone()
    }

    async fn download_and_extract_ui(&self, url: &str, version: &str) -> Result<(), String> {
        let bytes = self
            .http
            .get(url)
            .header("User-Agent", "GOLDSMP-Updater/1.0")
            .send()
            .await
            .map_err(|e| e.to_string())?
            .bytes()
            .await
            .map_err(|e| e.to_string())?;

        let tmp_dir = self.data_dir.join("ui-override-staging");
        let final_dir = self.data_dir.join("ui-override");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).map_err(|e| e.to_string())?;

        let dec = GzDecoder::new(&bytes[..]);
        let mut archive = Archive::new(dec);
        archive.unpack(&tmp_dir).map_err(|e| e.to_string())?;

        // Write version marker so UI knows the live version
        std::fs::write(tmp_dir.join(".version"), version).map_err(|e| e.to_string())?;

        // Atomic swap of UI directory
        let backup_dir = self.data_dir.join("ui-override-prev");
        let _ = std::fs::remove_dir_all(&backup_dir);
        if final_dir.exists() {
            let _ = std::fs::rename(&final_dir, &backup_dir);
        }
        std::fs::rename(&tmp_dir, &final_dir).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_dir_all(&backup_dir);
        Ok(())
    }

    async fn download_and_stage_binary(&self, url: &str) -> Result<(), String> {
        let bytes = self
            .http
            .get(url)
            .header("User-Agent", "GOLDSMP-Updater/1.0")
            .send()
            .await
            .map_err(|e| e.to_string())?
            .bytes()
            .await
            .map_err(|e| e.to_string())?;

        let updates_dir = self.data_dir.join("updates");
        std::fs::create_dir_all(&updates_dir).map_err(|e| e.to_string())?;
        let staged_path = updates_dir.join("goldsmp.new");
        std::fs::write(&staged_path, &bytes).map_err(|e| e.to_string())?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&staged_path)
                .map_err(|e| e.to_string())?
                .permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&staged_path, perms).map_err(|e| e.to_string())?;
        }

        // On Linux, atomically replacing the executable path while running does NOT interrupt
        // the currently running process or any child game server!
        if let Ok(current_exe) = std::env::current_exe() {
            let sibling_tmp = current_exe.with_extension("new");
            if std::fs::copy(&staged_path, &sibling_tmp).is_ok() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(m) = std::fs::metadata(&sibling_tmp) {
                        let mut p = m.permissions();
                        p.set_mode(0o755);
                        let _ = std::fs::set_permissions(&sibling_tmp, p);
                    }
                }
                let _ = std::fs::rename(&sibling_tmp, &current_exe);
            }
        }
        Ok(())
    }
}

fn compare_semver(a: &str, b: &str) -> i32 {
    let parse = |s: &str| -> Vec<u64> {
        s.trim_start_matches('v')
            .split('.')
            .map(|part| {
                part.chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>()
                    .parse::<u64>()
                    .unwrap_or(0)
            })
            .collect()
    };
    let va = parse(a);
    let vb = parse(b);
    let len = va.len().max(vb.len());
    for i in 0..len {
        let pa = *va.get(i).unwrap_or(&0);
        let pb = *vb.get(i).unwrap_or(&0);
        if pa > pb {
            return 1;
        }
        if pa < pb {
            return -1;
        }
    }
    0
}
