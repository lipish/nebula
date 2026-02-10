use std::convert::Infallible;

use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, HeaderName, HeaderValue, Request, StatusCode},
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use futures_util::StreamExt;
use reqwest::header::HeaderMap as ReqwestHeaderMap;
use tokio_stream::wrappers::ReceiverStream;

use nebula_common::ExecutionContext;

use crate::state::AppState;

pub async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

pub fn build_execution_context(headers: &HeaderMap) -> ExecutionContext {
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
        if k.as_str().eq_ignore_ascii_case("host")
            || k.as_str().eq_ignore_ascii_case("content-length")
        {
            continue;
        }
        out.insert(k, v.clone());
    }
    out
}

fn copy_response_headers(src: &ReqwestHeaderMap, dst: &mut Response) {
    for (k, v) in src.iter() {
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

pub async fn proxy_chat_completions(
    State(st): State<AppState>,
    headers: HeaderMap,
    req: Request<Body>,
) -> Response {
    let _ctx = build_execution_context(&headers);

    let method = req.method().clone();
    let uri_path = req.uri().path().to_string();
    let uri_query = req
        .uri()
        .query()
        .map(|q| format!("?{q}"))
        .unwrap_or_default();

    let (method_reqwest, body_bytes, model_uid) = match method {
        axum::http::Method::GET => (reqwest::Method::GET, None, st.model_uid.clone()),
        axum::http::Method::POST => {
            let body_bytes = match axum::body::to_bytes(req.into_body(), usize::MAX).await {
                Ok(b) => b,
                Err(_) => return (StatusCode::BAD_REQUEST, "invalid body").into_response(),
            };

            let model_uid =
                if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&body_bytes) {
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
            return (StatusCode::METHOD_NOT_ALLOWED, "method not allowed").into_response();
        }
    };

    let plan_version = st.plan_version.load(std::sync::atomic::Ordering::Relaxed);

    let ep = if model_uid == st.model_uid && plan_version > 0 {
        st.router
            .route_with_plan_version(&_ctx, &model_uid, plan_version)
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
            return (StatusCode::SERVICE_UNAVAILABLE, "endpoint missing base_url").into_response();
        }
    };

    let url = format!("{base}{uri_path}{uri_query}");
    let mut builder = st
        .http
        .request(method_reqwest, url)
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
    let resp_headers = resp.headers().clone();
    let is_sse = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v: &reqwest::header::HeaderValue| v.to_str().ok())
        .map(|s: &str| s.contains("text/event-stream"))
        .unwrap_or(false);

    if is_sse {
        let mut upstream = resp.bytes_stream();
        let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, Infallible>>(64);
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
        copy_response_headers(&resp_headers, &mut out);
        return out;
    }

    let bytes = match resp.bytes().await {
        Ok(b) => b,
        Err(_) => Bytes::new(),
    };

    let mut out = Response::builder()
        .status(status)
        .body(Body::from(bytes))
        .unwrap_or_else(|_| Response::new(Body::empty()));
    copy_response_headers(&resp_headers, &mut out);
    out
}
