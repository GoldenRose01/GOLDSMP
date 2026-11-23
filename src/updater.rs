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

#[derive(Debug, Deserialize)]
struct GithubTag {
    name: String,
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
        let active_ui = Self::read_ui_override_version(&data_dir)
            .map(|ui_v| {
                if compare_semver(&ui_v, &cur) >= 0 {
                    ui_v
                } else {
                    cur.clone()
                }
            })
            .unwrap_or_else(|| cur.clone());
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
            // Initial check 8 seconds after boot so startup stays instant
            tokio::time::sleep(std::time::Duration::from_secs(8)).await;
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

    async fn fetch_latest_release(&self, repo: &str) -> Option<GithubRelease> {
        // 1. Try /releases/latest
        let latest_url = format!("https://api.github.com/repos/{}/releases/latest", repo);
        if let Ok(resp) = self
            .http
            .get(&latest_url)
            .header("User-Agent", "GOLDSMP-Updater/1.0")
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
        {
            if resp.status().is_success() {
                if let Ok(rel) = resp.json::<GithubRelease>().await {
                    return Some(rel);
                }
            }
        }

        // 2. Fallback to /releases (includes prereleases or newest published release)
        let list_url = format!("https://api.github.com/repos/{}/releases?per_page=10", repo);
        if let Ok(resp) = self
            .http
            .get(&list_url)
            .header("User-Agent", "GOLDSMP-Updater/1.0")
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
        {
            if resp.status().is_success() {
                if let Ok(mut rels) = resp.json::<Vec<GithubRelease>>().await {
                    rels.sort_by(|a, b| compare_semver(&b.tag_name, &a.tag_name).cmp(&0));
                    if let Some(best) = rels.into_iter().next() {
                        return Some(best);
                    }
                }
            }
        }
        None
    }

    async fn fetch_highest_tag_version(&self, repo: &str) -> Option<String> {
        let tags_url = format!("https://api.github.com/repos/{}/tags?per_page=20", repo);
        let resp = self
            .http
            .get(&tags_url)
            .header("User-Agent", "GOLDSMP-Updater/1.0")
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
            .ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let tags: Vec<GithubTag> = resp.json().await.ok()?;
        let mut best: Option<String> = None;
        for t in tags {
            let v = t.name.trim_start_matches('v').to_string();
            if best.as_ref().map_or(true, |b| compare_semver(&v, b) > 0) {
                best = Some(v);
            }
        }
        best
    }

