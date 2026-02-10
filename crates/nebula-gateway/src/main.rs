mod engine;
mod responses;

use axum::{
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, HeaderName, HeaderValue, Request, StatusCode},
    middleware,
    middleware::Next,
    response::{sse::Event, IntoResponse, Response, Sse},
    routing::{get, post},
    Extension, Json, Router,
};
use bytes::Bytes;
use engine::{EngineClient, OpenAIEngineClient};
use nebula_common::ExecutionContext;
use serde_json::json;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::env;
use std::fs;
use std::sync::Arc;
use std::{convert::Infallible, time::Duration};
use std::time::Instant;
use tracing_subscriber::EnvFilter;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio_stream::{wrappers::ReceiverStream, StreamExt};
use uuid::Uuid;

use responses::{build_non_stream_json, build_response, CreateResponseRequest, ResponseStreamBuilder};

#[derive(Clone)]
struct AppState {
    _noop: Arc<()>,
    engine: Arc<dyn EngineClient>,
    router_base_url: String,
    http: reqwest::Client,
    store: Arc<EtcdMetaStore>,
    auth: AuthState,
    metrics: Arc<Metrics>,
    log_path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    Viewer,
    Operator,
    Admin,
}

impl Role {
    fn allows(self, required: Role) -> bool {
        matches!(
            (self, required),
            (Role::Admin, _)
                | (Role::Operator, Role::Viewer | Role::Operator)
                | (Role::Viewer, Role::Viewer)
        )
    }
}

#[derive(Debug, Clone)]
struct AuthContext {
    principal: String,
    role: Role,
}

#[derive(Debug, Clone)]
struct AuthState {
    enabled: bool,
    tokens: Arc<HashMap<String, Role>>, // token -> role
    rate_limits: Arc<Mutex<HashMap<String, RateWindow>>>,
    limit_per_minute: u64,
}

#[derive(Debug, Clone)]
struct RateWindow {
    window_start: Instant,
    count: u64,
}

#[derive(Debug, Default)]
struct Metrics {
    requests_total: AtomicU64,
    requests_inflight: AtomicU64,
    status_2xx: AtomicU64,
    status_4xx: AtomicU64,
    status_5xx: AtomicU64,
    auth_missing: AtomicU64,
    auth_invalid: AtomicU64,
    auth_forbidden: AtomicU64,
    auth_rate_limited: AtomicU64,
}

async fn create_responses(
    State(_st): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateResponseRequest>,
) -> Response {
    let _ctx = build_execution_context(&headers);

    let input_text = req.extract_input_text();

    if req.stream.unwrap_or(false) {
        let (tx, rx) = mpsc::channel::<Result<Event, Infallible>>(256);
        let engine = _st.engine.clone();

        let builder = ResponseStreamBuilder::new(&req);

        tokio::spawn(async move {
            let mut stream = engine.stream_text(input_text);
            let mut builder = builder;

            let _ = tx
                .send(Ok(Event::default().data(builder.created_event().to_string())))
                .await;

            while let Some(delta) = stream.next().await {
                if delta.is_empty() {
                    continue;
                }
                let ev = builder.push_delta(delta);
                let _ = tx.send(Ok(Event::default().data(ev.to_string()))).await;
            }

            let completed = builder.completed_event();
            let _ = tx
                .send(Ok(Event::default().data(completed.to_string())))
                .await;
        });

        Sse::new(ReceiverStream::new(rx))
            .keep_alive(axum::response::sse::KeepAlive::new().interval(Duration::from_secs(15)))
            .into_response()
    } else {
        let mut out = String::new();
        let mut stream = _st.engine.stream_text(input_text);
        while let Some(delta) = stream.next().await {
            if !delta.is_empty() {
                out.push_str(&delta);
            }
        }
        let built = build_response(&req, out);
        let body = build_non_stream_json(&built);
        (StatusCode::OK, Json(body)).into_response()
    }
}

