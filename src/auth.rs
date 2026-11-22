use crate::state::AppState;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{header, request::Parts, StatusCode},
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub username: String,
    pub role: String,
    pub permissions: Vec<String>,
    pub exp: usize,
}

pub fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| e.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    let parsed = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

pub fn create_jwt(
    user_id: &str,
    username: &str,
    role: &str,
    permissions: Vec<String>,
    secret: &str,
) -> Result<String, String> {
    let exp = (Utc::now() + Duration::days(7)).timestamp() as usize;
    let claims = Claims {
        sub: user_id.to_string(),
        username: username.to_string(),
        role: role.to_string(),
        permissions,
        exp,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| e.to_string())
}

pub fn verify_jwt(token: &str, secret: &str) -> Result<Claims, String> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map(|d| d.claims)
    .map_err(|e| e.to_string())
}

#[derive(Debug, Clone)]
pub struct AuthUser(pub Claims);

impl AuthUser {
    pub fn has_full_access(&self) -> bool {
        self.0.role == "superadmin"
            || self.0.role == "admin"
            || self.0.permissions.iter().any(|p| p == "*")
    }

    pub fn has_permission(&self, perm: &str) -> bool {
        self.has_full_access() || self.0.permissions.iter().any(|p| p == perm)
    }

    pub fn can_access_instance(&self, instance_id: &str) -> bool {
        if self.has_full_access() {
            return true;
        }
        let tag = format!("instance:{}", instance_id);
        self.0.permissions.iter().any(|p| p == &tag)
    }
}

#[async_trait]
impl FromRequestParts<AppState> for AuthUser {
    type Rejection = (StatusCode, String);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // Check Authorization: Bearer <token> or query ?token=<token> (for WebSockets)
        let mut token_opt: Option<String> = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
            .map(|s| s.to_string());

        if token_opt.is_none() {
            if let Some(q) = parts.uri.query() {
                for pair in q.split('&') {
                    if let Some(val) = pair.strip_prefix("token=") {
                        token_opt = Some(val.to_string());
                        break;
                    }
                }
            }
        }

        let token = token_opt.ok_or((
            StatusCode::UNAUTHORIZED,
            "Token di autenticazione mancante".to_string(),
        ))?;

        let claims = verify_jwt(&token, &state.jwt_secret).map_err(|_| {
            (
                StatusCode::UNAUTHORIZED,
                "Token non valido o scaduto".to_string(),
            )
        })?;

        Ok(AuthUser(claims))
    }
}
