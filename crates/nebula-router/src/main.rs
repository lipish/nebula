use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use axum::{
    body::Body,
    extract::{State},
    http::{HeaderMap, HeaderName, HeaderValue, Request, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use bytes::Bytes;
use clap::Parser;
use futures_util::StreamExt;
use nebula_common::ExecutionContext;
use nebula_common::PlacementPlan;
use nebula_meta::{EtcdMetaStore, MetaStore};
use reqwest::header::HeaderMap as ReqwestHeaderMap;
use tokio_stream::wrappers::ReceiverStream;
use tracing_subscriber::EnvFilter;

use nebula_common::EndpointInfo;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, default_value = "0.0.0.0:18081")]
    listen_addr: String,

    #[arg(long, default_value = "http://127.0.0.1:2379")]
    etcd_endpoint: String,

    #[arg(long, default_value = "qwen2_5_0_5b")]
    model_uid: String,
}

#[derive(Clone)]
struct AppState {
    model_uid: String,
    router: Arc<nebula_router::Router>,
    http: reqwest::Client,
    plan_version: Arc<AtomicU64>,
}

async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

fn build_execution_context(headers: &HeaderMap) -> ExecutionContext {
    let session_id = headers
        .get("x-session-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    ExecutionContext {
        request_id: format!("req_{}", uuid::Uuid::new_v4()),
        session_id,
        tenant_id: None,
        priority: None,
        deadline_ms: None,
        budget_tokens: None,
    }
}

fn to_reqwest_headers(headers: &HeaderMap) -> ReqwestHeaderMap {
    let mut out = ReqwestHeaderMap::new();
    for (k, v) in headers.iter() {
        // Drop host/content-length; reqwest will set.
        if k.as_str().eq_ignore_ascii_case("host") || k.as_str().eq_ignore_ascii_case("content-length") {
            continue;
        }
        out.insert(k, v.clone());
    }
    out
}

fn copy_response_headers(src: &reqwest::Response, dst: &mut Response) {
    for (k, v) in src.headers().iter() {
        // Skip hop-by-hop headers.
        if k.as_str().eq_ignore_ascii_case("transfer-encoding")
            || k.as_str().eq_ignore_ascii_case("connection")
            || k.as_str().eq_ignore_ascii_case("keep-alive")
            || k.as_str().eq_ignore_ascii_case("proxy-authenticate")
            || k.as_str().eq_ignore_ascii_case("proxy-authorization")
            || k.as_str().eq_ignore_ascii_case("te")
            || k.as_str().eq_ignore_ascii_case("trailer")
            || k.as_str().eq_ignore_ascii_case("upgrade")
        {
            continue;
        }

        if let (Ok(name), Ok(value)) = (
            HeaderName::from_bytes(k.as_str().as_bytes()),
            HeaderValue::from_bytes(v.as_bytes()),
        ) {
            dst.headers_mut().insert(name, value);
        }
    }
}

async fn proxy_chat_completions(
    State(st): State<AppState>,
    headers: HeaderMap,
    req: Request<Body>,
) -> Response {
    let _ctx = build_execution_context(&headers);

    let method = req.method().clone();
    let uri_path = req.uri().path().to_string();
    let uri_query = req.uri().query().map(|q| format!("?{q}")).unwrap_or_default();

    let (method_reqwest, body_bytes, model_uid) = match method {
        axum::http::Method::GET => {
            (reqwest::Method::GET, None, st.model_uid.clone())
        }
        axum::http::Method::POST => {
            let body_bytes = match axum::body::to_bytes(req.into_body(), usize::MAX).await {
                Ok(b) => b,
                Err(_) => return (StatusCode::BAD_REQUEST, "invalid body").into_response(),
            };
            
            // Try to extract model from JSON body
            let model_uid = if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&body_bytes) {
                json.get("model")
                    .and_then(|m| m.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| st.model_uid.clone())
            } else {
                st.model_uid.clone()
            };

            (reqwest::Method::POST, Some(body_bytes), model_uid)
        }
        _ => {
            return (
                StatusCode::METHOD_NOT_ALLOWED,
                "method not allowed",
            )
                .into_response();
        }
    };

    let plan_version = st.plan_version.load(Ordering::Relaxed);
    // Note: In a truly dynamic router, we might want to fetch plan_version per model_uid.
    // For now, if model_uid matches the CLI arg, we use the synced plan_version.
    // Otherwise, we assume version 0 (which might fail) or we need more infrastructure.
    // Let's simplify: if it matches, use synced version. If not, try to route anyway 
    // (the Router trait doesn't strictly require plan_version if we don't use route_with_plan_version).
    
    let ep = if model_uid == st.model_uid && plan_version > 0 {
        st.router.route_with_plan_version(&_ctx, &model_uid, plan_version)
    } else {
        st.router.route(&_ctx, &model_uid)
    };

    let ep = match ep {
        Some(ep) => ep,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                format!("no ready endpoint for model '{}'", model_uid),
            )
                .into_response();
        }
    };

    let base = match ep.base_url.as_deref() {
        Some(s) => s.trim_end_matches('/'),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "endpoint missing base_url",
            )
                .into_response();
        }
    };

    let url = format!("{base}{uri_path}{uri_query}");
    // Re-construct the request with the correct URL
    let mut builder = st.http.request(method_reqwest, url)
        .headers(to_reqwest_headers(&headers));
    
    if let Some(b) = body_bytes {
        builder = builder.body(b);
    }

    let resp = match builder.send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error=%e, "router upstream request failed");
            return (StatusCode::BAD_GATEWAY, "upstream request failed").into_response();
        }
    };

    let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let is_sse = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v: &reqwest::header::HeaderValue| v.to_str().ok())
        .map(|s: &str| s.contains("text/event-stream"))
        .unwrap_or(false);
    let resp_headers = resp.headers().clone();

    if is_sse {
        let mut upstream = resp.bytes_stream();
        let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, std::convert::Infallible>>(64);
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
        for (k, v) in resp_headers.iter() {
            if let (Ok(name), Ok(value)) = (
                HeaderName::from_bytes(k.as_str().as_bytes()),
                HeaderValue::from_bytes(v.as_bytes()),
            ) {
                out.headers_mut().insert(name, value);
            }
        }
        return out;
    }

    let bytes = match resp.bytes().await {
        Ok(b) => b,
        Err(_) => Bytes::new(),
    };

    let mut out = Response::builder()
        .status(status)
        .body(Body::from(bytes))
        .unwrap();
    for (k, v) in resp_headers.iter() {
        if let (Ok(name), Ok(value)) = (
            HeaderName::from_bytes(k.as_str().as_bytes()),
            HeaderValue::from_bytes(v.as_bytes()),
        ) {
            out.headers_mut().insert(name, value);
        }
    }
    out
}