fn build_execution_context(headers: &HeaderMap) -> ExecutionContext {
    let session_id = headers
        .get("x-session-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    ExecutionContext {
        request_id: format!("req_{}", Uuid::new_v4()),
        session_id,
        tenant_id: None,
        priority: None,
        deadline_ms: None,
        budget_tokens: None,
    }
}

async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

async fn not_implemented(State(_st): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let ctx = build_execution_context(&headers);
    let body = json!({
        "error": {
            "message": "not implemented",
            "type": "nebula_gateway_not_implemented",
            "request_id": ctx.request_id
        }
    });
    (StatusCode::NOT_IMPLEMENTED, Json(body))
}

fn require_role(metrics: &Metrics, ctx: &AuthContext, required: Role) -> Option<Response> {
    if ctx.role.allows(required) {
        None
    } else {
        metrics.auth_forbidden.fetch_add(1, Ordering::Relaxed);
        Some(forbidden("insufficient permissions"))
    }
}

fn read_engine_env_file(path: &str) -> Option<(String, String)> {
    let content = fs::read_to_string(path).ok()?;
    let mut base_url: Option<String> = None;
    let mut model: Option<String> = None;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (k, v) = line.split_once('=')?;
        let k = k.trim();
        let v = v.trim();
        match k {
            "NEBULA_ENGINE_BASE_URL" => base_url = Some(v.to_string()),
            "NEBULA_ENGINE_MODEL" => model = Some(v.to_string()),
            _ => {}
        }
    }
    Some((base_url?, model?))
}

#[derive(Debug, serde::Deserialize)]
struct LogsQuery {
    lines: Option<usize>,
}

fn parse_auth_from_env() -> AuthState {
    let tokens_raw = env::var("NEBULA_AUTH_TOKENS").ok();
    let enabled = tokens_raw.is_some();

    let mut tokens = HashMap::new();
    if let Some(raw) = tokens_raw {
        for entry in raw.split(',') {
            let trimmed = entry.trim();
            if trimmed.is_empty() {
                continue;
            }
            // Format: token:role (role in {admin,operator,viewer})
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

    let limit_per_minute = env::var("NEBULA_AUTH_RATE_LIMIT_PER_MINUTE")
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

fn unauthorized(msg: &str) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({"error": {"message": msg}})),
    )
        .into_response()
}

fn forbidden(msg: &str) -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(json!({"error": {"message": msg}})),
    )
        .into_response()
}

fn too_many_requests() -> Response {
    (
        StatusCode::TOO_MANY_REQUESTS,
        Json(json!({"error": {"message": "rate limited"}})),
    )
        .into_response()
}

fn render_metrics(metrics: &Metrics) -> String {
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

async fn metrics_handler(State(st): State<AppState>) -> impl IntoResponse {
    let body = render_metrics(&st.metrics);
    (StatusCode::OK, body)
}

async fn admin_metrics(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&st.metrics, &ctx, Role::Viewer) {
        return resp;
    }
    let body = render_metrics(&st.metrics);
    (StatusCode::OK, body).into_response()
}

async fn admin_logs(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    axum::extract::Query(query): axum::extract::Query<LogsQuery>,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&st.metrics, &ctx, Role::Viewer) {
        return resp;
    }

    let lines = query.lines.unwrap_or(200).min(2000);
    let content = std::fs::read_to_string(&st.log_path).unwrap_or_default();
    let mut out_lines: Vec<&str> = content.lines().collect();
    if out_lines.len() > lines {
        out_lines = out_lines[out_lines.len() - lines..].to_vec();
    }
    (StatusCode::OK, out_lines.join("\n")).into_response()
}

