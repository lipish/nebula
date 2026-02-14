use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

use crate::auth::{require_role, AuthContext, Role};
use crate::state::AppState;
use nebula_common::{
    DesiredState, DiskAlert, DownloadPhase, DownloadProgress, EndpointInfo, EndpointStats,
    ModelCacheEntry, ModelConfig, ModelDeployment, ModelRequest, ModelRequestStatus, ModelSource,
    ModelSpec, ModelTemplate, NodeDiskStatus, PlacementPlan, TemplateCategory, TemplateSource,
};
use nebula_meta::MetaStore;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

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

/// Sanitise a model name into a valid model_uid.
fn generate_model_uid(model_name: &str) -> String {
    let uid: String = model_name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    // Trim leading/trailing dashes, collapse consecutive dashes
    let uid = uid.trim_matches('-').to_string();
    let mut result = String::new();
    let mut prev_dash = false;
    for c in uid.chars() {
        if c == '-' {
            if !prev_dash {
                result.push(c);
            }
            prev_dash = true;
        } else {
            result.push(c);
            prev_dash = false;
        }
    }
    if result.len() > 63 {
        result.truncate(63);
    }
    // Trim trailing dash after truncation
    result.trim_end_matches('-').to_string()
}

fn is_valid_model_uid(uid: &str) -> bool {
    if uid.is_empty() || uid.len() > 63 {
        return false;
    }
    let mut chars = uid.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() || c.is_ascii_digit() => {}
        _ => return false,
    }
    for c in chars {
        if !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
            return false;
        }
    }
    true
}

fn model_name_matches(cache_name: &str, spec_name: &str) -> bool {
    if cache_name == spec_name {
        return true;
    }

    let cache_lc = cache_name.to_lowercase();
    let spec_lc = spec_name.to_lowercase();
    if cache_lc == spec_lc {
        return true;
    }

    let cache_tail = cache_lc.rsplit('/').next().unwrap_or_default();
    let spec_tail = spec_lc.rsplit('/').next().unwrap_or_default();

    cache_tail == spec_tail
        || cache_tail == spec_lc
        || spec_tail == cache_lc
        || spec_lc.starts_with(&(cache_lc.clone() + "/"))
        || cache_lc.starts_with(&(spec_lc + "/"))
}

// ---------------------------------------------------------------------------
// Aggregated State
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AggregatedModelState {
    Stopped,
    Downloading,
    Starting,
    Running,
    Degraded,
    Failed,
    Stopping,
}

/// Threshold (ms) after which a model with no endpoints is considered Failed.
const FAILED_THRESHOLD_MS: u64 = 5 * 60 * 1000; // 5 minutes

