use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use nebula_common::{EndpointInfo, NodeStatus};
use nebula_meta::{EtcdMetaStore, MetaStore};

use crate::gpu::read_gpu_statuses;
use crate::util::now_ms;

pub async fn register_endpoint(
    store: &EtcdMetaStore,
    info: &EndpointInfo,
    ttl_ms: u64,
) -> anyhow::Result<()> {
    let key = format!("/endpoints/{}/{}", info.model_uid, info.replica_id);
    let bytes = serde_json::to_vec(info)?;
    let _ = store.put(&key, bytes, Some(ttl_ms)).await?;
    Ok(())
}

pub async fn delete_endpoint(
    store: &EtcdMetaStore,
    model_uid: &str,
    replica_id: u32,
) -> anyhow::Result<()> {
    let key = format!("/endpoints/{}/{}", model_uid, replica_id);
    let _ = store.delete(&key).await?;
    Ok(())
}

pub async fn heartbeat_loop(
    store: EtcdMetaStore,
    node_id: String,
    ttl_ms: u64,
    interval_ms: u64,
    endpoint: Arc<Mutex<HashMap<String, EndpointInfo>>>,
) {
    let key = format!("/nodes/{}/status", node_id);
    loop {
        let gpus = read_gpu_statuses().await;
        let status = NodeStatus {
            node_id: node_id.clone(),
            last_heartbeat_ms: now_ms(),
            gpus,
        };

        let bytes = match serde_json::to_vec(&status) {
            Ok(b) => b,
            Err(_) => {
                tokio::time::sleep(Duration::from_millis(interval_ms)).await;
                continue;
            }
        };

        if let Err(e) = store.put(&key, bytes, Some(ttl_ms)).await {
            tracing::warn!(error=%e, "failed to write heartbeat");
        }

        let mut guard = endpoint.lock().await;
        for info in guard.values_mut() {
            info.last_heartbeat_ms = now_ms();
            if let Err(e) = register_endpoint(&store, info, ttl_ms).await {
                tracing::warn!(error=%e, "failed to refresh endpoint");
            }
        }

        tokio::time::sleep(Duration::from_millis(interval_ms)).await;
    }
}
