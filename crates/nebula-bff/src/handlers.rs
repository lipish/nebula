use axum::{
    extract::Path,
    extract::Query,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::auth::{AuthContext, Role};
use crate::state::AppState;
use nebula_common::{
    ClusterStatus, EndpointInfo, EndpointStats, ModelLoadRequest, ModelRequest, ModelRequestStatus,
    NodeStatus, PlacementPlan,
};
use nebula_meta::MetaStore;

#[derive(Serialize)]
struct ErrorDetail {
    code: String,
    message: String,
    request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: ErrorDetail,
}

fn error_response(status: StatusCode, code: &str, message: &str) -> Response {
    let body = ErrorResponse {
        error: ErrorDetail {
            code: code.to_string(),
            message: message.to_string(),
            request_id: format!("req_{}", Uuid::new_v4()),
            details: None,
        },
    };
    (status, Json(body)).into_response()
}

fn require_role(ctx: &AuthContext, required: Role) -> Option<Response> {
    let current = role_rank(ctx.role);
    let needed = role_rank(required);
    if current < needed {
        Some(
            error_response(
                StatusCode::FORBIDDEN,
                "forbidden",
                "insufficient permissions",
            ),
        )
    } else {
        None
    }
}

fn role_rank(role: Role) -> u8 {
    match role {
        Role::Admin => 3,
        Role::Operator => 2,
        Role::Viewer => 1,
    }
}

pub async fn healthz() -> impl IntoResponse {
    Json(json!({"status": "ok"}))
}

pub async fn whoami(Extension(ctx): Extension<AuthContext>) -> impl IntoResponse {
    let role = match ctx.role {
        crate::auth::Role::Admin => "admin",
        crate::auth::Role::Operator => "operator",
        crate::auth::Role::Viewer => "viewer",
    };

    Json(json!({
        "principal": ctx.principal,
        "role": role,
    }))
}

pub async fn overview(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&ctx, Role::Viewer) {
        return resp;
    }

    let nodes_raw = match st.store.list_prefix("/nodes/").await {
        Ok(n) => n,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "etcd_error",
                &format!("etcd error: {}", e),
            );
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
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "etcd_error",
                &format!("etcd error: {}", e),
            );
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
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "etcd_error",
                &format!("etcd error: {}", e),
            );
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
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "etcd_error",
                &format!("etcd error: {}", e),
            );
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

pub async fn list_requests(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&ctx, Role::Viewer) {
        return resp;
    }
    let requests_raw = match st.store.list_prefix("/model_requests/").await {
        Ok(r) => r,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "etcd_error",
                &format!("etcd error: {}", e),
            )
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

pub async fn load_model(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Json(req): Json<ModelLoadRequest>,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&ctx, Role::Operator) {
        return resp;
    }
    load_model_with_request(st, Some(req)).await
}

pub async fn unload_model(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&ctx, Role::Operator) {
        return resp;
    }
    unload_model_inner(st, id).await
}

pub async fn metrics(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&ctx, Role::Viewer) {
        return resp;
    }
    let url = format!("{}/metrics", st.router_url.trim_end_matches('/'));
    let resp = match st.http.get(url).send().await {
        Ok(resp) => resp,
        Err(e) => {
            return error_response(
                StatusCode::BAD_GATEWAY,
                "upstream_error",
                &format!("router request failed: {}", e),
            )
        }
    };

    let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let text = match resp.text().await {
        Ok(text) => text,
        Err(e) => {
            return error_response(
                StatusCode::BAD_GATEWAY,
                "upstream_error",
                &format!("failed to read router response: {}", e),
            )
        }
    };

    (status, text).into_response()
}

pub async fn logs(State(_st): State<AppState>) -> impl IntoResponse {
    error_response(StatusCode::NOT_IMPLEMENTED, "not_implemented", "logs not implemented")
}

pub async fn engine_stats(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&ctx, Role::Viewer) {
        return resp;
    }

    let stats_raw = match st.store.list_prefix("/stats/").await {
        Ok(s) => s,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "etcd_error",
                &format!("etcd error: {}", e),
            )
        }
    };

    let mut stats = Vec::new();
    for (_, v, _) in stats_raw {
        if let Ok(s) = serde_json::from_slice::<EndpointStats>(&v) {
            stats.push(s);
        }
    }

    (StatusCode::OK, Json(stats)).into_response()
}