fn compute_aggregated_state(
    deployment: Option<&ModelDeployment>,
    placement: Option<&PlacementPlan>,
    endpoints: &[EndpointInfo],
    download_progress: &[DownloadProgress],
    spec_created_at_ms: u64,
) -> AggregatedModelState {
    let dep = match deployment {
        None => return AggregatedModelState::Stopped,
        Some(d) => d,
    };

    if dep.desired_state == DesiredState::Stopped {
        // If there are still endpoints draining, show Stopping
        if !endpoints.is_empty() {
            return AggregatedModelState::Stopping;
        }
        return AggregatedModelState::Stopped;
    }

    // desired_state == Running
    if placement.is_none() {
        return AggregatedModelState::Starting;
    }

    // Check for active downloads
    let has_active_download = download_progress
        .iter()
        .any(|dp| dp.phase != DownloadPhase::Complete && dp.phase != DownloadPhase::Failed);
    if has_active_download {
        return AggregatedModelState::Downloading;
    }

    let ready_count = endpoints
        .iter()
        .filter(|ep| ep.status == nebula_common::EndpointStatus::Ready)
        .count();
    let total_count = endpoints.len();

    if total_count > 0 && ready_count == total_count {
        return AggregatedModelState::Running;
    }
    if ready_count > 0 {
        return AggregatedModelState::Degraded;
    }

    // No ready endpoints. Measure elapsed from most recent desired-state update,
    // not from model creation time, so restarts don't look immediately failed.
    let base_ts = dep.updated_at_ms.max(spec_created_at_ms);
    let elapsed = now_ms().saturating_sub(base_ts);
    if total_count == 0 && elapsed > FAILED_THRESHOLD_MS {
        return AggregatedModelState::Failed;
    }

    AggregatedModelState::Starting
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct ReplicaCount {
    pub desired: u32,
    pub ready: u32,
    pub unhealthy: u32,
}

#[derive(Serialize)]
pub struct ModelView {
    pub model_uid: String,
    pub model_name: String,
    pub engine_type: Option<String>,
    pub state: AggregatedModelState,
    pub replicas: ReplicaCount,
    pub endpoints: Vec<EndpointInfo>,
    pub labels: HashMap<String, String>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

#[derive(Serialize)]
pub struct DownloadProgressView {
    pub replicas: Vec<DownloadProgress>,
}

#[derive(Serialize)]
pub struct CacheStatusView {
    pub cached_on_nodes: Vec<String>,
    pub total_size_bytes: u64,
}

#[derive(Serialize)]
pub struct ModelDetailView {
    pub model_uid: String,
    pub model_name: String,
    pub engine_type: Option<String>,
    pub state: AggregatedModelState,
    pub replicas: ReplicaCount,
    pub labels: HashMap<String, String>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub spec: ModelSpec,
    pub deployment: Option<ModelDeployment>,
    pub placement: Option<PlacementPlan>,
    pub endpoints: Vec<EndpointInfo>,
    pub stats: Vec<EndpointStats>,
    pub download_progress: Option<DownloadProgressView>,
    pub cache_status: Option<CacheStatusView>,
}

#[derive(Serialize)]
pub struct CacheSummary {
    pub total_cached_models: usize,
    pub total_cache_size_bytes: u64,
    pub nodes: Vec<NodeDiskStatus>,
    pub caches: Vec<CacheEntryView>,
}

#[derive(Serialize)]
pub struct CacheEntryView {
    #[serde(flatten)]
    pub entry: ModelCacheEntry,
    #[serde(default)]
    pub matched_model_uids: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct ModelGcRequest {
    model_uid: String,
    model_name: String,
    model_path: Option<String>,
    requested_at_ms: u64,
}


// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct CreateModelRequest {
    pub model_name: String,
    pub model_uid: Option<String>,
    pub model_source: Option<ModelSource>,
    pub model_path: Option<String>,
    pub engine_type: Option<String>,
    pub docker_image: Option<String>,
    pub config: Option<ModelConfig>,
    pub labels: Option<HashMap<String, String>>,
    pub auto_start: Option<bool>,
    pub replicas: Option<u32>,
    pub node_id: Option<String>,
    pub gpu_indices: Option<Vec<u32>>,
}

#[derive(Deserialize)]
pub struct UpdateModelRequest {
    pub model_name: Option<String>,
    pub model_source: Option<ModelSource>,
    pub model_path: Option<String>,
    pub engine_type: Option<String>,
    pub docker_image: Option<String>,
    pub config: Option<ModelConfig>,
    pub labels: Option<HashMap<String, String>>,
}

#[derive(Deserialize)]
pub struct StartModelRequest {
    pub replicas: Option<u32>,
    pub config_overrides: Option<ModelConfig>,
    pub node_id: Option<String>,
    pub gpu_indices: Option<Vec<u32>>,
}

#[derive(Deserialize)]
pub struct ScaleModelRequest {
    pub replicas: u32,
}

#[derive(Deserialize)]
pub struct DeployTemplateRequest {
    pub model_uid: Option<String>,
    pub replicas: Option<u32>,
    pub config_overrides: Option<ModelConfig>,
    pub node_id: Option<String>,
    pub gpu_indices: Option<Vec<u32>>,
}

#[derive(Deserialize)]
pub struct SaveAsTemplateRequest {
    pub template_id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub category: Option<TemplateCategory>,
}

#[derive(Deserialize)]
pub struct CreateTemplateRequest {
    pub template_id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub category: Option<TemplateCategory>,
    pub model_name: String,
    pub model_source: Option<ModelSource>,
    pub engine_type: Option<String>,
    pub docker_image: Option<String>,
    pub config: Option<ModelConfig>,
    pub default_replicas: Option<u32>,
    pub labels: Option<HashMap<String, String>>,
}

#[derive(Deserialize)]
pub struct UpdateTemplateRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub category: Option<TemplateCategory>,
    pub model_name: Option<String>,
    pub model_source: Option<ModelSource>,
    pub engine_type: Option<String>,
    pub docker_image: Option<String>,
    pub config: Option<ModelConfig>,
    pub default_replicas: Option<u32>,
    pub labels: Option<HashMap<String, String>>,
}

#[derive(Deserialize)]
pub struct ListModelsQuery {
    pub state: Option<String>,
    pub label: Option<String>,
}

// ---------------------------------------------------------------------------
// Helper: build a ModelView from a spec by fetching related data
// ---------------------------------------------------------------------------

async fn build_model_view(st: &AppState, spec: &ModelSpec) -> ModelView {
    let uid = &spec.model_uid;

    let deployment = st
        .store
        .get(&format!("/deployments/{uid}"))
        .await
        .ok()
        .flatten()
        .and_then(|(data, _)| serde_json::from_slice::<ModelDeployment>(&data).ok());

    let placement = st
        .store
        .get(&format!("/placements/{uid}"))
        .await
        .ok()
        .flatten()
        .and_then(|(data, _)| serde_json::from_slice::<PlacementPlan>(&data).ok());

    let endpoints: Vec<EndpointInfo> = st
        .store
        .list_prefix(&format!("/endpoints/{uid}/"))
        .await
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(_, v, _)| serde_json::from_slice(&v).ok())
        .collect();

    let download_progress: Vec<DownloadProgress> = st
        .store
        .list_prefix(&format!("/download_progress/{uid}/"))
        .await
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(_, v, _)| serde_json::from_slice(&v).ok())
        .collect();

    let state = compute_aggregated_state(
        deployment.as_ref(),
        placement.as_ref(),
        &endpoints,
        &download_progress,
        spec.created_at_ms,
    );

    let desired = deployment.as_ref().map(|d| d.replicas).unwrap_or(0);
    let ready = endpoints
        .iter()
        .filter(|ep| ep.status == nebula_common::EndpointStatus::Ready)
        .count() as u32;
    let unhealthy = endpoints
        .iter()
        .filter(|ep| ep.status == nebula_common::EndpointStatus::Unhealthy)
        .count() as u32;

    ModelView {
        model_uid: spec.model_uid.clone(),
        model_name: spec.model_name.clone(),
        engine_type: spec.engine_type.clone(),
        state,
        replicas: ReplicaCount {
            desired,
            ready,
            unhealthy,
        },
        endpoints,
        labels: spec.labels.clone(),
        created_at_ms: spec.created_at_ms,
        updated_at_ms: spec.updated_at_ms,
    }
}

// ===========================================================================
// Model CRUD
// ===========================================================================