async fn track_requests(
    State(st): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, Infallible> {
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

async fn admin_auth(State(st): State<AppState>, mut req: Request<Body>, next: Next) -> Result<Response, Infallible> {
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
        st.metrics.auth_missing.fetch_add(1, Ordering::Relaxed);
        return Ok(unauthorized("missing token"));
    };

    let Some(role) = st.auth.tokens.get(&token).copied() else {
        st.metrics.auth_invalid.fetch_add(1, Ordering::Relaxed);
        return Ok(forbidden("invalid token"));
    };

    // Simple fixed-window rate limit per token
    if st.auth.limit_per_minute > 0 {
        let mut guard = st.auth.rate_limits.lock().await;
        let entry = guard.entry(token.clone()).or_insert(RateWindow {
            window_start: Instant::now(),
            count: 0,
        });
        let now = Instant::now();
        if now.duration_since(entry.window_start) >= Duration::from_secs(60) {
            entry.window_start = now;
            entry.count = 0;
        }
        if entry.count >= st.auth.limit_per_minute {
            st.metrics.auth_rate_limited.fetch_add(1, Ordering::Relaxed);
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

fn to_reqwest_headers(headers: &HeaderMap) -> reqwest::header::HeaderMap {
    let mut out = reqwest::header::HeaderMap::new();
    for (k, v) in headers.iter() {
        if k.as_str().eq_ignore_ascii_case("host") || k.as_str().eq_ignore_ascii_case("content-length") {
            continue;
        }
        out.insert(k, v.clone());
    }
    out
}

fn append_headers(src: &reqwest::header::HeaderMap, dst: &mut Response) {
    for (k, v) in src.iter() {
        if let (Ok(name), Ok(value)) = (
            HeaderName::from_bytes(k.as_str().as_bytes()),
            HeaderValue::from_bytes(v.as_bytes()),
        ) {
            dst.headers_mut().insert(name, value);
        }
    }
}

async fn proxy_post(State(st): State<AppState>, headers: HeaderMap, req: Request<Body>) -> Response {
    let base = st.router_base_url.trim_end_matches('/');
    let uri_path = req.uri().path().to_string();
    let uri_query = req
        .uri()
        .query()
        .map(|q| format!("?{q}"))
        .unwrap_or_default();
    let url = format!("{base}{uri_path}{uri_query}");

    let body_bytes = match axum::body::to_bytes(req.into_body(), usize::MAX).await {
        Ok(b) => b,
        Err(_) => return (StatusCode::BAD_REQUEST, "invalid body").into_response(),
    };

    let original_body_bytes = body_bytes.clone();

    let resp = match st
        .http
        .post(url)
        .headers(to_reqwest_headers(&headers))
        .body(body_bytes)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error=%e, "upstream request failed");
            return (StatusCode::BAD_GATEWAY, "upstream request failed").into_response();
        }
    };

    let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let is_sse = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.contains("text/event-stream"))
        .unwrap_or(false);
    let resp_headers = resp.headers().clone();

    if is_sse {
        let mut upstream = resp.bytes_stream();
        let (tx, rx) = mpsc::channel::<Result<Bytes, Infallible>>(64);
        tokio::spawn(async move {
            while let Some(item) = upstream.next().await {
                match item {
                    Ok(b) => {
                        let _ = tx.send(Ok(b)).await;
                    }
                    Err(_) => break,
                }
            }
        });

        let stream = ReceiverStream::new(rx);
        let mut out = Response::builder()
            .status(status)
            .header("content-type", "text/event-stream")
            .body(Body::from_stream(stream))
            .unwrap();
        append_headers(&resp_headers, &mut out);
        return out;
    }

    let bytes = resp.bytes().await.unwrap_or_default();
    let mut out = Response::builder()
        .status(status)
        .body(Body::from(bytes))
        .unwrap();
    append_headers(&resp_headers, &mut out);
    out
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let router_base_url = env::var("NEBULA_ROUTER_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:18081".to_string());

    let engine_model = env::var("NEBULA_ENGINE_MODEL")
        .ok()
        .or_else(|| {
            read_engine_env_file("/tmp/nebula/engine.env").map(|(_url, model)| model)
        })
        .unwrap_or_else(|| "unknown".to_string());

    tracing::info!(router_base_url=%router_base_url, engine_model=%engine_model, "gateway starting");

    let engine: Arc<dyn EngineClient> =
        Arc::new(OpenAIEngineClient::new(router_base_url.clone(), engine_model));

    let http = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(300))
        .build()
        .expect("reqwest client");

    let etcd_endpoint = env::var("ETCD_ENDPOINT").unwrap_or_else(|_| "http://127.0.0.1:2379".to_string());
    let store = EtcdMetaStore::connect(&vec![etcd_endpoint]).await.expect("etcd connect");

    let auth = parse_auth_from_env();

    let metrics = Arc::new(Metrics::default());

    let log_path = env::var("NEBULA_GATEWAY_LOG_PATH")
        .unwrap_or_else(|_| "/tmp/nebula-gateway.log".to_string());

    let st = AppState {
        _noop: Arc::new(()),
        engine,
        router_base_url,
        http,
        store: Arc::new(store),
        auth,
        metrics,
        log_path,
    };

    let admin_routes = Router::new()
        .route("/models/load", post(admin_load_model))
        .route("/models/requests", get(admin_list_requests))
        .route("/models/requests/:id", post(admin_delete_request).delete(admin_delete_request))
        .route("/cluster/status", get(admin_cluster_status))
        .route("/whoami", get(admin_whoami))
        .route("/metrics", get(admin_metrics))
        .route("/logs", get(admin_logs))
        .layer(middleware::from_fn_with_state(st.clone(), admin_auth))
        .with_state(st.clone());

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/health", get(healthz))
        .route("/metrics", get(metrics_handler))
        .route("/v1/responses", get(not_implemented).post(create_responses))
        .route("/v1/chat/completions", post(proxy_post))
        .route("/v1/embeddings", post(proxy_post))
        .route("/v1/rerank", post(proxy_post))
        .route("/v1/models", get(list_models))
        .route("/v1/admin/ui", get(admin_ui))
        .nest("/v1/admin", admin_routes)
        .layer(middleware::from_fn_with_state(st.clone(), track_requests))
        .with_state(st);

    let addr = env::var("NEBULA_GATEWAY_ADDR").unwrap_or_else(|_| {
        if let Ok(port) = env::var("PORT") {
            format!("0.0.0.0:{port}")
        } else {
            "0.0.0.0:8080".to_string()
        }
    });

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("bind");

    axum::serve(listener, app).await.expect("serve");
}

