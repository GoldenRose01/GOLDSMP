use crate::auth::{create_jwt, hash_password, verify_password, AuthUser};
use crate::models::{CreateUserReq, LoginReq, LoginRes, User};
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use rusqlite::params;
use uuid::Uuid;

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginReq>,
) -> Result<Json<LoginRes>, (StatusCode, String)> {
    let (mut user, hash) = state
        .db
        .get_user_by_username(&req.username)
        .ok_or((StatusCode::UNAUTHORIZED, "Credenziali non valide".into()))?;

    if !verify_password(&req.password, &hash) {
        return Err((StatusCode::UNAUTHORIZED, "Credenziali non valide".into()));
    }

    let now = Utc::now().to_rfc3339();
    {
        let conn = state.db.conn();
        let _ = conn.execute(
            "UPDATE users SET last_login = ?1 WHERE id = ?2",
            params![now, user.id],
        );
    }
    user.last_login = Some(now);
    state.db.log_audit(&user.username, "AUTH_LOGIN", "panel", "Accesso effettuato", "local");

    let token = create_jwt(
        &user.id,
        &user.username,
        &user.role,
        user.permissions.clone(),
        &state.jwt_secret,
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(LoginRes { token, user }))
}

pub async fn me(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<User>, (StatusCode, String)> {
    let (user, _) = state
        .db
        .get_user_by_username(&claims.username)
        .ok_or((StatusCode::NOT_FOUND, "Utente non trovato".into()))?;
    Ok(Json(user))
}

pub async fn list_users(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<User>>, (StatusCode, String)> {
    if !auth.has_permission("users.read") && auth.0.role != "admin" {
        return Err((StatusCode::FORBIDDEN, "Permessi insufficienti".into()));
    }
    let conn = state.db.conn();
    let mut stmt = conn
        .prepare(
            "SELECT id, username, email, role, permissions_json, oidc_subject, created_at, last_login
             FROM users ORDER BY created_at ASC",
        )
        .unwrap();
    let users: Vec<User> = stmt
        .query_map([], |r| {
            let perms_str: String = r.get(4)?;
            Ok(User {
                id: r.get(0)?,
                username: r.get(1)?,
                email: r.get(2)?,
                role: r.get(3)?,
                permissions: serde_json::from_str(&perms_str).unwrap_or_default(),
                oidc_subject: r.get(5)?,
                created_at: r.get(6)?,
                last_login: r.get(7)?,
            })
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    Ok(Json(users))
}

pub async fn create_user(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CreateUserReq>,
) -> Result<Json<User>, (StatusCode, String)> {
    if !auth.has_permission("users.write") && auth.0.role != "admin" {
        return Err((StatusCode::FORBIDDEN, "Permessi insufficienti".into()));
    }
    if req.username.trim().is_empty() || req.password.len() < 4 {
        return Err((StatusCode::BAD_REQUEST, "Username richiesto e password minimo 4 caratteri".into()));
    }

    let hash = hash_password(&req.password).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let id = Uuid::new_v4().to_string();
    let created_at = Utc::now().to_rfc3339();
    let permissions = req.permissions.unwrap_or_else(|| match req.role.as_str() {
        "superadmin" | "admin" => vec!["*".into()],
        "operator" => vec![
            "instances.read".into(),
            "instances.control".into(),
            "files.write".into(),
            "backups.write".into(),
            "analytics.read".into(),
        ],
        _ => vec!["instances.read".into(), "analytics.read".into()],
    });

    {
        let conn = state.db.conn();
        conn.execute(
            "INSERT INTO users (id, username, email, password_hash, role, permissions_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                id,
                req.username.trim(),
                req.email.trim(),
                hash,
                req.role,
                serde_json::to_string(&permissions).unwrap(),
                created_at
            ],
        )
        .map_err(|e| (StatusCode::CONFLICT, format!("Errore creazione utente: {}", e)))?;
    }

    state.db.log_audit(
        &auth.0.username,
        "USER_CREATE",
        req.username.trim(),
        &format!("Ruolo: {}", req.role),
        "local",
    );

    Ok(Json(User {
        id,
        username: req.username.trim().to_string(),
        email: req.email.trim().to_string(),
        role: req.role,
        permissions,
        oidc_subject: None,
        created_at,
        last_login: None,
    }))
}

pub async fn delete_user(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    if !auth.has_permission("users.write") && auth.0.role != "admin" {
        return Err((StatusCode::FORBIDDEN, "Permessi insufficienti".into()));
    }
    if id == auth.0.sub {
        return Err((StatusCode::BAD_REQUEST, "Non puoi eliminare il tuo stesso account".into()));
    }
    {
        let conn = state.db.conn();
        conn.execute("DELETE FROM users WHERE id = ?1", params![id])
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    state
        .db
        .log_audit(&auth.0.username, "USER_DELETE", &id, "Account rimosso", "local");
    Ok(StatusCode::NO_CONTENT)
}