pub async fn create_model(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Json(req): Json<CreateModelRequest>,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&ctx, Role::Operator) {
        return resp;
    }

    let uid = match req.model_uid {
        Some(ref uid) => {
            if !is_valid_model_uid(uid) {
                return error_response(
                    StatusCode::BAD_REQUEST,
                    "invalid_model_uid",
                    "model_uid must match [a-z0-9][a-z0-9-]* and be at most 63 chars",
                );
            }
            uid.clone()
        }
        None => generate_model_uid(&req.model_name),
    };

    // Check for conflict
    if let Ok(Some(_)) = st.store.get(&format!("/models/{uid}/spec")).await {
        return error_response(
            StatusCode::CONFLICT,
            "model_exists",
            &format!("model with uid '{uid}' already exists"),
        );
    }

    let now = now_ms();
    let spec = ModelSpec {
        model_uid: uid.clone(),
        model_name: req.model_name,
        model_source: req.model_source.unwrap_or(ModelSource::HuggingFace),
        model_path: req.model_path,
        engine_type: req.engine_type,
        docker_image: req.docker_image,
        config: req.config,
        labels: req.labels.unwrap_or_default(),
        created_at_ms: now,
        updated_at_ms: now,
        created_by: Some(ctx.principal.clone()),
    };

    let val = match serde_json::to_vec(&spec) {
        Ok(v) => v,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "serialization_error",
                &format!("serialization error: {e}"),
            )
        }
    };

    if let Err(e) = st.store.put(&format!("/models/{uid}/spec"), val, None).await {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "etcd_error",
            &format!("etcd error: {e}"),
        );
    }

    // auto_start: create deployment
    if req.auto_start.unwrap_or(false) {
        let deployment = ModelDeployment {
            model_uid: uid.clone(),
            desired_state: DesiredState::Running,
            replicas: req.replicas.unwrap_or(1),
            min_replicas: None,
            max_replicas: None,
            node_affinity: req.node_id,
            gpu_affinity: req.gpu_indices,
            config_overrides: None,
            version: 1,
            updated_at_ms: now,
        };
        if let Ok(dv) = serde_json::to_vec(&deployment) {
            let _ = st.store.put(&format!("/deployments/{uid}"), dv, None).await;
        }
    }

    (StatusCode::CREATED, Json(json!(spec))).into_response()
}

pub async fn list_models(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Query(params): Query<ListModelsQuery>,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&ctx, Role::Viewer) {
        return resp;
    }

    let specs_raw = match st.store.list_prefix("/models/").await {
        Ok(v) => v,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "etcd_error",
                &format!("etcd error: {e}"),
            )
        }
    };

    let specs: Vec<ModelSpec> = specs_raw
        .into_iter()
        .filter(|(k, _, _)| k.ends_with("/spec"))
        .filter_map(|(_, v, _)| serde_json::from_slice(&v).ok())
        .collect();

    let mut views = Vec::with_capacity(specs.len());
    for spec in &specs {
        let view = build_model_view(&st, spec).await;

        // Filter by state
        if let Some(ref state_filter) = params.state {
            let state_str = serde_json::to_string(&view.state).unwrap_or_default();
            let state_str = state_str.trim_matches('"');
            if state_str != state_filter {
                continue;
            }
        }

        // Filter by label
        if let Some(ref label_filter) = params.label {
            if let Some((k, v)) = label_filter.split_once('=') {
                if spec.labels.get(k) != Some(&v.to_string()) {
                    continue;
                }
            }
        }

        views.push(view);
    }

    (StatusCode::OK, Json(views)).into_response()
}

pub async fn get_model(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Path(model_uid): Path<String>,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&ctx, Role::Viewer) {
        return resp;
    }

    let spec: ModelSpec = match st.store.get(&format!("/models/{model_uid}/spec")).await {
        Ok(Some((data, _))) => match serde_json::from_slice(&data) {
            Ok(s) => s,
            Err(e) => {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "deserialization_error",
                    &format!("deserialization error: {e}"),
                )
            }
        },
        Ok(None) => {
            return error_response(StatusCode::NOT_FOUND, "not_found", "model not found")
        }
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "etcd_error",
                &format!("etcd error: {e}"),
            )
        }
    };

    let deployment = st
        .store
        .get(&format!("/deployments/{model_uid}"))
        .await
        .ok()
        .flatten()
        .and_then(|(data, _)| serde_json::from_slice::<ModelDeployment>(&data).ok());

    let placement = st
        .store
        .get(&format!("/placements/{model_uid}"))
        .await
        .ok()
        .flatten()
        .and_then(|(data, _)| serde_json::from_slice::<PlacementPlan>(&data).ok());

    let endpoints: Vec<EndpointInfo> = st
        .store
        .list_prefix(&format!("/endpoints/{model_uid}/"))
        .await
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(_, v, _)| serde_json::from_slice(&v).ok())
        .collect();

    let stats: Vec<EndpointStats> = st
        .store
        .list_prefix(&format!("/stats/{model_uid}/"))
        .await
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(_, v, _)| serde_json::from_slice(&v).ok())
        .collect();

    let download_progress: Vec<DownloadProgress> = st
        .store
        .list_prefix(&format!("/download_progress/{model_uid}/"))
        .await
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(_, v, _)| serde_json::from_slice(&v).ok())
        .collect();

    // Cache info: scan all model_cache entries and filter by model_name
    let all_caches: Vec<ModelCacheEntry> = st
        .store
        .list_prefix("/model_cache/")
        .await
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(_, v, _)| serde_json::from_slice(&v).ok())
        .filter(|c: &ModelCacheEntry| model_name_matches(&c.model_name, &spec.model_name))
        .collect();

    let state = compute_aggregated_state(
        deployment.as_ref(),
        placement.as_ref(),
        &endpoints,
        &download_progress,
        spec.created_at_ms,
    );

    let desired = deployment.as_ref().map(|d| d.replicas).unwrap_or(0);
    let ready = endpoints
        .iter()
        .filter(|ep| ep.status == nebula_common::EndpointStatus::Ready)
        .count() as u32;
    let unhealthy = endpoints
        .iter()
        .filter(|ep| ep.status == nebula_common::EndpointStatus::Unhealthy)
        .count() as u32;

    let cache_status = if all_caches.is_empty() {
        None
    } else {
        Some(CacheStatusView {
            cached_on_nodes: all_caches.iter().map(|c| c.node_id.clone()).collect(),
            total_size_bytes: all_caches.iter().map(|c| c.size_bytes).sum(),
        })
    };

    let dp_view = if download_progress.is_empty() {
        None
    } else {
        Some(DownloadProgressView {
            replicas: download_progress,
        })
    };

    let detail = ModelDetailView {
        model_uid: spec.model_uid.clone(),
        model_name: spec.model_name.clone(),
        engine_type: spec.engine_type.clone(),
        state,
        replicas: ReplicaCount {
            desired,
            ready,
            unhealthy,
        },
        labels: spec.labels.clone(),
        created_at_ms: spec.created_at_ms,
        updated_at_ms: spec.updated_at_ms,
        spec,
        deployment,
        placement,
        endpoints,
        stats,
        download_progress: dp_view,
        cache_status,
    };

    (StatusCode::OK, Json(detail)).into_response()
}

