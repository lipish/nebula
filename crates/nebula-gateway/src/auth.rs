use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use axum::{
    body::Body,
    http::Request,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use tokio::sync::Mutex;

use crate::metrics::Metrics;
use crate::state::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Viewer,
    Operator,
    Admin,
}

impl Role {
    pub fn allows(self, required: Role) -> bool {
        matches!(
            (self, required),
            (Role::Admin, _)
                | (Role::Operator, Role::Viewer | Role::Operator)
                | (Role::Viewer, Role::Viewer)
        )
    }
}

#[derive(Debug, Clone)]
pub struct AuthContext {
    pub principal: String,
    pub role: Role,
}

#[derive(Debug, Clone)]
pub struct AuthState {
    pub enabled: bool,
    pub tokens: Arc<HashMap<String, Role>>,
    pub rate_limits: Arc<Mutex<HashMap<String, RateWindow>>>,
    pub limit_per_minute: u64,
}

#[derive(Debug, Clone)]
pub struct RateWindow {
    pub window_start: Instant,
    pub count: u64,
}

pub fn parse_auth_from_env() -> AuthState {
    let tokens_raw = std::env::var("NEBULA_AUTH_TOKENS").ok();
    let enabled = tokens_raw.is_some();

    let mut tokens = HashMap::new();
    if let Some(raw) = tokens_raw {
        for entry in raw.split(',') {
            let trimmed = entry.trim();
            if trimmed.is_empty() {
                continue;
            }
            let Some((token, role_raw)) = trimmed.split_once(':') else {
                tracing::warn!(entry=%trimmed, "invalid NEBULA_AUTH_TOKENS entry, expected token:role");
                continue;
            };
            let role = match role_raw.to_ascii_lowercase().as_str() {
                "admin" => Role::Admin,
                "operator" => Role::Operator,
                "viewer" => Role::Viewer,
                other => {
                    tracing::warn!(role=%other, "unknown role in NEBULA_AUTH_TOKENS, skipping");
                    continue;
                }
            };
            tokens.insert(token.to_string(), role);
        }
    }

    let limit_per_minute = std::env::var("NEBULA_AUTH_RATE_LIMIT_PER_MINUTE")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(120);

    if !enabled {
        tracing::warn!("auth disabled: NEBULA_AUTH_TOKENS not set");
    }

    AuthState {
        enabled,
        tokens: Arc::new(tokens),
        rate_limits: Arc::new(Mutex::new(HashMap::new())),
        limit_per_minute,
    }
}

pub fn require_role(metrics: &Metrics, ctx: &AuthContext, required: Role) -> Option<Response> {
    if ctx.role.allows(required) {
        None
    } else {
        metrics
            .auth_forbidden
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Some(forbidden("insufficient permissions"))
    }
}

pub async fn admin_auth(
    axum::extract::State(st): axum::extract::State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, std::convert::Infallible> {
    if !st.auth.enabled {
        return Ok(next.run(req).await);
    }

    let token = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string())
        .or_else(|| {
            req.headers()
                .get("x-api-key")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        });

    let Some(token) = token else {
        st.metrics
            .auth_missing
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        return Ok(unauthorized("missing token"));
    };

    let Some(role) = st.auth.tokens.get(&token).copied() else {
        st.metrics
            .auth_invalid
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        return Ok(forbidden("invalid token"));
    };

    if st.auth.limit_per_minute > 0 {
        let mut guard = st.auth.rate_limits.lock().await;
        let entry = guard.entry(token.clone()).or_insert(RateWindow {
            window_start: Instant::now(),
            count: 0,
        });
        let now = Instant::now();
        if now.duration_since(entry.window_start) >= std::time::Duration::from_secs(60) {
            entry.window_start = now;
            entry.count = 0;
        }
        if entry.count >= st.auth.limit_per_minute {
            st.metrics
                .auth_rate_limited
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return Ok(too_many_requests());
        }
        entry.count += 1;
    }

    let ctx = AuthContext {
        principal: token,
        role,
    };
    req.extensions_mut().insert(ctx);

    Ok(next.run(req).await)
}

fn unauthorized(msg: &str) -> Response {
    (
        axum::http::StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({"error": {"message": msg}})),
    )
        .into_response()
}

fn forbidden(msg: &str) -> Response {
    (
        axum::http::StatusCode::FORBIDDEN,
        Json(serde_json::json!({"error": {"message": msg}})),
    )
        .into_response()
}

fn too_many_requests() -> Response {
    (
        axum::http::StatusCode::TOO_MANY_REQUESTS,
        Json(serde_json::json!({"error": {"message": "rate limited"}})),
    )
        .into_response()
}