async fn endpoints_sync_loop(store: EtcdMetaStore, router: Arc<nebula_router::Router>) -> anyhow::Result<()> {
    loop {
        let mut snapshot: Vec<EndpointInfo> = Vec::new();
        match store.list_prefix("/endpoints/").await {
            Ok(items) => {
                for (_k, v, _rev) in items {
                    if let Ok(info) = serde_json::from_slice::<EndpointInfo>(&v) {
                        snapshot.push(info);
                    }
                }
                router.replace_all_endpoints(snapshot);
            }
            Err(e) => {
                tracing::warn!(error=%e, "failed to list endpoints, will retry");
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
        }

        let mut stream = match store.watch_prefix("/endpoints/", None).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(error=%e, "failed to watch endpoints, will retry");
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
        };

        while let Some(ev) = stream.next().await {
            if let Some(v) = ev.value {
                if let Ok(info) = serde_json::from_slice::<EndpointInfo>(&v) {
                    router.upsert_endpoint(info);
                }
            } else {
                // best-effort: parse key /endpoints/{model_uid}/{replica_id}
                let parts: Vec<&str> = ev.key.split('/').collect();
                if parts.len() >= 4 {
                    if let Ok(replica_id) = parts[3].parse::<u32>() {
                        router.remove_endpoint(parts[2], replica_id);
                    }
                }
            }
        }

        tracing::warn!("endpoints watch stream ended, reconnecting");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

async fn placement_sync_loop(
    store: EtcdMetaStore,
    model_uid: String,
    plan_version: Arc<AtomicU64>,
) -> anyhow::Result<()> {
    let key = format!("/placements/{model_uid}");
    loop {
        match store.get(&key).await {
            Ok(Some((bytes, _rev))) => {
                if let Ok(plan) = serde_json::from_slice::<PlacementPlan>(&bytes) {
                    if plan.model_uid == model_uid {
                        plan_version.store(plan.version, Ordering::Relaxed);
                    }
                }
            }
            Ok(None) => {
                plan_version.store(0, Ordering::Relaxed);
            }
            Err(e) => {
                tracing::warn!(error=%e, "failed to get placement, will retry");
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
        }

        let mut stream = match store.watch_prefix("/placements/", None).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(error=%e, "failed to watch placements, will retry");
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
        };
        while let Some(ev) = stream.next().await {
            let Some(v) = ev.value else {
                continue;
            };
            let Ok(plan) = serde_json::from_slice::<PlacementPlan>(&v) else {
                continue;
            };
            if plan.model_uid != model_uid {
                continue;
            }
            plan_version.store(plan.version, Ordering::Relaxed);
        }

        tracing::warn!("placements watch stream ended, reconnecting");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args = Args::parse();

    let store = EtcdMetaStore::connect(&vec![args.etcd_endpoint.clone()]).await?;
    let router = nebula_router::Router::new();

    let plan_version = Arc::new(AtomicU64::new(0));

    let router_for_sync = router.clone();
    let store_for_endpoints = store.clone();
    let store_for_placement = store.clone();
    let model_uid_for_placement = args.model_uid.clone();
    let plan_version_for_placement = plan_version.clone();

    tokio::spawn(async move {
        if let Err(e) = endpoints_sync_loop(store_for_endpoints, router_for_sync).await {
            tracing::error!(error=%e, "endpoints sync loop exited");
        }
    });

    tokio::spawn(async move {
        if let Err(e) =
            placement_sync_loop(store_for_placement, model_uid_for_placement, plan_version_for_placement).await
        {
            tracing::error!(error=%e, "placement sync loop exited");
        }
    });

    let http = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(300))
        .build()
        .expect("reqwest client");

    let st = AppState {
        model_uid: args.model_uid,
        router,
        http,
        plan_version,
    };

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/health", get(healthz))
        .route("/v1/chat/completions", post(proxy_chat_completions))
        .route("/v1/completions", post(proxy_chat_completions))
        .route("/v1/embeddings", post(proxy_chat_completions))
        .route("/v1/models", post(proxy_chat_completions).get(proxy_chat_completions))
        .with_state(st);

    let listener = tokio::net::TcpListener::bind(&args.listen_addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