pub async fn update_model(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Path(model_uid): Path<String>,
    Json(req): Json<UpdateModelRequest>,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&ctx, Role::Operator) {
        return resp;
    }

    let mut spec: ModelSpec = match st.store.get(&format!("/models/{model_uid}/spec")).await {
        Ok(Some((data, _))) => match serde_json::from_slice(&data) {
            Ok(s) => s,
            Err(e) => {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "deserialization_error",
                    &format!("deserialization error: {e}"),
                )
            }
        },
        Ok(None) => {
            return error_response(StatusCode::NOT_FOUND, "not_found", "model not found")
        }
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "etcd_error",
                &format!("etcd error: {e}"),
            )
        }
    };

    if let Some(name) = req.model_name {
        spec.model_name = name;
    }
    if let Some(source) = req.model_source {
        spec.model_source = source;
    }
    if req.model_path.is_some() {
        spec.model_path = req.model_path;
    }
    if req.engine_type.is_some() {
        spec.engine_type = req.engine_type;
    }
    if req.docker_image.is_some() {
        spec.docker_image = req.docker_image;
    }
    if req.config.is_some() {
        spec.config = req.config;
    }
    if let Some(labels) = req.labels {
        spec.labels = labels;
    }
    spec.updated_at_ms = now_ms();

    let val = match serde_json::to_vec(&spec) {
        Ok(v) => v,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "serialization_error",
                &format!("serialization error: {e}"),
            )
        }
    };

    if let Err(e) = st
        .store
        .put(&format!("/models/{model_uid}/spec"), val, None)
        .await
    {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "etcd_error",
            &format!("etcd error: {e}"),
        );
    }

    (StatusCode::OK, Json(json!(spec))).into_response()
}

pub async fn delete_model(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Path(model_uid): Path<String>,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&ctx, Role::Admin) {
        return resp;
    }

    // Verify exists and read model spec for cache GC.
    let spec: ModelSpec = match st.store.get(&format!("/models/{model_uid}/spec")).await {
        Ok(Some((data, _))) => match serde_json::from_slice(&data) {
            Ok(s) => s,
            Err(e) => {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "deserialization_error",
                    &format!("deserialization error: {e}"),
                )
            }
        },
        Ok(None) => return error_response(StatusCode::NOT_FOUND, "not_found", "model not found"),
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "etcd_error",
                &format!("etcd error: {e}"),
            )
        }
    };

    // Enqueue per-node model cache GC requests.
    let mut queued_gc_nodes: usize = 0;
    if let Ok(nodes) = st.store.list_prefix("/node_disk/").await {
        let req = ModelGcRequest {
            model_uid: model_uid.clone(),
            model_name: spec.model_name.clone(),
            model_path: spec.model_path.clone(),
            requested_at_ms: now_ms(),
        };
        if let Ok(payload) = serde_json::to_vec(&req) {
            for (key, _, _) in nodes {
                if let Some(node_id) = key.strip_prefix("/node_disk/").filter(|id| !id.is_empty()) {
                    let gc_key = format!("/model_gc_requests/{node_id}/{model_uid}");
                    if st.store.put(&gc_key, payload.clone(), None).await.is_ok() {
                        queued_gc_nodes += 1;
                    }
                }
            }
        }
    }

    // Delete all related keys
    let _ = st.store.delete(&format!("/models/{model_uid}/spec")).await;
    let _ = st.store.delete(&format!("/deployments/{model_uid}")).await;
    let _ = st.store.delete(&format!("/placements/{model_uid}")).await;

    // Delete all endpoints
    if let Ok(kvs) = st
        .store
        .list_prefix(&format!("/endpoints/{model_uid}/"))
        .await
    {
        for (k, _, _) in kvs {
            let _ = st.store.delete(&k).await;
        }
    }

    // Delete all stats
    if let Ok(kvs) = st
        .store
        .list_prefix(&format!("/stats/{model_uid}/"))
        .await
    {
        for (k, _, _) in kvs {
            let _ = st.store.delete(&k).await;
        }
    }

    (
        StatusCode::OK,
        Json(json!({"model_uid": model_uid, "status": "deleted", "gc_queued_nodes": queued_gc_nodes})),
    )
        .into_response()
}

// ===========================================================================
// Lifecycle Control
// ===========================================================================

