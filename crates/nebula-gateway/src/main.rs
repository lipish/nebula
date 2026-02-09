mod engine;
mod responses;

use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, HeaderName, HeaderValue, Request, StatusCode},
    response::{sse::Event, IntoResponse, Response, Sse},
    routing::{get, post},
    Json, Router,
};
use bytes::Bytes;
use engine::{EngineClient, OpenAIEngineClient};
use nebula_common::ExecutionContext;
use serde_json::json;
use std::env;
use std::fs;
use std::sync::Arc;
use std::{convert::Infallible, time::Duration};
use tracing_subscriber::EnvFilter;
use tokio::sync::mpsc;
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

    let st = AppState {
        _noop: Arc::new(()),
        engine,
        router_base_url,
        http,
        store: Arc::new(store),
    };

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/health", get(healthz))
        .route("/v1/responses", get(not_implemented).post(create_responses))
        .route("/v1/chat/completions", post(proxy_post))
        .route("/v1/embeddings", post(proxy_post))
        .route("/v1/admin/models/load", post(admin_load_model))
        .route("/v1/admin/models/requests", get(admin_list_requests))
        .route("/v1/admin/models/requests/:id", post(admin_delete_request).delete(admin_delete_request))
        .route("/v1/admin/cluster/status", get(admin_cluster_status))
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
) -> impl IntoResponse {
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
) -> impl IntoResponse {
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

async fn admin_delete_request(
    State(st): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
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
    Json(req): Json<ModelLoadRequest>,
) -> impl IntoResponse {
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
