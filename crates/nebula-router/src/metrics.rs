use std::sync::atomic::{AtomicU64, Ordering};

use axum::{
    body::Body,
    extract::State,
    http::Request,
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::state::AppState;

#[derive(Debug, Default)]
pub struct Metrics {
    pub requests_total: AtomicU64,
    pub requests_inflight: AtomicU64,
    pub status_2xx: AtomicU64,
    pub status_4xx: AtomicU64,
    pub status_5xx: AtomicU64,
}

pub async fn metrics_handler(State(st): State<AppState>) -> impl IntoResponse {
    let body = format!(
        "nebula_router_requests_total {}\nnebula_router_requests_inflight {}\nnebula_router_responses_2xx {}\nnebula_router_responses_4xx {}\nnebula_router_responses_5xx {}\n",
        st.metrics.requests_total.load(Ordering::Relaxed),
        st.metrics.requests_inflight.load(Ordering::Relaxed),
        st.metrics.status_2xx.load(Ordering::Relaxed),
        st.metrics.status_4xx.load(Ordering::Relaxed),
        st.metrics.status_5xx.load(Ordering::Relaxed),
    );
    (axum::http::StatusCode::OK, body)
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