pub async fn start_model(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Path(model_uid): Path<String>,
    Json(req): Json<StartModelRequest>,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&ctx, Role::Operator) {
        return resp;
    }

    // Verify spec exists
    if let Ok(None) | Err(_) = st.store.get(&format!("/models/{model_uid}/spec")).await {
        return error_response(StatusCode::NOT_FOUND, "not_found", "model not found");
    }

    let now = now_ms();
    let deployment = match st.store.get(&format!("/deployments/{model_uid}")).await {
        Ok(Some((data, _))) => {
            let mut dep: ModelDeployment =
                serde_json::from_slice(&data).unwrap_or(ModelDeployment {
                    model_uid: model_uid.clone(),
                    desired_state: DesiredState::Stopped,
                    replicas: 1,
                    min_replicas: None,
                    max_replicas: None,
                    node_affinity: None,
                    gpu_affinity: None,
                    config_overrides: None,
                    version: 0,
                    updated_at_ms: 0,
                });
            dep.desired_state = DesiredState::Running;
            if let Some(r) = req.replicas {
                dep.replicas = r;
            }
            if req.config_overrides.is_some() {
                dep.config_overrides = req.config_overrides;
            }
            if req.node_id.is_some() {
                dep.node_affinity = req.node_id;
            }
            if req.gpu_indices.is_some() {
                dep.gpu_affinity = req.gpu_indices;
            }
            dep.version += 1;
            dep.updated_at_ms = now;
            dep
        }
        _ => ModelDeployment {
            model_uid: model_uid.clone(),
            desired_state: DesiredState::Running,
            replicas: req.replicas.unwrap_or(1),
            min_replicas: None,
            max_replicas: None,
            node_affinity: req.node_id,
            gpu_affinity: req.gpu_indices,
            config_overrides: req.config_overrides,
            version: 1,
            updated_at_ms: now,
        },
    };

    let val = match serde_json::to_vec(&deployment) {
        Ok(v) => v,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "serialization_error",
                &format!("serialization error: {e}"),
            )
        }
    };

    if let Err(e) = st
        .store
        .put(&format!("/deployments/{model_uid}"), val, None)
        .await
    {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "etcd_error",
            &format!("etcd error: {e}"),
        );
    }

    (StatusCode::OK, Json(json!(deployment))).into_response()
}

pub async fn stop_model(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Path(model_uid): Path<String>,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&ctx, Role::Operator) {
        return resp;
    }

    // Verify spec exists
    if let Ok(None) | Err(_) = st.store.get(&format!("/models/{model_uid}/spec")).await {
        return error_response(StatusCode::NOT_FOUND, "not_found", "model not found");
    }

    let now = now_ms();
    let deployment = match st.store.get(&format!("/deployments/{model_uid}")).await {
        Ok(Some((data, _))) => {
            let mut dep: ModelDeployment = match serde_json::from_slice(&data) {
                Ok(d) => d,
                Err(e) => {
                    return error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "deserialization_error",
                        &format!("deserialization error: {e}"),
                    )
                }
            };
            dep.desired_state = DesiredState::Stopped;
            dep.version += 1;
            dep.updated_at_ms = now;
            dep
        }
        _ => {
            // No deployment exists, create a stopped one
            ModelDeployment {
                model_uid: model_uid.clone(),
                desired_state: DesiredState::Stopped,
                replicas: 0,
                min_replicas: None,
                max_replicas: None,
                node_affinity: None,
                gpu_affinity: None,
                config_overrides: None,
                version: 1,
                updated_at_ms: now,
            }
        }
    };

    let val = match serde_json::to_vec(&deployment) {
        Ok(v) => v,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "serialization_error",
                &format!("serialization error: {e}"),
            )
        }
    };

    if let Err(e) = st
        .store
        .put(&format!("/deployments/{model_uid}"), val, None)
        .await
    {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "etcd_error",
            &format!("etcd error: {e}"),
        );
    }

    (StatusCode::OK, Json(json!(deployment))).into_response()
}

pub async fn scale_model(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Path(model_uid): Path<String>,
    Json(req): Json<ScaleModelRequest>,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&ctx, Role::Operator) {
        return resp;
    }

    let mut dep: ModelDeployment = match st.store.get(&format!("/deployments/{model_uid}")).await {
        Ok(Some((data, _))) => match serde_json::from_slice(&data) {
            Ok(d) => d,
            Err(e) => {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "deserialization_error",
                    &format!("deserialization error: {e}"),
                )
            }
        },
        Ok(None) => {
            return error_response(
                StatusCode::NOT_FOUND,
                "not_found",
                "deployment not found (model may not be started)",
            )
        }
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "etcd_error",
                &format!("etcd error: {e}"),
            )
        }
    };

    if dep.desired_state == DesiredState::Stopped {
        return error_response(
            StatusCode::CONFLICT,
            "model_stopped",
            "cannot scale a stopped model; start it first",
        );
    }

    dep.replicas = req.replicas;
    dep.version += 1;
    dep.updated_at_ms = now_ms();

    let val = match serde_json::to_vec(&dep) {
        Ok(v) => v,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "serialization_error",
                &format!("serialization error: {e}"),
            )
        }
    };

    if let Err(e) = st
        .store
        .put(&format!("/deployments/{model_uid}"), val, None)
        .await
    {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "etcd_error",
            &format!("etcd error: {e}"),
        );
    }

    (StatusCode::OK, Json(json!(dep))).into_response()
}

// ===========================================================================
// Templates
// ===========================================================================

pub async fn list_templates(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&ctx, Role::Viewer) {
        return resp;
    }

    let kvs = match st.store.list_prefix("/templates/").await {
        Ok(v) => v,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "etcd_error",
                &format!("etcd error: {e}"),
            )
        }
    };

    let templates: Vec<ModelTemplate> = kvs
        .into_iter()
        .filter_map(|(_, v, _)| serde_json::from_slice(&v).ok())
        .collect();

    (StatusCode::OK, Json(templates)).into_response()
}