use nebula_common::{ClusterStatus, ModelLoadRequest, ModelRequest, ModelRequestStatus, NodeStatus, PlacementPlan, EndpointInfo};
use nebula_meta::{EtcdMetaStore, MetaStore};

async fn admin_cluster_status(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&st.metrics, &ctx, Role::Viewer) {
        return resp;
    }
    let nodes_raw = match st.store.list_prefix("/nodes/").await {
        Ok(n) => n,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("etcd error: {}", e)).into_response(),
    };
    let mut nodes = Vec::new();
    for (_, v, _) in nodes_raw {
        if let Ok(n) = serde_json::from_slice::<NodeStatus>(&v) {
            nodes.push(n);
        }
    }

    let endpoints_raw = match st.store.list_prefix("/endpoints/").await {
        Ok(e) => e,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("etcd error: {}", e)).into_response(),
    };
    let mut endpoints = Vec::new();
    for (_, v, _) in endpoints_raw {
        if let Ok(ep) = serde_json::from_slice::<EndpointInfo>(&v) {
            endpoints.push(ep);
        }
    }

    let placements_raw = match st.store.list_prefix("/placements/").await {
        Ok(p) => p,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("etcd error: {}", e)).into_response(),
    };
    let mut placements = Vec::new();
    for (_, v, _) in placements_raw {
        if let Ok(p) = serde_json::from_slice::<PlacementPlan>(&v) {
            placements.push(p);
        }
    }

    let requests_raw = match st.store.list_prefix("/model_requests/").await {
        Ok(r) => r,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("etcd error: {}", e)).into_response(),
    };
    let mut model_requests = Vec::new();
    for (_, v, _) in requests_raw {
        if let Ok(r) = serde_json::from_slice::<ModelRequest>(&v) {
            model_requests.push(r);
        }
    }

    let status = ClusterStatus {
        nodes,
        endpoints,
        placements,
        model_requests,
    };

    (StatusCode::OK, Json(status)).into_response()
}

