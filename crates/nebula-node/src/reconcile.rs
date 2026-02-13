use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use nebula_common::{EndpointInfo, EndpointKind, EndpointStatus, ModelRequest, ModelRequestStatus, PlacementPlan};
use nebula_meta::{EtcdMetaStore, MetaStore};

use crate::args::Args;
use crate::engine::{write_engine_env, Engine, EngineHandle, EngineStartContext};
use crate::heartbeat::{delete_endpoint, register_endpoint};
use crate::util::now_ms;

pub struct RunningModel {
    pub model_uid: String,
    pub replica_id: u32,
    pub plan_version: u64,
    pub handle: EngineHandle,
    pub engine: Arc<dyn Engine>,
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
                rm.engine.stop(&mut rm.handle).await?;
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
            rm.engine.stop(&mut rm.handle).await?;
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
        rm.engine.stop(&mut rm.handle).await?;
        let _ = delete_endpoint(store, &rm.model_uid, rm.replica_id).await;
        endpoint_state.lock().await.remove(model_uid);
    }

    // Create engine instance based on assignment's engine_type
    let engine_type = assignment.engine_type.as_deref();
    let engine: Arc<dyn Engine> = Arc::from(crate::engine::create_engine(args, engine_type));

    let ctx = EngineStartContext {
        model_uid: model_uid.to_string(),
        model_name: plan.model_name.clone(),
        replica_id: assignment.replica_id,
        port: assignment.port,
        gpu_indices: assignment.effective_gpu_indices(),
        engine_config_path: assignment.engine_config_path.clone(),
        extra_args: assignment.extra_args.clone(),
        ready_timeout: Duration::from_secs(args.ready_timeout_secs),
    };

    // Try to reuse an existing instance before starting a new one.
    let handle = if let Some(h) = engine.try_reuse(&ctx).await {
        tracing::info!(%model_uid, base_url=%h.base_url, engine=%engine.engine_type(), "reused existing engine instance");
        h
    } else {
        tracing::info!(%model_uid, engine=%engine.engine_type(), "starting new engine instance");
        match engine.start(ctx).await {
            Ok(h) => h,
            Err(e) => {
                tracing::error!(%model_uid, error=%e, engine=%engine.engine_type(), "failed to start engine");
                if let Some(request_id) = plan.request_id.as_deref() {
                    mark_request_failed(store, request_id, e.to_string()).await;
                }
                return Ok(());
            }
        }
    };

    write_engine_env(&args.engine_env_path, &handle.base_url, &handle.engine_model).await?;

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
        base_url: Some(handle.base_url.clone()),
    };

    register_endpoint(store, &info, args.heartbeat_ttl_ms).await?;
    tracing::info!(model_uid=%info.model_uid, replica_id=info.replica_id, base_url=%handle.base_url, "registered endpoint");

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
            handle,
            engine,
        },
    );

    Ok(())
}