pub async fn get_template(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&ctx, Role::Viewer) {
        return resp;
    }

    match st.store.get(&format!("/templates/{id}")).await {
        Ok(Some((data, _))) => match serde_json::from_slice::<ModelTemplate>(&data) {
            Ok(t) => (StatusCode::OK, Json(json!(t))).into_response(),
            Err(e) => error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "deserialization_error",
                &format!("deserialization error: {e}"),
            ),
        },
        Ok(None) => error_response(StatusCode::NOT_FOUND, "not_found", "template not found"),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "etcd_error",
            &format!("etcd error: {e}"),
        ),
    }
}

pub async fn create_template(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Json(req): Json<CreateTemplateRequest>,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&ctx, Role::Operator) {
        return resp;
    }

    let tid = req
        .template_id
        .unwrap_or_else(|| format!("tpl-{}", Uuid::new_v4()));
    let now = now_ms();

    let template = ModelTemplate {
        template_id: tid.clone(),
        name: req.name,
        description: req.description,
        category: req.category,
        model_name: req.model_name,
        model_source: req.model_source,
        engine_type: req.engine_type,
        docker_image: req.docker_image,
        config: req.config,
        default_replicas: req.default_replicas.unwrap_or(1),
        labels: req.labels.unwrap_or_default(),
        source: TemplateSource::User,
        created_at_ms: now,
        updated_at_ms: now,
    };

    let val = match serde_json::to_vec(&template) {
        Ok(v) => v,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "serialization_error",
                &format!("serialization error: {e}"),
            )
        }
    };

    if let Err(e) = st.store.put(&format!("/templates/{tid}"), val, None).await {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "etcd_error",
            &format!("etcd error: {e}"),
        );
    }

    (StatusCode::CREATED, Json(json!(template))).into_response()
}

pub async fn update_template(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Path(id): Path<String>,
    Json(req): Json<UpdateTemplateRequest>,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&ctx, Role::Operator) {
        return resp;
    }

    let mut tpl: ModelTemplate = match st.store.get(&format!("/templates/{id}")).await {
        Ok(Some((data, _))) => match serde_json::from_slice(&data) {
            Ok(t) => t,
            Err(e) => {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "deserialization_error",
                    &format!("deserialization error: {e}"),
                )
            }
        },
        Ok(None) => {
            return error_response(StatusCode::NOT_FOUND, "not_found", "template not found")
        }
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "etcd_error",
                &format!("etcd error: {e}"),
            )
        }
    };

    if let Some(name) = req.name {
        tpl.name = name;
    }
    if req.description.is_some() {
        tpl.description = req.description;
    }
    if req.category.is_some() {
        tpl.category = req.category;
    }
    if let Some(model_name) = req.model_name {
        tpl.model_name = model_name;
    }
    if req.model_source.is_some() {
        tpl.model_source = req.model_source;
    }
    if req.engine_type.is_some() {
        tpl.engine_type = req.engine_type;
    }
    if req.docker_image.is_some() {
        tpl.docker_image = req.docker_image;
    }
    if req.config.is_some() {
        tpl.config = req.config;
    }
    if let Some(r) = req.default_replicas {
        tpl.default_replicas = r;
    }
    if let Some(labels) = req.labels {
        tpl.labels = labels;
    }
    tpl.updated_at_ms = now_ms();

    let val = match serde_json::to_vec(&tpl) {
        Ok(v) => v,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "serialization_error",
                &format!("serialization error: {e}"),
            )
        }
    };

    if let Err(e) = st.store.put(&format!("/templates/{id}"), val, None).await {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "etcd_error",
            &format!("etcd error: {e}"),
        );
    }

    (StatusCode::OK, Json(json!(tpl))).into_response()
}

pub async fn delete_template(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&ctx, Role::Operator) {
        return resp;
    }

    if let Ok(None) = st.store.get(&format!("/templates/{id}")).await {
        return error_response(StatusCode::NOT_FOUND, "not_found", "template not found");
    }

    if let Err(e) = st.store.delete(&format!("/templates/{id}")).await {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "etcd_error",
            &format!("etcd error: {e}"),
        );
    }

    (
        StatusCode::OK,
        Json(json!({"template_id": id, "status": "deleted"})),
    )
        .into_response()
}

pub async fn deploy_template(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Path(id): Path<String>,
    Json(req): Json<DeployTemplateRequest>,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&ctx, Role::Operator) {
        return resp;
    }

    let tpl: ModelTemplate = match st.store.get(&format!("/templates/{id}")).await {
        Ok(Some((data, _))) => match serde_json::from_slice(&data) {
            Ok(t) => t,
            Err(e) => {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "deserialization_error",
                    &format!("deserialization error: {e}"),
                )
            }
        },
        Ok(None) => {
            return error_response(StatusCode::NOT_FOUND, "not_found", "template not found")
        }
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "etcd_error",
                &format!("etcd error: {e}"),
            )
        }
    };

    let uid = req
        .model_uid
        .unwrap_or_else(|| generate_model_uid(&tpl.model_name));

    // Check for conflict
    if let Ok(Some(_)) = st.store.get(&format!("/models/{uid}/spec")).await {
        return error_response(
            StatusCode::CONFLICT,
            "model_exists",
            &format!("model with uid '{uid}' already exists"),
        );
    }

    let now = now_ms();
    let spec = ModelSpec {
        model_uid: uid.clone(),
        model_name: tpl.model_name,
        model_source: tpl.model_source.unwrap_or(ModelSource::HuggingFace),
        model_path: None,
        engine_type: tpl.engine_type,
        docker_image: tpl.docker_image,
        config: tpl.config,
        labels: tpl.labels,
        created_at_ms: now,
        updated_at_ms: now,
        created_by: Some(ctx.principal.clone()),
    };

    let spec_val = match serde_json::to_vec(&spec) {
        Ok(v) => v,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "serialization_error",
                &format!("serialization error: {e}"),
            )
        }
    };

    if let Err(e) = st
        .store
        .put(&format!("/models/{uid}/spec"), spec_val, None)
        .await
    {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "etcd_error",
            &format!("etcd error: {e}"),
        );
    }

    // Create deployment with Running state
    let deployment = ModelDeployment {
        model_uid: uid.clone(),
        desired_state: DesiredState::Running,
        replicas: req.replicas.unwrap_or(tpl.default_replicas),
        min_replicas: None,
        max_replicas: None,
        node_affinity: req.node_id,
        gpu_affinity: req.gpu_indices,
        config_overrides: req.config_overrides,
        version: 1,
        updated_at_ms: now,
    };

    if let Ok(dv) = serde_json::to_vec(&deployment) {
        let _ = st.store.put(&format!("/deployments/{uid}"), dv, None).await;
    }

    (StatusCode::CREATED, Json(json!(spec))).into_response()
}