    async fn fetch_main_cargo_version(&self, repo: &str) -> Option<String> {
        let raw_url = format!(
            "https://raw.githubusercontent.com/{}/main/Cargo.toml?t={}",
            repo,
            Utc::now().timestamp()
        );
        let text = self
            .http
            .get(&raw_url)
            .header("User-Agent", "GOLDSMP-Updater/1.0")
            .header("Cache-Control", "no-cache")
            .send()
            .await
            .ok()?
            .text()
            .await
            .ok()?;
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("version") && trimmed.contains('=') {
                if let Some(val) = trimmed.split('=').nth(1) {
                    let clean = val.trim().trim_matches('"').trim_matches('\'').trim_start_matches('v');
                    if !clean.is_empty() {
                        return Some(clean.to_string());
                    }
                }
            }
        }
        None
    }

    pub async fn check_and_maybe_download(&self, auto_download: bool) -> UpdateStatus {
        let repo = {
            let mut st = self.status.lock().unwrap();
            st.state = "checking".into();
            st.error = None;
            st.config.repo.clone()
        };

        // Query all 3 sources on GitHub so we NEVER get stuck on 1.0.0 even while GitHub Actions is still building the Release!
        let (release_opt, tag_ver_opt, cargo_ver_opt) = tokio::join!(
            self.fetch_latest_release(&repo),
            self.fetch_highest_tag_version(&repo),
            self.fetch_main_cargo_version(&repo)
        );

        let mut best_ver: Option<String> = None;
        let mut consider = |v: &str| {
            let clean = v.trim().trim_start_matches('v').to_string();
            if !clean.is_empty()
                && best_ver
                    .as_ref()
                    .map_or(true, |cur_best| compare_semver(&clean, cur_best) > 0)
            {
                best_ver = Some(clean);
            }
        };

        if let Some(ref rel) = release_opt {
            consider(&rel.tag_name);
        }
        if let Some(ref tv) = tag_ver_opt {
            consider(tv);
        }
        if let Some(ref cv) = cargo_ver_opt {
            consider(cv);
        }

        let latest_ver = match best_ver {
            Some(v) => v,
            None => {
                let mut st = self.status.lock().unwrap();
                st.state = "error".into();
                st.last_checked = Some(Utc::now().to_rfc3339());
                st.error = Some(format!(
                    "Impossibile leggere la versione da GitHub ({}). Verifica la connessione o il nome repo.",
                    repo
                ));
                return st.clone();
            }
        };

        let (current_bin_ver, current_ui_ver) = {
            let st = self.status.lock().unwrap();
            (st.current_version.clone(), st.active_ui_version.clone())
        };

        // An update is available if either the active UI or the running binary is older than latest_ver
        let is_newer = compare_semver(&latest_ver, &current_ui_ver) > 0
            || compare_semver(&latest_ver, &current_bin_ver) > 0;

        {
            let mut st = self.status.lock().unwrap();
            st.latest_version = Some(latest_ver.clone());
            st.update_available = is_newer;
            st.release_notes = release_opt
                .as_ref()
                .and_then(|r| r.body.clone())
                .unwrap_or_else(|| format!("Aggiornamento GOLDSMP v{} rilevato su GitHub ({})", latest_ver, repo));
            st.release_url = release_opt
                .as_ref()
                .map(|r| r.html_url.clone())
                .unwrap_or_else(|| format!("https://github.com/{}/releases/tag/v{}", repo, latest_ver));
            st.last_checked = Some(Utc::now().to_rfc3339());
            if !is_newer {
                st.state = "up_to_date".into();
                return st.clone();
            }
        }

        if auto_download {
            return self
                .download_and_stage_release(release_opt.as_ref(), &repo, &latest_ver)
                .await;
        }

        let mut st = self.status.lock().unwrap();
        st.state = "idle".into();
        st.clone()
    }

    async fn download_and_stage_release(
        &self,
        release: Option<&GithubRelease>,
        repo: &str,
        latest_ver: &str,
    ) -> UpdateStatus {
        {
            let mut st = self.status.lock().unwrap();
            st.state = "downloading".into();
            st.error = None;
        }

        let mut ui_updated = false;
        let mut bin_staged = false;

        // Only use GitHub Release assets if the release tag matches latest_ver
        let matching_release = release.filter(|r| {
            compare_semver(r.tag_name.trim_start_matches('v'), latest_ver) == 0
        });

        if let Some(rel) = matching_release {
            // 1. Zero-Downtime UI Hot-Swap (frontend-dist.tar.gz) -> user only needs to press F5!
            if let Some(ui_asset) = rel
                .assets
                .iter()
                .find(|a| a.name == "frontend-dist.tar.gz" || a.name.contains("frontend"))
            {
                match self
                    .download_and_extract_ui(&ui_asset.browser_download_url, latest_ver)
                    .await
                {
                    Ok(_) => {
                        ui_updated = true;
                        tracing::info!(
                            "GOLDSMP UI v{} scaricata da GitHub Release e attivata a caldo!",
                            latest_ver
                        );
                    }
                    Err(e) => {
                        tracing::warn!("Errore download UI asset da Release: {}", e);
                    }
                }
            }

            // 2. Atomic Binary Self-Update (stages new binary without killing running game servers)
            let arch_asset_name = if cfg!(target_arch = "aarch64") {
                "goldsmp-linux-aarch64"
            } else {
                "goldsmp-linux-x86_64"
            };
            if let Some(bin_asset) = rel
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
                        "GOLDSMP binario v{} scaricato da GitHub Release e sostituito su disco.",
                        latest_ver
                    );
                }
            }
        }

        // 3. Fallback: If Release assets are not yet built by GitHub Actions (or only a git tag/commit exists),
        // pull and build directly from GitHub source so updating ALWAYS works immediately!
        if !ui_updated || !bin_staged {
            match self.pull_and_build_from_git(repo, latest_ver).await {
                Ok((git_ui, git_bin)) => {
                    if git_ui {
                        ui_updated = true;
                    }
                    if git_bin {
                        bin_staged = true;
                    }
                }
                Err(e) => {
                    if !ui_updated && !bin_staged {
                        let mut st = self.status.lock().unwrap();
                        st.state = "error".into();
                        st.error = Some(format!(
                            "Release v{} trovata ma asset in compilazione su GitHub Actions ({}). Riprova tra 1 minuto.",
                            latest_ver, e
                        ));
                        return st.clone();
                    }
                }
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

    async fn pull_and_build_from_git(
        &self,
        repo: &str,
        latest_ver: &str,
    ) -> Result<(bool, bool), String> {
        // Detect if current working dir is a git workspace, otherwise clone/pull into <data_dir>/git-updater-src
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let src_dir = if cwd.join(".git").exists() && cwd.join("Cargo.toml").exists() {
            cwd
        } else {
            let d = self.data_dir.join("git-updater-src");
            if !d.join(".git").exists() {
                let _ = std::fs::remove_dir_all(&d);
                let clone_url = format!("https://github.com/{}.git", repo);
                let out = tokio::process::Command::new("git")
                    .args(["clone", "--depth", "1", &clone_url, &d.to_string_lossy()])
                    .output()
                    .await
                    .map_err(|e| format!("git clone fallito: {}", e))?;
                if !out.status.success() {
                    return Err(String::from_utf8_lossy(&out.stderr).to_string());
                }
            }
            d
        };

        // Fetch & pull latest changes from origin/main (or tag)
        let _ = tokio::process::Command::new("git")
            .args(["fetch", "--all", "--tags"])
            .current_dir(&src_dir)
            .output()
            .await;
        let _ = tokio::process::Command::new("git")
            .args(["pull", "--ff-only", "origin", "main"])
            .current_dir(&src_dir)
            .output()
            .await;

        let mut ui_ok = false;
        let mut bin_ok = false;

        // Build Frontend and hot-swap into <data_dir>/ui-override
        let fe_dir = src_dir.join("frontend");
        if fe_dir.exists() {
            if !fe_dir.join("node_modules").exists() {
                let _ = tokio::process::Command::new("npm")
                    .arg("install")
                    .current_dir(&fe_dir)
                    .output()
                    .await;
            }
            if let Ok(out) = tokio::process::Command::new("npm")
                .args(["run", "build"])
                .current_dir(&fe_dir)
                .output()
                .await
            {
                if out.status.success() && fe_dir.join("dist").exists() {
                    let final_dir = self.data_dir.join("ui-override");
                    let tmp_dir = self.data_dir.join("ui-override-staging");
                    let _ = std::fs::remove_dir_all(&tmp_dir);
                    if copy_dir_recursive(&fe_dir.join("dist"), &tmp_dir).is_ok() {
                        let _ = std::fs::write(tmp_dir.join(".version"), latest_ver);
                        let _ = std::fs::remove_dir_all(&final_dir);
                        if std::fs::rename(&tmp_dir, &final_dir).is_ok() {
                            ui_ok = true;
                            tracing::info!(
                                "GOLDSMP UI v{} compilata da Git e attivata a caldo in ui-override!",
                                latest_ver
                            );
                        }
                    }
                }
            }
        }

        // Build Rust Binary and stage atomically over current_exe
        let _ = std::fs::File::open(src_dir.join("src/api/mod.rs")).and_then(|f| {
            f.set_modified(std::time::SystemTime::now())
        });
        let is_release = std::env::current_exe()
            .ok()
            .map(|p| p.to_string_lossy().contains("release"))
            .unwrap_or(false);
        let cargo_args: Vec<&str> = if is_release {
            vec!["build", "--release"]
        } else {
            vec!["build"]
        };
        if let Ok(out) = tokio::process::Command::new("cargo")
            .args(&cargo_args)
            .current_dir(&src_dir)
            .output()
            .await
        {
            if out.status.success() {
                let built_bin = if is_release {
                    src_dir.join("target/release/goldsmp")
                } else {
                    src_dir.join("target/debug/goldsmp")
                };
                if built_bin.exists() {
                    if let Ok(bytes) = std::fs::read(&built_bin) {
                        if self.stage_binary_bytes(&bytes).is_ok() {
                            bin_ok = true;
                            tracing::info!(
                                "GOLDSMP binario v{} compilato da Git e sostituito su disco!",
                                latest_ver
                            );
                        }
                    }
                }
            }
        }

        if !ui_ok && !bin_ok {
            return Err("Build locale da Git non disponibile".into());
        }
        Ok((ui_ok, bin_ok))
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
        self.stage_binary_bytes(&bytes)
    }

    fn stage_binary_bytes(&self, bytes: &[u8]) -> Result<(), String> {
        let updates_dir = self.data_dir.join("updates");
        std::fs::create_dir_all(&updates_dir).map_err(|e| e.to_string())?;
        let staged_path = updates_dir.join("goldsmp.new");
        std::fs::write(&staged_path, bytes).map_err(|e| e.to_string())?;

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
            let clean_exe = current_exe
                .to_string_lossy()
                .trim_end_matches(" (deleted)")
                .to_string();
            let target_exe = PathBuf::from(clean_exe);
            let sibling_tmp = target_exe.with_extension("new");
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
                let _ = std::fs::rename(&sibling_tmp, &target_exe);
            }
        }
        Ok(())
    }

    /// Spawns a detached replacement process from the updated binary and exits the current one
    /// so even non-systemd servers can switch to the new backend binary with 1 click.
    pub fn trigger_self_restart(&self) -> Result<(), String> {
        let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let clean_exe = current_exe
            .to_string_lossy()
            .trim_end_matches(" (deleted)")
            .to_string();
        let staged_fallback = self.data_dir.join("updates").join("goldsmp.new");
        let exe_to_run = if Path::new(&clean_exe).exists() {
            PathBuf::from(&clean_exe)
        } else if staged_fallback.exists() {
            staged_fallback
        } else {
            return Err("Binario aggiornato non trovato su disco".into());
        };

        let args: Vec<String> = std::env::args().skip(1).collect();
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(450)).await;
            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt;
                let mut cmd = std::process::Command::new(&exe_to_run);
                cmd.args(&args).current_dir(&cwd);
                // Replace current process image in-place using exec()!
                let err = cmd.exec();
                tracing::error!("exec() fallito durante self-restart: {}", err);
            }
            std::process::exit(0);
        });
        Ok(())
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            std::fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }
    Ok(())
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
