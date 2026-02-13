// Thin wrapper around nebula_common::auth with backward-compatible env parsing.

pub use nebula_common::auth::{require_role, AuthConfig, AuthContext, Role};

use std::collections::HashMap;
use std::sync::Arc;

/// Parse BFF auth configuration from environment variables.
///
/// 1. First tries the shared `NEBULA_AUTH_TOKENS` format (via nebula_common).
/// 2. If that returns `enabled: false`, falls back to the legacy per-role
///    env vars: `BFF_ADMIN_TOKEN`, `BFF_OPERATOR_TOKEN`, `BFF_VIEWER_TOKEN`.
pub fn parse_bff_auth_from_env() -> AuthConfig {
    let shared = nebula_common::auth::parse_auth_from_env();
    if shared.enabled {
        return shared;
    }

    // Legacy fallback: read individual BFF_*_TOKEN env vars.
    let admin = std::env::var("BFF_ADMIN_TOKEN").ok();
    let operator = std::env::var("BFF_OPERATOR_TOKEN").ok();
    let viewer = std::env::var("BFF_VIEWER_TOKEN").ok();

    let has_any = admin.is_some() || operator.is_some() || viewer.is_some();

    let mut tokens = HashMap::new();
    if let Some(t) = admin {
        tokens.insert(t, Role::Admin);
    }
    if let Some(t) = operator {
        tokens.insert(t, Role::Operator);
    }
    if let Some(t) = viewer {
        tokens.insert(t, Role::Viewer);
    }

    if !has_any {
        tracing::warn!("auth disabled: neither NEBULA_AUTH_TOKENS nor BFF_*_TOKEN vars set");
    }

    AuthConfig {
        enabled: has_any,
        tokens: Arc::new(tokens),
        rate_limits: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        limit_per_minute: std::env::var("NEBULA_AUTH_RATE_LIMIT_PER_MINUTE")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(120),
    }
}