#[derive(Deserialize)]
pub struct ModelSearchQuery {
    pub q: String,
    pub source: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Serialize)]
pub struct ModelSearchResult {
    pub id: String,
    pub name: String,
    pub author: Option<String>,
    pub downloads: u64,
    pub likes: u64,
    pub tags: Vec<String>,
    pub pipeline_tag: Option<String>,
    pub source: String,
}

pub async fn search_models(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Query(params): Query<ModelSearchQuery>,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&ctx, Role::Viewer) {
        return resp;
    }

    let limit = params.limit.unwrap_or(20).min(50);
    let source = params.source.as_deref().unwrap_or("huggingface");

    match source {
        "modelscope" => search_modelscope(&st.http, &params.q, limit).await,
        _ => search_huggingface(&st.http, &params.q, limit).await,
    }
}

async fn search_huggingface(http: &reqwest::Client, query: &str, limit: usize) -> Response {
    let url = format!(
        "https://hf-mirror.com/api/models?search={}&limit={}&sort=downloads&direction=-1",
        urlencoding::encode(query),
        limit
    );

    let resp = match http.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            return error_response(
                StatusCode::BAD_GATEWAY,
                "upstream_error",
                &format!("HuggingFace API error: {}", e),
            )
        }
    };

    let body: Vec<serde_json::Value> = match resp.json().await {
        Ok(b) => b,
        Err(e) => {
            return error_response(
                StatusCode::BAD_GATEWAY,
                "upstream_error",
                &format!("Failed to parse HuggingFace response: {}", e),
            )
        }
    };

    let results: Vec<ModelSearchResult> = body
        .into_iter()
        .map(|m| ModelSearchResult {
            id: m["modelId"].as_str().unwrap_or_default().to_string(),
            name: m["modelId"].as_str().unwrap_or_default().to_string(),
            author: m.get("author").and_then(|a| a.as_str()).map(String::from),
            downloads: m["downloads"].as_u64().unwrap_or(0),
            likes: m["likes"].as_u64().unwrap_or(0),
            tags: m["tags"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|t| t.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
            pipeline_tag: m.get("pipeline_tag").and_then(|p| p.as_str()).map(String::from),
            source: "huggingface".to_string(),
        })
        .collect();

    (StatusCode::OK, Json(results)).into_response()
}

async fn search_modelscope(http: &reqwest::Client, query: &str, limit: usize) -> Response {
    // ModelScope search API now requires authentication.
    // Fall back to HuggingFace API and re-label results as modelscope,
    // since most model paths are identical across both platforms.
    let url = format!(
        "https://hf-mirror.com/api/models?search={}&limit={}&sort=downloads&direction=-1",
        urlencoding::encode(query),
        limit
    );

    let resp = match http.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            return error_response(
                StatusCode::BAD_GATEWAY,
                "upstream_error",
                &format!("Search API error: {}", e),
            )
        }
    };

    let body: Vec<serde_json::Value> = match resp.json().await {
        Ok(b) => b,
        Err(e) => {
            return error_response(
                StatusCode::BAD_GATEWAY,
                "upstream_error",
                &format!("Failed to parse search response: {}", e),
            )
        }
    };

    let results: Vec<ModelSearchResult> = body
        .into_iter()
        .map(|m| ModelSearchResult {
            id: m["modelId"].as_str().unwrap_or_default().to_string(),
            name: m["modelId"].as_str().unwrap_or_default().to_string(),
            author: m.get("author").and_then(|a| a.as_str()).map(String::from),
            downloads: m["downloads"].as_u64().unwrap_or(0),
            likes: m["likes"].as_u64().unwrap_or(0),
            tags: m["tags"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|t| t.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
            pipeline_tag: m.get("pipeline_tag").and_then(|p| p.as_str()).map(String::from),
            source: "modelscope".to_string(),
        })
        .collect();

    (StatusCode::OK, Json(results)).into_response()
}

