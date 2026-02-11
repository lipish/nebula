use std::collections::HashMap;
use std::sync::Arc;

use tokio::process::Child;
use tokio::sync::Mutex;

use nebula_common::{EndpointInfo, EndpointKind, EndpointStatus, ModelRequest, ModelRequestStatus, PlacementPlan};
use nebula_meta::{EtcdMetaStore, MetaStore};

use crate::args::Args;
use crate::engine::{start_vllm, write_engine_env};
use crate::heartbeat::{delete_endpoint, register_endpoint};
use crate::util::{now_ms, stop_child};

pub struct RunningModel {
    pub model_uid: String,
    pub replica_id: u32,
    pub plan_version: u64,
    pub child: Child,
}

async fn mark_request_failed(store: &EtcdMetaStore, request_id: &str, reason: String) {
    let key = format!("/model_requests/{request_id}");
    let loaded = store.get(&key).await;
    let Ok(Some((bytes, _rev))) = loaded else {
        tracing::warn!(%request_id, "failed to load model request for failure update");
        return;
    };
    let mut req: ModelRequest = match serde_json::from_slice(&bytes) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(%request_id, error=%e, "failed to deserialize model request for failure update");
            return;
        }
    };
    req.status = ModelRequestStatus::Failed(reason);
    let Ok(val) = serde_json::to_vec(&req) else {
        tracing::warn!(%request_id, "failed to serialize model request for failure update");
        return;
    };
    if let Err(e) = store.put(&key, val, None).await {
        tracing::warn!(%request_id, error=%e, "failed to persist model request failure update");
    }
}

pub async fn reconcile_model(
    store: &EtcdMetaStore,
    args: &Args,
    running: &mut HashMap<String, RunningModel>,
    endpoint_state: &Arc<Mutex<HashMap<String, EndpointInfo>>>,
    model_uid: &str,
    plan: Option<PlacementPlan>,
) -> anyhow::Result<()> {
    let plan = match plan {
        Some(p) => p,
        None => {
            if let Some(mut rm) = running.remove(model_uid) {
                tracing::info!(%model_uid, "stopping engine");
                stop_child(&mut rm.child).await;
                let _ = delete_endpoint(store, &rm.model_uid, rm.replica_id).await;
                endpoint_state.lock().await.remove(model_uid);
            }
            return Ok(());
        }
    };

    let desired = plan.assignments.iter().find(|a| a.node_id == args.node_id);

    let Some(assignment) = desired else {
        if let Some(mut rm) = running.remove(model_uid) {
            tracing::info!(%model_uid, "no longer assigned, stopping engine");
            stop_child(&mut rm.child).await;
            let _ = delete_endpoint(store, &rm.model_uid, rm.replica_id).await;
            endpoint_state.lock().await.remove(model_uid);
        }
        return Ok(());
    };

    let needs_restart = match running.get(model_uid) {
        Some(rm) => rm.replica_id != assignment.replica_id || rm.plan_version != plan.version,
        None => true,
    };

    if !needs_restart {
        return Ok(());
    }

    if let Some(mut rm) = running.remove(model_uid) {
        tracing::info!(%model_uid, "restarting engine due to placement update");
        stop_child(&mut rm.child).await;
        let _ = delete_endpoint(store, &rm.model_uid, rm.replica_id).await;
        endpoint_state.lock().await.remove(model_uid);
    }

    let (child, base_url, engine_model) = match start_vllm(args, assignment, model_uid, &plan.model_name).await {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(%model_uid, error=%e, "failed to start vllm engine");
            if let Some(request_id) = plan.request_id.as_deref() {
                mark_request_failed(store, request_id, e.to_string()).await;
            }
            return Ok(());
        }
    };
    write_engine_env(&args.engine_env_path, &base_url, &engine_model).await?;

    let info = EndpointInfo {
        model_uid: plan.model_uid.clone(),
        replica_id: assignment.replica_id,
        plan_version: plan.version,
        node_id: args.node_id.clone(),
        endpoint_kind: EndpointKind::NativeHttp,
        api_flavor: "openai".to_string(),
        status: EndpointStatus::Ready,
        last_heartbeat_ms: now_ms(),
        grpc_target: None,
        base_url: Some(base_url.clone()),
    };

    register_endpoint(store, &info, args.heartbeat_ttl_ms).await?;
    tracing::info!(model_uid=%info.model_uid, replica_id=info.replica_id, base_url=%base_url, "registered endpoint");

    endpoint_state
        .lock()
        .await
        .insert(model_uid.to_string(), info);
    running.insert(
        model_uid.to_string(),
        RunningModel {
            model_uid: plan.model_uid,
            replica_id: assignment.replica_id,
            plan_version: plan.version,
            child,
        },
    );

    Ok(())
}