async fn admin_list_requests(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&st.metrics, &ctx, Role::Viewer) {
        return resp;
    }
    let requests_raw = match st.store.list_prefix("/model_requests/").await {
        Ok(r) => r,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("etcd error: {}", e)).into_response(),
    };
    let mut model_requests = Vec::new();
    for (_, v, _) in requests_raw {
        if let Ok(r) = serde_json::from_slice::<ModelRequest>(&v) {
            model_requests.push(r);
        }
    }
    (StatusCode::OK, Json(model_requests)).into_response()
}

async fn admin_whoami(
    Extension(ctx): Extension<AuthContext>,
) -> impl IntoResponse {
    let role = match ctx.role {
        Role::Admin => "admin",
        Role::Operator => "operator",
        Role::Viewer => "viewer",
    };
    (StatusCode::OK, Json(json!({
        "principal": ctx.principal,
        "role": role,
    })))
        .into_response()
}

async fn list_models(State(st): State<AppState>) -> impl IntoResponse {
    let placements_raw = match st.store.list_prefix("/placements/").await {
        Ok(p) => p,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("etcd error: {}", e)).into_response(),
    };

    let mut models = Vec::new();
    for (key, val, _) in placements_raw {
        if let Ok(plan) = serde_json::from_slice::<PlacementPlan>(&val) {
            models.push(plan.model_uid);
            continue;
        }
        if let Some(uid) = key.strip_prefix("/placements/") {
            models.push(uid.to_string());
        }
    }
    models.sort();
    models.dedup();

    let data: Vec<serde_json::Value> = models
        .into_iter()
        .map(|id| json!({"id": id, "object": "model", "owned_by": "nebula"}))
        .collect();

    (StatusCode::OK, Json(json!({"object": "list", "data": data}))).into_response()
}