pub async fn save_as_template(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Path(model_uid): Path<String>,
    Json(req): Json<SaveAsTemplateRequest>,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&ctx, Role::Operator) {
        return resp;
    }

    let spec: ModelSpec = match st.store.get(&format!("/models/{model_uid}/spec")).await {
        Ok(Some((data, _))) => match serde_json::from_slice(&data) {
            Ok(s) => s,
            Err(e) => {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "deserialization_error",
                    &format!("deserialization error: {e}"),
                )
            }
        },
        Ok(None) => {
            return error_response(StatusCode::NOT_FOUND, "not_found", "model not found")
        }
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "etcd_error",
                &format!("etcd error: {e}"),
            )
        }
    };

    let deployment = st
        .store
        .get(&format!("/deployments/{model_uid}"))
        .await
        .ok()
        .flatten()
        .and_then(|(data, _)| serde_json::from_slice::<ModelDeployment>(&data).ok());

    let tid = req
        .template_id
        .unwrap_or_else(|| format!("tpl-{}", Uuid::new_v4()));
    let now = now_ms();

    let template = ModelTemplate {
        template_id: tid.clone(),
        name: req.name,
        description: req.description,
        category: req.category,
        model_name: spec.model_name,
        model_source: Some(spec.model_source),
        engine_type: spec.engine_type,
        docker_image: spec.docker_image,
        config: spec.config,
        default_replicas: deployment.as_ref().map(|d| d.replicas).unwrap_or(1),
        labels: spec.labels,
        source: TemplateSource::Saved,
        created_at_ms: now,
        updated_at_ms: now,
    };

    let val = match serde_json::to_vec(&template) {
        Ok(v) => v,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "serialization_error",
                &format!("serialization error: {e}"),
            )
        }
    };

    if let Err(e) = st.store.put(&format!("/templates/{tid}"), val, None).await {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "etcd_error",
            &format!("etcd error: {e}"),
        );
    }

    (StatusCode::CREATED, Json(json!(template))).into_response()
}

// ===========================================================================
// Cache / Disk / Alerts
// ===========================================================================

pub async fn node_cache(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Path(node_id): Path<String>,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&ctx, Role::Viewer) {
        return resp;
    }

    let kvs = match st
        .store
        .list_prefix(&format!("/model_cache/{node_id}/"))
        .await
    {
        Ok(v) => v,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "etcd_error",
                &format!("etcd error: {e}"),
            )
        }
    };

    let caches: Vec<ModelCacheEntry> = kvs
        .into_iter()
        .filter_map(|(_, v, _)| serde_json::from_slice(&v).ok())
        .collect();

    (StatusCode::OK, Json(caches)).into_response()
}

pub async fn node_disk(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Path(node_id): Path<String>,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&ctx, Role::Viewer) {
        return resp;
    }

    match st.store.get(&format!("/node_disk/{node_id}")).await {
        Ok(Some((data, _))) => match serde_json::from_slice::<NodeDiskStatus>(&data) {
            Ok(d) => (StatusCode::OK, Json(json!(d))).into_response(),
            Err(e) => error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "deserialization_error",
                &format!("deserialization error: {e}"),
            ),
        },
        Ok(None) => error_response(
            StatusCode::NOT_FOUND,
            "not_found",
            "disk status not found for node",
        ),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "etcd_error",
            &format!("etcd error: {e}"),
        ),
    }
}

pub async fn cache_summary(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&ctx, Role::Viewer) {
        return resp;
    }

    let cache_entries: Vec<ModelCacheEntry> = st
        .store
        .list_prefix("/model_cache/")
        .await
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(_, v, _)| serde_json::from_slice(&v).ok())
        .collect();

    let specs_raw = st.store.list_prefix("/models/").await.unwrap_or_default();
    let specs: Vec<ModelSpec> = specs_raw
        .into_iter()
        .filter(|(k, _, _)| k.ends_with("/spec"))
        .filter_map(|(_, v, _)| serde_json::from_slice(&v).ok())
        .collect();

    let caches: Vec<CacheEntryView> = cache_entries
        .into_iter()
        .map(|entry| {
            let matched_model_uids = specs
                .iter()
                .filter(|spec| model_name_matches(&entry.model_name, &spec.model_name))
                .map(|spec| spec.model_uid.clone())
                .collect();
            CacheEntryView {
                entry,
                matched_model_uids,
            }
        })
        .collect();

    let nodes: Vec<NodeDiskStatus> = st
        .store
        .list_prefix("/node_disk/")
        .await
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(_, v, _)| serde_json::from_slice(&v).ok())
        .collect();

    let total_size: u64 = caches.iter().map(|c| c.entry.size_bytes).sum();

    let summary = CacheSummary {
        total_cached_models: caches.len(),
        total_cache_size_bytes: total_size,
        nodes,
        caches,
    };

    (StatusCode::OK, Json(summary)).into_response()
}

