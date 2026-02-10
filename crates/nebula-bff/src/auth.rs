use std::env;

use axum::{
    extract::State,
    http::{header, Request, StatusCode},
    middleware::Next,
    response::IntoResponse,
};

use crate::state::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Admin,
    Operator,
    Viewer,
}

#[derive(Debug, Clone)]
pub struct AuthContext {
    pub principal: String,
    pub role: Role,
}

#[derive(Debug, Clone, Default)]
pub struct AuthConfig {
    pub admin_token: Option<String>,
    pub operator_token: Option<String>,
    pub viewer_token: Option<String>,
}

pub fn parse_auth_from_env() -> AuthConfig {
    AuthConfig {
        admin_token: env::var("BFF_ADMIN_TOKEN").ok(),
        operator_token: env::var("BFF_OPERATOR_TOKEN").ok(),
        viewer_token: env::var("BFF_VIEWER_TOKEN").ok(),
    }
}

pub async fn auth_middleware(
    State(st): State<AppState>,
    mut req: Request<axum::body::Body>,
    next: Next,
) -> impl IntoResponse {
    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    let token = auth_header.strip_prefix("Bearer ").unwrap_or("").trim();

    let ctx = if token.is_empty() && st.auth.is_empty() {
        Some(AuthContext {
            principal: "anonymous".to_string(),
            role: Role::Admin,
        })
    } else if let Some(ctx) = st.auth.match_token(token) {
        Some(ctx)
    } else {
        None
    };

    let Some(ctx) = ctx else {
        return (StatusCode::UNAUTHORIZED, "unauthorized").into_response();
    };

    req.extensions_mut().insert(ctx);
    next.run(req).await
}

impl AuthConfig {
    fn is_empty(&self) -> bool {
        self.admin_token.is_none() && self.operator_token.is_none() && self.viewer_token.is_none()
    }

    fn match_token(&self, token: &str) -> Option<AuthContext> {
        if token.is_empty() {
            return None;
        }

        if let Some(admin) = &self.admin_token {
            if token == admin {
                return Some(AuthContext {
                    principal: "admin".to_string(),
                    role: Role::Admin,
                });
            }
        }

        if let Some(operator) = &self.operator_token {
            if token == operator {
                return Some(AuthContext {
                    principal: "operator".to_string(),
                    role: Role::Operator,
                });
            }
        }

        if let Some(viewer) = &self.viewer_token {
            if token == viewer {
                return Some(AuthContext {
                    principal: "viewer".to_string(),
                    role: Role::Viewer,
                });
            }
        }

        None
    }
}
