use crate::auth::AuthUser;
use crate::files;
use crate::models::FileEntry;
use crate::state::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Deserialize)]
pub struct PathQuery {
    #[serde(default)]
    pub path: String,
}

#[derive(Serialize)]
pub struct FileContentRes {
    pub path: String,
    pub content: String,
}

#[derive(Deserialize)]
pub struct WriteFileReq {
    pub path: String,
    pub content: String,
}

#[derive(Deserialize)]
pub struct MkdirReq {
    pub path: String,
}

#[derive(Deserialize)]
pub struct RenameReq {
    pub from: String,
    pub to: String,
}

#[derive(Deserialize)]
pub struct DownloadUrlReq {
    pub url: String,
    pub dest_path: String,
}

fn get_root(state: &AppState, id: &str) -> Result<PathBuf, (StatusCode, String)> {
    state
        .pm
        .load_instance(id)
        .map(|i| PathBuf::from(i.work_dir))
        .ok_or((StatusCode::NOT_FOUND, "Istanza non trovata".into()))
}

pub async fn list_files(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<String>,
    Query(q): Query<PathQuery>,
) -> Result<Json<Vec<FileEntry>>, (StatusCode, String)> {
    let root = get_root(&state, &id)?;
    let list = files::list_dir(&root, &q.path).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    Ok(Json(list))
}

pub async fn read_file(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<String>,
    Query(q): Query<PathQuery>,
) -> Result<Json<FileContentRes>, (StatusCode, String)> {
    let root = get_root(&state, &id)?;
    let content = files::read_file_text(&root, &q.path).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    Ok(Json(FileContentRes {
        path: q.path,
        content,
    }))
}

pub async fn write_file(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(req): Json<WriteFileReq>,
) -> Result<StatusCode, (StatusCode, String)> {
    let root = get_root(&state, &id)?;
    files::write_file_text(&root, &req.path, &req.content)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    state.db.log_audit(
        &auth.0.username,
        "FILE_WRITE",
        &format!("{}:{}", id, req.path),
        &format!("{} bytes scritti", req.content.len()),
        "local",
    );
    Ok(StatusCode::OK)
}

pub async fn create_dir(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<String>,
    Json(req): Json<MkdirReq>,
) -> Result<StatusCode, (StatusCode, String)> {
    let root = get_root(&state, &id)?;
    files::create_directory(&root, &req.path).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    Ok(StatusCode::CREATED)
}

pub async fn delete_file(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Query(q): Query<PathQuery>,
) -> Result<StatusCode, (StatusCode, String)> {
    let root = get_root(&state, &id)?;
    files::delete_path(&root, &q.path).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    state.db.log_audit(
        &auth.0.username,
        "FILE_DELETE",
        &format!("{}:{}", id, q.path),
        "File/cartella eliminati",
        "local",
    );
    Ok(StatusCode::NO_CONTENT)
}

pub async fn rename_file(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<String>,
    Json(req): Json<RenameReq>,
) -> Result<StatusCode, (StatusCode, String)> {
    let root = get_root(&state, &id)?;
    files::rename_path(&root, &req.from, &req.to).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    Ok(StatusCode::OK)
}

pub async fn download_from_url(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(req): Json<DownloadUrlReq>,
) -> Result<StatusCode, (StatusCode, String)> {
    let root = get_root(&state, &id)?;
    let target = files::resolve_safe_path(&root, &req.dest_path)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let bytes = state
        .http
        .get(&req.url)
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Errore download: {}", e)))?
        .bytes()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;

    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    std::fs::write(&target, &bytes).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    state.db.log_audit(
        &auth.0.username,
        "FILE_DOWNLOAD_URL",
        &format!("{}:{}", id, req.dest_path),
        &format!("Scaricato da {}", req.url),
        "local",
    );
    Ok(StatusCode::CREATED)
}
