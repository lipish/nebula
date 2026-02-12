use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use nebula_common::{EndpointInfo, EndpointStatus, NodeStatus};
use nebula_meta::{EtcdMetaStore, MetaStore};

use crate::engine::container_name;
use crate::gpu::read_gpu_statuses;
use crate::reconcile::RunningModel;
use crate::scrape::scrape_engine_stats;
use crate::util::now_ms;

/// Number of consecutive health-check failures before marking endpoint as Unhealthy.
const UNHEALTHY_THRESHOLD: u32 = 3;
/// Number of consecutive failures before attempting a container restart.
const RESTART_THRESHOLD: u32 = 5;
/// Cooldown period after a restart attempt (seconds). During this time health checks
/// are skipped to give the engine time to initialize.
const RESTART_COOLDOWN_SECS: u64 = 120;

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
    api_port: u16,
    endpoint: Arc<Mutex<HashMap<String, EndpointInfo>>>,
    running: Arc<Mutex<HashMap<String, RunningModel>>>,
) {
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap_or_default();

    // Track consecutive health-check failures per model_uid
    let mut fail_counts: HashMap<String, u32> = HashMap::new();
    // Track last restart timestamp (ms) per model_uid for cooldown
    let mut restart_at: HashMap<String, u64> = HashMap::new();

    let key = format!("/nodes/{}/status", node_id);
    loop {
        let gpus = read_gpu_statuses().await;
        let status = NodeStatus {
            node_id: node_id.clone(),
            last_heartbeat_ms: now_ms(),
            gpus,
            api_addr: Some(format!("http://0.0.0.0:{}", api_port)),
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

        // Refresh endpoint registrations
        let mut guard = endpoint.lock().await;
        for info in guard.values_mut() {
            info.last_heartbeat_ms = now_ms();
            if let Err(e) = register_endpoint(&store, info, ttl_ms).await {
                tracing::warn!(error=%e, "failed to refresh endpoint");
            }
        }
        drop(guard);

        // Scrape engine metrics and write to etcd /stats/
        // Also perform health checks on each running engine
        {
            let running_guard = running.lock().await;
            let now = now_ms();
            for rm in running_guard.values() {
                // Skip health check during restart cooldown
                if let Some(&last_restart) = restart_at.get(&rm.model_uid) {
                    let elapsed_secs = (now.saturating_sub(last_restart)) / 1000;
                    if elapsed_secs < RESTART_COOLDOWN_SECS {
                        tracing::debug!(
                            model_uid=%rm.model_uid,
                            remaining_secs=RESTART_COOLDOWN_SECS - elapsed_secs,
                            "skipping health check during restart cooldown"
                        );
                        continue;
                    }
                }

                let health_url = format!("{}/health", rm.base_url);
                let healthy = match http.get(&health_url).send().await {
                    Ok(resp) => resp.status().is_success(),
                    Err(_) => false,
                };

                let count = fail_counts.entry(rm.model_uid.clone()).or_insert(0);

                if healthy {
                    // Recovered: reset counter and ensure endpoint is Ready
                    if *count > 0 {
                        tracing::info!(model_uid=%rm.model_uid, prev_failures=*count, "engine recovered");
                        *count = 0;
                        restart_at.remove(&rm.model_uid);
                        // Mark endpoint back to Ready
                        let mut ep_guard = endpoint.lock().await;
                        if let Some(info) = ep_guard.get_mut(&rm.model_uid) {
                            if info.status == EndpointStatus::Unhealthy {
                                info.status = EndpointStatus::Ready;
                                let _ = register_endpoint(&store, info, ttl_ms).await;
                                tracing::info!(model_uid=%rm.model_uid, "endpoint marked Ready again");
                            }
                        }
                    }

                    // Scrape metrics only when healthy
                    if let Some(stats) =
                        scrape_engine_stats(&http, &rm.base_url, &rm.model_uid, rm.replica_id).await
                    {
                        let stats_key = format!("/stats/{}/{}", rm.model_uid, rm.replica_id);
                        match serde_json::to_vec(&stats) {
                            Ok(bytes) => {
                                if let Err(e) = store.put(&stats_key, bytes, Some(ttl_ms)).await {
                                    tracing::warn!(error=%e, %stats_key, "failed to write engine stats");
                                }
                            }
                            Err(e) => {
                                tracing::warn!(error=%e, "failed to serialize engine stats");
                            }
                        }
                    }
                } else {
                    *count += 1;
                    tracing::warn!(
                        model_uid=%rm.model_uid,
                        consecutive_failures=*count,
                        "engine health check failed"
                    );

                    // Mark Unhealthy after threshold
                    if *count == UNHEALTHY_THRESHOLD {
                        let mut ep_guard = endpoint.lock().await;
                        if let Some(info) = ep_guard.get_mut(&rm.model_uid) {
                            info.status = EndpointStatus::Unhealthy;
                            let _ = register_endpoint(&store, info, ttl_ms).await;
                            tracing::warn!(model_uid=%rm.model_uid, "endpoint marked Unhealthy");
                        }
                    }

                    // Attempt container restart after higher threshold
                    if *count >= RESTART_THRESHOLD {
                        let cname = container_name(&rm.model_uid, rm.replica_id);
                        tracing::warn!(model_uid=%rm.model_uid, container=%cname, "attempting docker restart");
                        let _ = tokio::process::Command::new("docker")
                            .args(["restart", "-t", "10", &cname])
                            .output()
                            .await;
                        // Keep count at 1 (not 0) so that recovery is detected after cooldown
                        *count = 1;
                        restart_at.insert(rm.model_uid.clone(), now_ms());
                    }
                }
            }
        }

        tokio::time::sleep(Duration::from_millis(interval_ms)).await;
    }
}
