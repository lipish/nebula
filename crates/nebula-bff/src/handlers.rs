use axum::{
    extract::Path,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Extension, Json,
};
use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

use crate::auth::{AuthContext, Role};
use crate::state::AppState;
use nebula_common::{
    ClusterStatus, EndpointInfo, ModelLoadRequest, ModelRequest, ModelRequestStatus, NodeStatus,
    PlacementPlan,
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

fn error_response(status: StatusCode, code: &str, message: &str) -> impl IntoResponse {
    let body = ErrorResponse {
        error: ErrorDetail {
            code: code.to_string(),
            message: message.to_string(),
            request_id: format!("req_{}", Uuid::new_v4()),
            details: None,
        },
    };
    (status, Json(body))
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
            )
            .into_response(),
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

async fn load_model_with_request(
    st: AppState,
    req: Option<ModelLoadRequest>,
) -> impl IntoResponse {
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

async fn unload_model_inner(st: AppState, id: String) -> impl IntoResponse {
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