async fn admin_ui() -> impl IntoResponse {
        let html = r#"<!doctype html>
<html lang="en">
    <head>
        <meta charset="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <title>Nebula Control Console</title>
        <link rel="preconnect" href="https://fonts.googleapis.com" />
        <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin />
        <link href="https://fonts.googleapis.com/css2?family=Space+Grotesk:wght@400;600&display=swap" rel="stylesheet" />
        <style>
            :root {
                --bg: #f6f2ea;
                --ink: #1b1b1b;
                --accent: #0d5c63;
                --accent-2: #f4b183;
                --card: #ffffff;
                --muted: #6b6b6b;
            }
            * { box-sizing: border-box; }
            body {
                margin: 0;
                font-family: "Space Grotesk", sans-serif;
                color: var(--ink);
                background: radial-gradient(circle at top left, #fdf7ef 0%, #f6f2ea 45%, #efe6d9 100%);
            }
            header {
                padding: 28px 32px;
                display: flex;
                align-items: center;
                justify-content: space-between;
            }
            .brand {
                font-size: 22px;
                font-weight: 600;
                letter-spacing: 0.5px;
            }
            .tag {
                color: var(--muted);
                font-size: 13px;
            }
            main {
                padding: 0 32px 40px;
                display: grid;
                gap: 20px;
            }
            .card {
                background: var(--card);
                border-radius: 14px;
                padding: 18px 20px;
                box-shadow: 0 8px 24px rgba(0, 0, 0, 0.08);
            }
            .row {
                display: grid;
                gap: 14px;
                grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
            }
            .input {
                display: flex;
                gap: 10px;
                align-items: center;
            }
            input[type="text"] {
                flex: 1;
                padding: 10px 12px;
                border: 1px solid #e1d8ca;
                border-radius: 10px;
                font-size: 14px;
            }
            button {
                border: none;
                background: var(--accent);
                color: #fff;
                padding: 10px 14px;
                border-radius: 10px;
                cursor: pointer;
                font-weight: 600;
            }
            button.secondary {
                background: #2c2c2c;
            }
            pre {
                white-space: pre-wrap;
                background: #f7f3ec;
                border-radius: 10px;
                padding: 12px;
                font-size: 13px;
            }
            .pill {
                display: inline-block;
                padding: 4px 10px;
                background: var(--accent-2);
                border-radius: 999px;
                font-size: 12px;
                font-weight: 600;
            }
        </style>
    </head>
    <body>
        <header>
            <div>
                <div class="brand">Nebula Control Console</div>
                <div class="tag">Gateway-side Admin UI (MVP)</div>
            </div>
            <span class="pill">/v1/admin</span>
        </header>
        <main>
            <section class="card">
                <div class="input">
                    <input id="token" type="text" placeholder="Bearer token (e.g. devtoken)" />
                    <button id="save">Save Token</button>
                    <button class="secondary" id="refresh">Refresh</button>
                </div>
            </section>
            <section class="row">
                <div class="card">
                    <h3>Who Am I</h3>
                    <pre id="whoami">(empty)</pre>
                </div>
                <div class="card">
                    <h3>Cluster Status</h3>
                    <pre id="cluster">(empty)</pre>
                </div>
                <div class="card">
                    <h3>Model Requests</h3>
                    <pre id="models">(empty)</pre>
                </div>
            </section>
        </main>
        <script>
            const tokenInput = document.getElementById('token');
            const saveBtn = document.getElementById('save');
            const refreshBtn = document.getElementById('refresh');
            const whoamiEl = document.getElementById('whoami');
            const clusterEl = document.getElementById('cluster');
            const modelsEl = document.getElementById('models');

            const stored = localStorage.getItem('nebula_token');
            if (stored) tokenInput.value = stored;

            const authHeaders = () => {
                const token = tokenInput.value.trim();
                return token ? { 'Authorization': `Bearer ${token}` } : {};
            };

            async function fetchJson(path, target) {
                const resp = await fetch(path, { headers: authHeaders() });
                const text = await resp.text();
                target.textContent = `HTTP ${resp.status}\n${text}`;
            }

            async function refreshAll() {
                await fetchJson('/v1/admin/whoami', whoamiEl);
                await fetchJson('/v1/admin/cluster/status', clusterEl);
                await fetchJson('/v1/admin/models/requests', modelsEl);
            }

            saveBtn.addEventListener('click', () => {
                localStorage.setItem('nebula_token', tokenInput.value.trim());
            });

            refreshBtn.addEventListener('click', refreshAll);
            refreshAll();
        </script>
    </body>
</html>"#;

        (StatusCode::OK, html)
}

async fn admin_delete_request(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&st.metrics, &ctx, Role::Operator) {
        return resp;
    }
    let key = format!("/model_requests/{}", id);
    
    let (data, _) = match st.store.get(&key).await {
        Ok(Some(kv)) => kv,
        Ok(None) => return (StatusCode::NOT_FOUND, "request not found").into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("etcd error: {}", e)).into_response(),
    };

    let mut req: ModelRequest = match serde_json::from_slice(&data) {
        Ok(r) => r,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("deserialization error: {}", e)).into_response(),
    };

    req.status = ModelRequestStatus::Unloading;
    let val = serde_json::to_vec(&req).unwrap();
    if let Err(e) = st.store.put(&key, val, None).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, format!("etcd error: {}", e)).into_response();
    }

    (StatusCode::OK, Json(json!({"status": "unloading_triggered"}))).into_response()
}

async fn admin_load_model(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Json(req): Json<ModelLoadRequest>,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&st.metrics, &ctx, Role::Operator) {
        return resp;
    }
    let request_id = Uuid::new_v4().to_string();
    let model_req = ModelRequest {
        id: request_id.clone(),
        request: req,
        status: ModelRequestStatus::Pending,
        created_at_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
    };

    let key = format!("/model_requests/{}", model_req.id);
    let val = serde_json::to_vec(&model_req).unwrap();

    if let Err(e) = st.store.put(&key, val, None).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, format!("etcd error: {}", e)).into_response();
    }

    let body = json!({
        "request_id": request_id,
        "status": "pending"
    });
    (StatusCode::OK, Json(body)).into_response()
}