pub async fn list_alerts(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&ctx, Role::Viewer) {
        return resp;
    }

    let kvs = match st.store.list_prefix("/alerts/").await {
        Ok(v) => v,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "etcd_error",
                &format!("etcd error: {e}"),
            )
        }
    };

    let alerts: Vec<DiskAlert> = kvs
        .into_iter()
        .filter_map(|(_, v, _)| serde_json::from_slice(&v).ok())
        .collect();

    (StatusCode::OK, Json(alerts)).into_response()
}

// ===========================================================================
// V1 → V2 Migration
// ===========================================================================

#[derive(Serialize)]
struct MigrationDetail {
    model_uid: String,
    action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    desired_state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

#[derive(Serialize)]
struct MigrationResult {
    total: usize,
    migrated: usize,
    skipped: usize,
    failed: usize,
    details: Vec<MigrationDetail>,
}

pub async fn migrate_v1_to_v2(
    State(st): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
) -> impl IntoResponse {
    if let Some(resp) = require_role(&ctx, Role::Admin) {
        return resp;
    }

    // 1. List all existing model_requests
    let requests_raw = match st.store.list_prefix("/model_requests/").await {
        Ok(r) => r,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "etcd_error",
                &format!("failed to list model_requests: {e}"),
            );
        }
    };

    let model_requests: Vec<ModelRequest> = requests_raw
        .into_iter()
        .filter_map(|(_, v, _)| serde_json::from_slice(&v).ok())
        .collect();

    let total = model_requests.len();
    let mut migrated = 0usize;
    let mut skipped = 0usize;
    let mut failed = 0usize;
    let mut details = Vec::new();

    for mr in &model_requests {
        let model_uid = &mr.request.model_uid;

        // 2a. Check if already migrated (idempotency)
        match st.store.get(&format!("/models/{model_uid}/spec")).await {
            Ok(Some(_)) => {
                skipped += 1;
                details.push(MigrationDetail {
                    model_uid: model_uid.clone(),
                    action: "skipped".to_string(),
                    desired_state: None,
                    reason: Some("already_exists".to_string()),
                });
                continue;
            }
            Ok(None) => {}
            Err(e) => {
                failed += 1;
                details.push(MigrationDetail {
                    model_uid: model_uid.clone(),
                    action: "failed".to_string(),
                    desired_state: None,
                    reason: Some(format!("etcd get error: {e}")),
                });
                continue;
            }
        }

        let now = now_ms();

        // 2b. Build ModelSpec from ModelLoadRequest
        let spec = ModelSpec {
            model_uid: model_uid.clone(),
            model_name: mr.request.model_name.clone(),
            model_source: ModelSource::HuggingFace,
            model_path: None,
            engine_type: mr.request.engine_type.clone(),
            docker_image: mr.request.docker_image.clone(),
            config: mr.request.config.clone(),
            labels: HashMap::new(),
            created_at_ms: mr.created_at_ms,
            updated_at_ms: now,
            created_by: Some("migration".to_string()),
        };

        // 2c. Write ModelSpec
        let spec_val = match serde_json::to_vec(&spec) {
            Ok(v) => v,
            Err(e) => {
                failed += 1;
                details.push(MigrationDetail {
                    model_uid: model_uid.clone(),
                    action: "failed".to_string(),
                    desired_state: None,
                    reason: Some(format!("spec serialization error: {e}")),
                });
                continue;
            }
        };

        if let Err(e) = st
            .store
            .put(&format!("/models/{model_uid}/spec"), spec_val, None)
            .await
        {
            failed += 1;
            details.push(MigrationDetail {
                model_uid: model_uid.clone(),
                action: "failed".to_string(),
                desired_state: None,
                reason: Some(format!("spec write error: {e}")),
            });
            continue;
        }

        // 2d. Build ModelDeployment
        let desired_state = match &mr.status {
            ModelRequestStatus::Running | ModelRequestStatus::Scheduled => DesiredState::Running,
            _ => DesiredState::Stopped,
        };

        let gpu_affinity = mr
            .request
            .gpu_indices
            .clone()
            .or_else(|| mr.request.gpu_index.map(|idx| vec![idx]));

        let deployment = ModelDeployment {
            model_uid: model_uid.clone(),
            desired_state: desired_state.clone(),
            replicas: mr.request.replicas,
            min_replicas: mr.request.min_replicas,
            max_replicas: mr.request.max_replicas,
            node_affinity: mr.request.node_id.clone(),
            gpu_affinity,
            config_overrides: mr.request.config.clone(),
            version: 1,
            updated_at_ms: now,
        };

        // 2e. Write ModelDeployment
        let dep_val = match serde_json::to_vec(&deployment) {
            Ok(v) => v,
            Err(e) => {
                failed += 1;
                details.push(MigrationDetail {
                    model_uid: model_uid.clone(),
                    action: "failed".to_string(),
                    desired_state: None,
                    reason: Some(format!("deployment serialization error: {e}")),
                });
                continue;
            }
        };

        if let Err(e) = st
            .store
            .put(&format!("/deployments/{model_uid}"), dep_val, None)
            .await
        {
            failed += 1;
            details.push(MigrationDetail {
                model_uid: model_uid.clone(),
                action: "failed".to_string(),
                desired_state: None,
                reason: Some(format!("deployment write error: {e}")),
            });
            continue;
        }

        let ds_str = match desired_state {
            DesiredState::Running => "running",
            DesiredState::Stopped => "stopped",
        };

        migrated += 1;
        details.push(MigrationDetail {
            model_uid: model_uid.clone(),
            action: "migrated".to_string(),
            desired_state: Some(ds_str.to_string()),
            reason: None,
        });
    }

    let result = MigrationResult {
        total,
        migrated,
        skipped,
        failed,
        details,
    };

    (StatusCode::OK, Json(result)).into_response()
}