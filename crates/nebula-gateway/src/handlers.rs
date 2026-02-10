use std::convert::Infallible;
use std::time::Duration;

use axum::{
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, HeaderName, HeaderValue, Request, StatusCode},
    response::{sse::Event, IntoResponse, Response, Sse},
    Extension, Json,
};
use bytes::Bytes;
use serde_json::json;
use tokio::fs;
use tokio::sync::mpsc;
use tokio_stream::{wrappers::ReceiverStream, StreamExt};
use uuid::Uuid;

use nebula_common::{
    ClusterStatus, EndpointInfo, ExecutionContext, ModelLoadRequest, ModelRequest,
    ModelRequestStatus, NodeStatus, PlacementPlan,
};
use nebula_meta::MetaStore;

use crate::auth::{require_role, AuthContext, Role};
use crate::responses::{
    build_non_stream_json, build_response, CreateResponseRequest, ResponseStreamBuilder,
};
use crate::state::AppState;

#[derive(Debug, serde::Deserialize)]
pub(crate) struct LogsQuery {
    lines: Option<usize>,
}

pub async fn create_responses(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateResponseRequest>,
) -> Response {
    let _ctx = build_execution_context(&headers);

    let input_text = req.extract_input_text();

    if req.stream.unwrap_or(false) {
        let (tx, rx) = mpsc::channel::<Result<Event, Infallible>>(256);
        let engine = st.engine.clone();

        let builder = ResponseStreamBuilder::new(&req);

        tokio::spawn(async move {
            let mut stream = engine.stream_text(input_text);
            let mut builder = builder;

            let _ = tx
                .send(Ok(
                    Event::default().data(builder.created_event().to_string())
                ))
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
        let mut stream = st.engine.stream_text(input_text);
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

pub fn build_execution_context(headers: &HeaderMap) -> ExecutionContext {
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

pub async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

pub async fn not_implemented(State(_st): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
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

fn to_reqwest_headers(headers: &HeaderMap) -> reqwest::header::HeaderMap {
    let mut out = reqwest::header::HeaderMap::new();
    for (k, v) in headers.iter() {
        if k.as_str().eq_ignore_ascii_case("host")
            || k.as_str().eq_ignore_ascii_case("content-length")
        {
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

pub async fn proxy_post(
    State(st): State<AppState>,
    headers: HeaderMap,
    req: Request<Body>,
) -> Response {
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
            .unwrap_or_else(|_| Response::new(Body::empty()));
        append_headers(&resp_headers, &mut out);
        return out;
    }

    let bytes = match resp.bytes().await {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::warn!(error=%e, "failed to read upstream response body");
            Bytes::new()
        }
    };
    let mut out = Response::builder()
        .status(status)
        .body(Body::from(bytes))
        .unwrap_or_else(|_| Response::new(Body::empty()));
    append_headers(&resp_headers, &mut out);
    out
}

pub async fn admin_cluster_status(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&st.metrics, &ctx, Role::Viewer) {
        return resp;
    }
    let nodes_raw = match st.store.list_prefix("/nodes/").await {
        Ok(n) => n,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("etcd error: {}", e),
            )
                .into_response();
        }
    };
    let mut nodes = Vec::new();
    for (_, v, _) in nodes_raw {
        if let Ok(n) = serde_json::from_slice::<NodeStatus>(&v) {
            nodes.push(n);
        }
    }

    let endpoints_raw = match st.store.list_prefix("/endpoints/").await {
        Ok(e) => e,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("etcd error: {}", e),
            )
                .into_response();
        }
    };
    let mut endpoints = Vec::new();
    for (_, v, _) in endpoints_raw {
        if let Ok(ep) = serde_json::from_slice::<EndpointInfo>(&v) {
            endpoints.push(ep);
        }
    }

    let placements_raw = match st.store.list_prefix("/placements/").await {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("etcd error: {}", e),
            )
                .into_response();
        }
    };
    let mut placements = Vec::new();
    for (_, v, _) in placements_raw {
        if let Ok(p) = serde_json::from_slice::<PlacementPlan>(&v) {
            placements.push(p);
        }
    }

    let requests_raw = match st.store.list_prefix("/model_requests/").await {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("etcd error: {}", e),
            )
                .into_response();
        }
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

pub async fn admin_list_requests(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&st.metrics, &ctx, Role::Viewer) {
        return resp;
    }
    let requests_raw = match st.store.list_prefix("/model_requests/").await {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("etcd error: {}", e),
            )
                .into_response();
        }
    };
    let mut model_requests = Vec::new();
    for (_, v, _) in requests_raw {
        if let Ok(r) = serde_json::from_slice::<ModelRequest>(&v) {
            model_requests.push(r);
        }
    }
    (StatusCode::OK, Json(model_requests)).into_response()
}

pub async fn admin_whoami(Extension(ctx): Extension<AuthContext>) -> impl IntoResponse {
    let role = match ctx.role {
        Role::Admin => "admin",
        Role::Operator => "operator",
        Role::Viewer => "viewer",
    };
    (
        StatusCode::OK,
        Json(json!({
            "principal": ctx.principal,
            "role": role,
        })),
    )
        .into_response()
}

pub async fn admin_logs(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    axum::extract::Query(query): axum::extract::Query<LogsQuery>,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&st.metrics, &ctx, Role::Viewer) {
        return resp;
    }

    let lines = query.lines.unwrap_or(200).min(2000);
    let content = match fs::read_to_string(&st.log_path).await {
        Ok(content) => content,
        Err(e) => {
            tracing::warn!(error=%e, path=%st.log_path, "failed to read log file");
            String::new()
        }
    };
    let mut out_lines: Vec<&str> = content.lines().rev().take(lines).collect();
    out_lines.reverse();
    (StatusCode::OK, out_lines.join("\n")).into_response()
}

pub async fn list_models(State(st): State<AppState>) -> impl IntoResponse {
    let placements_raw = match st.store.list_prefix("/placements/").await {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("etcd error: {}", e),
            )
                .into_response();
        }
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

    (
        StatusCode::OK,
        Json(json!({"object": "list", "data": data})),
    )
        .into_response()
}

pub async fn admin_ui() -> impl IntoResponse {
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

pub async fn admin_delete_request(
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
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("etcd error: {}", e),
            )
                .into_response();
        }
    };

    let mut req: ModelRequest = match serde_json::from_slice(&data) {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("deserialization error: {}", e),
            )
                .into_response();
        }
    };

    req.status = ModelRequestStatus::Unloading;
    let val = match serde_json::to_vec(&req) {
        Ok(val) => val,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("serialization error: {}", e),
            )
                .into_response();
        }
    };
    if let Err(e) = st.store.put(&key, val, None).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("etcd error: {}", e),
        )
            .into_response();
    }

    (
        StatusCode::OK,
        Json(json!({"status": "unloading_triggered"})),
    )
        .into_response()
}

pub async fn admin_load_model(
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
    let val = match serde_json::to_vec(&model_req) {
        Ok(val) => val,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("serialization error: {}", e),
            )
                .into_response();
        }
    };

    if let Err(e) = st.store.put(&key, val, None).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("etcd error: {}", e),
        )
            .into_response();
    }

    let body = json!({
        "request_id": request_id,
        "status": "pending"
    });
    (StatusCode::OK, Json(body)).into_response()
}
