use crate::db::Database;
use crate::process_manager::ProcessManager;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub pm: Arc<ProcessManager>,
    pub data_dir: PathBuf,
    pub jwt_secret: String,
    pub http: reqwest::Client,
}
