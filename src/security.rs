use axum::{
    body::Body,
    extract::Request,
    http::{header, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct IpBucket {
    api_requests: u32,
    login_failures: u32,
    window_start: Instant,
    banned_until: Option<Instant>,
}

#[derive(Clone)]
pub struct SecurityGuard {
    buckets: Arc<Mutex<HashMap<String, IpBucket>>>,
    max_api_per_min: u32,
    max_login_failures: u32,
}

impl SecurityGuard {
    pub fn new() -> Self {
        Self {
            buckets: Arc::new(Mutex::new(HashMap::new())),
            max_api_per_min: 300,
            max_login_failures: 8,
        }
    }

    pub fn check_rate_limit(&self, ip: &str, is_login: bool) -> Result<(), (StatusCode, String)> {
        let now = Instant::now();
        let mut map = self.buckets.lock().unwrap();

        // Periodic cleanup if map grows large during DDoS
        if map.len() > 10_000 {
            map.retain(|_, b| now.duration_since(b.window_start) < Duration::from_secs(300));
        }

        let bucket = map.entry(ip.to_string()).or_insert(IpBucket {
            api_requests: 0,
            login_failures: 0,
            window_start: now,
            banned_until: None,
        });

        if let Some(until) = bucket.banned_until {
            if now < until {
                let secs = until.duration_since(now).as_secs().max(1);
                return Err((
                    StatusCode::TOO_MANY_REQUESTS,
                    format!(
                        "Protezione anti-DDoS / Brute-Force attiva: riprova tra {} secondi",
                        secs
                    ),
                ));
            } else {
                bucket.banned_until = None;
                bucket.login_failures = 0;
                bucket.api_requests = 0;
                bucket.window_start = now;
            }
        }

        if now.duration_since(bucket.window_start) > Duration::from_secs(60) {
            bucket.window_start = now;
            bucket.api_requests = 0;
        }

        bucket.api_requests += if is_login { 5 } else { 1 };
        if bucket.api_requests > self.max_api_per_min {
            bucket.banned_until = Some(now + Duration::from_secs(60));
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                "Rate limit superato (Protezione Anti-DDoS GOLDSMP)".into(),
            ));
        }

        Ok(())
    }

    pub fn record_login_failure(&self, ip: &str) {
        let now = Instant::now();
        let mut map = self.buckets.lock().unwrap();
        let bucket = map.entry(ip.to_string()).or_insert(IpBucket {
            api_requests: 0,
            login_failures: 0,
            window_start: now,
            banned_until: None,
        });
        bucket.login_failures += 1;
        if bucket.login_failures >= self.max_login_failures {
            bucket.banned_until = Some(now + Duration::from_secs(300));
        }
    }

    pub fn clear_login_failures(&self, ip: &str) {
        let mut map = self.buckets.lock().unwrap();
        if let Some(b) = map.get_mut(ip) {
            b.login_failures = 0;
            b.banned_until = None;
        }
    }
}

pub fn extract_client_ip(headers: &axum::http::HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "127.0.0.1".into())
}

pub async fn security_middleware(
    axum::extract::State(state): axum::extract::State<crate::state::AppState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let path = req.uri().path().to_string();
    if path.starts_with("/api/") {
        let ip = extract_client_ip(req.headers());
        let is_login = path == "/api/auth/login";
        if let Err((code, msg)) = state.security.check_rate_limit(&ip, is_login) {
            return (code, msg).into_response();
        }
    }

    let mut res = next.run(req).await;
    let headers = res.headers_mut();
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("SAMEORIGIN"));
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    headers.insert(
        "X-XSS-Protection",
        HeaderValue::from_static("1; mode=block"),
    );
    res
}