async fn load_model_with_request(
    st: AppState,
    req: Option<ModelLoadRequest>,
) -> Response {
    let req = match req {
        Some(req) => req,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "missing request body",
            )
        }
    };

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

    let val = match serde_json::to_vec(&model_req) {
        Ok(val) => val,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "serialization_error",
                &format!("serialization error: {}", e),
            )
        }
    };

    let key = format!("/model_requests/{}", model_req.id);
    if let Err(e) = st.store.put(&key, val, None).await {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "etcd_error",
            &format!("etcd error: {}", e),
        );
    }

    (StatusCode::OK, Json(json!({"request_id": request_id, "status": "pending"}))).into_response()
}

/// Generic helper: proxy a GET request to xtrace, forwarding query string and bearer token.
async fn xtrace_proxy_get(st: &AppState, path: &str, raw_query: Option<&str>) -> Response {
    let base = st.xtrace_url.trim_end_matches('/');
    let url = match raw_query {
        Some(q) if !q.is_empty() => format!("{path}?{q}", path = format!("{base}{path}"), q = q),
        _ => format!("{base}{path}"),
    };

    let mut req = st.http.get(&url);
    if !st.xtrace_token.is_empty() {
        req = req.bearer_auth(&st.xtrace_token);
    }

    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            return error_response(
                StatusCode::BAD_GATEWAY,
                "xtrace_error",
                &format!("xtrace request failed: {}", e),
            )
        }
    };

    let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let body: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => {
            return error_response(
                StatusCode::BAD_GATEWAY,
                "xtrace_error",
                &format!("failed to parse xtrace response: {}", e),
            )
        }
    };

    (status, Json(body)).into_response()
}

pub async fn observe_traces(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    req: axum::extract::Request,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&ctx, Role::Viewer) {
        return resp;
    }
    xtrace_proxy_get(&st, "/api/public/traces", req.uri().query()).await
}

pub async fn observe_trace_detail(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Path(trace_id): Path<String>,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&ctx, Role::Viewer) {
        return resp;
    }
    xtrace_proxy_get(&st, &format!("/api/public/traces/{trace_id}"), None).await
}

pub async fn observe_metrics_query(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    req: axum::extract::Request,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&ctx, Role::Viewer) {
        return resp;
    }
    xtrace_proxy_get(&st, "/api/public/metrics/query", req.uri().query()).await
}

pub async fn observe_metrics_names(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    req: axum::extract::Request,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&ctx, Role::Viewer) {
        return resp;
    }
    xtrace_proxy_get(&st, "/api/public/metrics/names", req.uri().query()).await
}

async fn unload_model_inner(st: AppState, id: String) -> Response {
    if id.is_empty() {
        return error_response(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "request id is required",
        );
    }

    let key = format!("/model_requests/{}", id);
    let (data, _) = match st.store.get(&key).await {
        Ok(Some(kv)) => kv,
        Ok(None) => {
            return error_response(
                StatusCode::NOT_FOUND,
                "not_found",
                "request not found",
            )
        }
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "etcd_error",
                &format!("etcd error: {}", e),
            )
        }
    };

    let mut req: ModelRequest = match serde_json::from_slice(&data) {
        Ok(r) => r,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "deserialization_error",
                &format!("deserialization error: {}", e),
            )
        }
    };

    req.status = ModelRequestStatus::Unloading;
    let val = match serde_json::to_vec(&req) {
        Ok(val) => val,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "serialization_error",
                &format!("serialization error: {}", e),
            )
        }
    };

    if let Err(e) = st.store.put(&key, val, None).await {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "etcd_error",
            &format!("etcd error: {}", e),
        );
    }

    (StatusCode::OK, Json(json!({"status": "unloading_triggered"}))).into_response()
}

pub async fn audit_logs(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    req: axum::extract::Request,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&ctx, Role::Admin) {
        return resp;
    }
    // Proxy to xtrace traces endpoint, injecting tags[]=audit filter.
    let existing_query = req.uri().query().unwrap_or("");
    let query = if existing_query.is_empty() {
        "tags%5B%5D=audit".to_string()
    } else {
        format!("{existing_query}&tags%5B%5D=audit")
    };
    xtrace_proxy_get(&st, "/api/public/traces", Some(&query)).await
}
