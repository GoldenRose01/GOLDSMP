use crate::auth::{create_jwt, hash_password, verify_password, AuthUser};
use crate::mailer::send_account_active_email;
use crate::models::{CreateUserReq, LoginReq, LoginRes, User};
use crate::security::extract_client_ip;
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::Utc;
use rusqlite::params;
use serde::Deserialize;
use uuid::Uuid;

pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<LoginReq>,
) -> Result<Json<LoginRes>, (StatusCode, String)> {
    let ip = extract_client_ip(&headers);

    let (mut user, hash) = match state.db.get_user_by_username(&req.username) {
        Some(u) => u,
        None => {
            state.security.record_login_failure(&ip);
            return Err((StatusCode::UNAUTHORIZED, "Credenziali non valide".into()));
        }
    };

    if !verify_password(&req.password, &hash) {
        state.security.record_login_failure(&ip);
        state.db.log_audit(
            &req.username,
            "AUTH_FAILED",
            "panel",
            "Tentativo di login fallito",
            &ip,
        );
        return Err((StatusCode::UNAUTHORIZED, "Credenziali non valide".into()));
    }

    state.security.clear_login_failures(&ip);

    let now = Utc::now().to_rfc3339();
    {
        let conn = state.db.conn();
        let _ = conn.execute(
            "UPDATE users SET last_login = ?1 WHERE id = ?2",
            params![now, user.id],
        );
    }
    user.last_login = Some(now);
    state
        .db
        .log_audit(&user.username, "AUTH_LOGIN", "panel", "Accesso effettuato", &ip);

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
    _auth: AuthUser,
) -> Result<Json<Vec<User>>, (StatusCode, String)> {
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
    if !auth.has_permission("users.write") && !auth.has_full_access() {
        return Err((StatusCode::FORBIDDEN, "Permessi insufficienti".into()));
    }
    if req.username.trim().is_empty() || req.password.len() < 4 {
        return Err((
            StatusCode::BAD_REQUEST,
            "Username richiesto e password minimo 4 caratteri".into(),
        ));
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

    let scope_desc = if permissions.iter().any(|p| p == "*") {
        "Intero Server (Tutte le istanze)".to_string()
    } else {
        let insts: Vec<&str> = permissions
            .iter()
            .filter_map(|p| p.strip_prefix("instance:"))
            .collect();
        if insts.is_empty() {
            "Accesso Globale Limitato".to_string()
        } else {
            format!("Istanze assegnate: {}", insts.join(", "))
        }
    };

    // Send "Il tuo account per GOLDSMP è attivo" email notification
    let mail_status = send_account_active_email(
        &state.db,
        req.email.trim(),
        req.username.trim(),
        &req.role,
        &scope_desc,
    );

    state.db.log_audit(
        &auth.0.username,
        "USER_CREATE",
        req.username.trim(),
        &format!("Ruolo: {} | Scope: {} | Email: {}", req.role, scope_desc, mail_status),
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

#[derive(Deserialize)]
pub struct GrantInstanceReq {
    pub instance_id: String,
    #[serde(default)]
    pub full_server: bool,
}

pub async fn grant_user_instance(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
    Json(req): Json<GrantInstanceReq>,
) -> Result<Json<User>, (StatusCode, String)> {
    if !auth.has_full_access() && !auth.can_access_instance(&req.instance_id) {
        return Err((StatusCode::FORBIDDEN, "Permessi insufficienti".into()));
    }

    let mut user = {
        let conn = state.db.conn();
        conn.query_row(
            "SELECT id, username, email, role, permissions_json, oidc_subject, created_at, last_login
             FROM users WHERE id = ?1",
            params![id],
            |r| {
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
            },
        )
        .map_err(|_| (StatusCode::NOT_FOUND, "Utente non trovato".to_string()))?
    };

    if req.full_server {
        if !user.permissions.iter().any(|p| p == "*") {
            user.permissions.push("*".into());
        }
    } else {
        let tag = format!("instance:{}", req.instance_id);
        if !user.permissions.iter().any(|p| p == &tag) {
            user.permissions.push(tag);
        }
    }

    {
        let conn = state.db.conn();
        conn.execute(
            "UPDATE users SET permissions_json = ?1 WHERE id = ?2",
            params![serde_json::to_string(&user.permissions).unwrap(), user.id],
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    state.db.log_audit(
        &auth.0.username,
        "USER_GRANT_INSTANCE",
        &user.username,
        &format!("Aggiunto permesso (+) per istanza {}", req.instance_id),
        "local",
    );

    Ok(Json(user))
}

pub async fn revoke_user_instance(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((id, instance_id)): Path<(String, String)>,
) -> Result<Json<User>, (StatusCode, String)> {
    if !auth.has_full_access() && !auth.can_access_instance(&instance_id) {
        return Err((StatusCode::FORBIDDEN, "Permessi insufficienti".into()));
    }

    let mut user = {
        let conn = state.db.conn();
        conn.query_row(
            "SELECT id, username, email, role, permissions_json, oidc_subject, created_at, last_login
             FROM users WHERE id = ?1",
            params![id],
            |r| {
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
            },
        )
        .map_err(|_| (StatusCode::NOT_FOUND, "Utente non trovato".to_string()))?
    };

    let tag = format!("instance:{}", instance_id);
    user.permissions.retain(|p| p != &tag);

    {
        let conn = state.db.conn();
        conn.execute(
            "UPDATE users SET permissions_json = ?1 WHERE id = ?2",
            params![serde_json::to_string(&user.permissions).unwrap(), user.id],
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    state.db.log_audit(
        &auth.0.username,
        "USER_REVOKE_INSTANCE",
        &user.username,
        &format!("Rimosso permesso per istanza {}", instance_id),
        "local",
    );

    Ok(Json(user))
}

pub async fn delete_user(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    if !auth.has_permission("users.write") && !auth.has_full_access() {
        return Err((StatusCode::FORBIDDEN, "Permessi insufficienti".into()));
    }
    if id == auth.0.sub {
        return Err((
            StatusCode::BAD_REQUEST,
            "Non puoi eliminare il tuo stesso account".into(),
        ));
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
