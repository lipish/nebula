use std::sync::atomic::{AtomicU64, Ordering};

use axum::{
    body::Body,
    extract::State,
    http::Request,
    middleware::Next,
    response::{IntoResponse, Response},
    Extension,
};

use crate::auth::{require_role, AuthContext, Role};
use crate::state::AppState;

#[derive(Debug, Default)]
pub struct Metrics {
    pub requests_total: AtomicU64,
    pub requests_inflight: AtomicU64,
    pub status_2xx: AtomicU64,
    pub status_4xx: AtomicU64,
    pub status_5xx: AtomicU64,
    pub auth_missing: AtomicU64,
    pub auth_invalid: AtomicU64,
    pub auth_forbidden: AtomicU64,
    pub auth_rate_limited: AtomicU64,
}

pub fn render_metrics(metrics: &Metrics) -> String {
    format!(
        "nebula_gateway_requests_total {}\nnebula_gateway_requests_inflight {}\nnebula_gateway_responses_2xx {}\nnebula_gateway_responses_4xx {}\nnebula_gateway_responses_5xx {}\nnebula_gateway_auth_missing {}\nnebula_gateway_auth_invalid {}\nnebula_gateway_auth_forbidden {}\nnebula_gateway_auth_rate_limited {}\n",
        metrics.requests_total.load(Ordering::Relaxed),
        metrics.requests_inflight.load(Ordering::Relaxed),
        metrics.status_2xx.load(Ordering::Relaxed),
        metrics.status_4xx.load(Ordering::Relaxed),
        metrics.status_5xx.load(Ordering::Relaxed),
        metrics.auth_missing.load(Ordering::Relaxed),
        metrics.auth_invalid.load(Ordering::Relaxed),
        metrics.auth_forbidden.load(Ordering::Relaxed),
        metrics.auth_rate_limited.load(Ordering::Relaxed),
    )
}

pub async fn metrics_handler(State(st): State<AppState>) -> impl IntoResponse {
    let body = render_metrics(&st.metrics);
    (axum::http::StatusCode::OK, body)
}

pub async fn admin_metrics(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&st.metrics, &ctx, Role::Viewer) {
        return resp;
    }
    let body = render_metrics(&st.metrics);
    (axum::http::StatusCode::OK, body).into_response()
}

pub async fn track_requests(
    State(st): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, std::convert::Infallible> {
    st.metrics.requests_inflight.fetch_add(1, Ordering::Relaxed);
    let resp = next.run(req).await;
    st.metrics.requests_inflight.fetch_sub(1, Ordering::Relaxed);
    st.metrics.requests_total.fetch_add(1, Ordering::Relaxed);

    let status = resp.status().as_u16();
    if status >= 500 {
        st.metrics.status_5xx.fetch_add(1, Ordering::Relaxed);
    } else if status >= 400 {
        st.metrics.status_4xx.fetch_add(1, Ordering::Relaxed);
    } else if status >= 200 {
        st.metrics.status_2xx.fetch_add(1, Ordering::Relaxed);
    }

    Ok(resp)
}
