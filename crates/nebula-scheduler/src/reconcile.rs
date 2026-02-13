use std::collections::HashMap;
use std::time::Duration;

use tracing::{info, warn};

use nebula_common::{
    EndpointInfo, EndpointStats, EndpointStatus, ModelRequest, ModelRequestStatus, PlacementPlan,
};
use nebula_meta::{EtcdMetaStore, MetaStore};

use crate::planner::{allocate_port, list_used_resources, select_node_and_gpus};
use crate::util::now_ms;

/// Endpoint heartbeat timeout: if last_heartbeat_ms is older than this, consider it dead.
const ENDPOINT_TIMEOUT_MS: u64 = 30_000;

/// Reconcile interval.
const RECONCILE_INTERVAL: Duration = Duration::from_secs(30);

/// KV cache usage fraction above which we consider scaling up.
const SCALE_UP_KV_THRESHOLD: f64 = 0.80;

/// Pending requests average above which we consider scaling up.
const SCALE_UP_PENDING_THRESHOLD: f64 = 5.0;

/// If avg pending_requests == 0 for all endpoints, consider scaling down.
/// (We only scale down after a cooldown — tracked via plan version age.)
const SCALE_DOWN_IDLE_THRESHOLD: f64 = 0.0;

/// Minimum time (ms) a plan must be stable before we allow scale-down.
const SCALE_DOWN_COOLDOWN_MS: u64 = 300_000; // 5 minutes

/// Main reconcile loop. Runs periodically alongside the watch-based placement loop.
///
/// For each model with a PlacementPlan:
///   1. Check endpoint health — remove stale assignments whose endpoints timed out.
///   2. Check replica count — if fewer healthy replicas than desired, try to add new ones.
pub async fn reconcile_loop(store: EtcdMetaStore, default_port: u16) {
    // Wait a bit before first reconcile to let the system stabilize.
    tokio::time::sleep(Duration::from_secs(10)).await;
    info!("reconcile loop started (interval={}s)", RECONCILE_INTERVAL.as_secs());

    loop {
        if let Err(e) = reconcile_once(&store, default_port).await {
            warn!(error=%e, "reconcile cycle failed");
        }
        tokio::time::sleep(RECONCILE_INTERVAL).await;
    }
}

async fn reconcile_once(store: &EtcdMetaStore, default_port: u16) -> anyhow::Result<()> {
    let now = now_ms();

    // 1. Load all placements
    let placement_kvs = store.list_prefix("/placements/").await?;
    let mut plans: Vec<PlacementPlan> = Vec::new();
    for (_, val, _) in &placement_kvs {
        if let Ok(plan) = serde_json::from_slice::<PlacementPlan>(val) {
            plans.push(plan);
        }
    }

    if plans.is_empty() {
        return Ok(());
    }

    // 2. Load all endpoints
    let endpoint_kvs = store.list_prefix("/endpoints/").await?;
    let mut endpoints: HashMap<(String, u32), EndpointInfo> = HashMap::new();
    for (_, val, _) in &endpoint_kvs {
        if let Ok(ep) = serde_json::from_slice::<EndpointInfo>(val) {
            endpoints.insert((ep.model_uid.clone(), ep.replica_id), ep);
        }
    }

    // 3. Load all model requests (to know desired replica count)
    let request_kvs = store.list_prefix("/model_requests/").await?;
    let mut requests: HashMap<String, ModelRequest> = HashMap::new();
    for (_, val, _) in &request_kvs {
        if let Ok(req) = serde_json::from_slice::<ModelRequest>(val) {
            if req.status == ModelRequestStatus::Scheduled
                || req.status == ModelRequestStatus::Running
            {
                requests.insert(req.request.model_uid.clone(), req);
            }
        }
    }

    // 4. Load all stats (for autoscaling decisions)
    let stats_kvs = store.list_prefix("/stats/").await?;
    let mut stats_by_model: HashMap<String, Vec<EndpointStats>> = HashMap::new();
    for (_, val, _) in &stats_kvs {
        if let Ok(s) = serde_json::from_slice::<EndpointStats>(val) {
            stats_by_model
                .entry(s.model_uid.clone())
                .or_default()
                .push(s);
        }
    }

    // 5. For each placement, reconcile
    for plan in &plans {
        let req_opt = requests.get(&plan.model_uid);
        let base_replicas = req_opt
            .map(|r| r.request.replicas.max(1))
            .unwrap_or(plan.assignments.len() as u32);
        let min_replicas = req_opt
            .and_then(|r| r.request.min_replicas)
            .unwrap_or(1)
            .max(1);
        let max_replicas = req_opt
            .and_then(|r| r.request.max_replicas)
            .unwrap_or(base_replicas);

        // Compute desired replicas: start from base, adjust by load signals
        let desired_replicas = compute_desired_replicas(
            base_replicas,
            min_replicas,
            max_replicas,
            plan,
            stats_by_model.get(&plan.model_uid),
            now,
        );

        // Identify healthy vs stale assignments
        let mut healthy_assignments = Vec::new();
        let mut stale_replica_ids = Vec::new();

        for assignment in &plan.assignments {
            let key = (plan.model_uid.clone(), assignment.replica_id);
            match endpoints.get(&key) {
                Some(ep) => {
                    let age = now.saturating_sub(ep.last_heartbeat_ms);
                    if age > ENDPOINT_TIMEOUT_MS || ep.status == EndpointStatus::Unhealthy {
                        warn!(
                            model_uid=%plan.model_uid,
                            replica_id=assignment.replica_id,
                            age_ms=age,
                            status=?ep.status,
                            "endpoint stale/unhealthy, removing assignment"
                        );
                        stale_replica_ids.push(assignment.replica_id);
                    } else {
                        healthy_assignments.push(assignment.clone());
                    }
                }
                None => {
                    // No endpoint registered yet — could be still starting.
                    // Only remove if the plan is old enough (give 2 minutes for startup).
                    let plan_age = now.saturating_sub(plan.version);
                    if plan_age > 120_000 {
                        warn!(
                            model_uid=%plan.model_uid,
                            replica_id=assignment.replica_id,
                            plan_age_ms=plan_age,
                            "no endpoint registered after 2min, removing assignment"
                        );
                        stale_replica_ids.push(assignment.replica_id);
                    } else {
                        // Still within startup grace period, keep it.
                        healthy_assignments.push(assignment.clone());
                    }
                }
            }
        }

        // Clean up stale endpoint keys from etcd
        for replica_id in &stale_replica_ids {
            let ep_key = format!("/endpoints/{}/{}", plan.model_uid, replica_id);
            let _ = store.delete(&ep_key).await;

            let stats_key = format!("/stats/{}/{}", plan.model_uid, replica_id);
            let _ = store.delete(&stats_key).await;
        }

        let need_update = !stale_replica_ids.is_empty()
            || (healthy_assignments.len() as u32) < desired_replicas;

        if !need_update {
            continue;
        }

        // Try to add new assignments to meet desired replica count
        let mut new_assignments = healthy_assignments.clone();
        let current_count = new_assignments.len() as u32;

        if current_count < desired_replicas {
            let deficit = desired_replicas - current_count;
            info!(
                model_uid=%plan.model_uid,
                current=current_count,
                desired=desired_replicas,
                deficit=deficit,
                "attempting to add replacement replicas"
            );

            // Get the original request for scheduling parameters
            if let Some(req) = requests.get(&plan.model_uid) {
                let (mut used_ports, mut used_gpus) =
                    list_used_resources(store).await.unwrap_or_default();

                // Also mark ports/GPUs from our healthy assignments as used
                for a in &new_assignments {
                    used_ports.insert(a.port);
                    if let Some(indices) = a.effective_gpu_indices() {
                        let entry = used_gpus.entry(a.node_id.clone()).or_default();
                        for idx in indices {
                            entry.insert(idx);
                        }
                    }
                }

                // Find the next available replica_id
                let max_existing_id = new_assignments
                    .iter()
                    .map(|a| a.replica_id)
                    .max()
                    .unwrap_or(0);

                let extra_args = crate::planner::build_extra_args(req);

                for i in 0..deficit {
                    let new_replica_id = max_existing_id + 1 + i;

                    match select_node_and_gpus(store, req, &used_gpus).await {
                        Ok((node_id, gpu_indices)) => {
                            let port = allocate_port(default_port, &used_ports);
                            used_ports.insert(port);

                            if !gpu_indices.is_empty() {
                                let entry = used_gpus.entry(node_id.clone()).or_default();
                                for &idx in &gpu_indices {
                                    entry.insert(idx);
                                }
                            }

                            let gpu_index = if gpu_indices.len() == 1 {
                                Some(gpu_indices[0])
                            } else {
                                gpu_indices.first().copied()
                            };
                            let gpu_indices_field = if gpu_indices.is_empty() {
                                None
                            } else {
                                Some(gpu_indices)
                            };

                            new_assignments.push(nebula_common::PlacementAssignment {
                                replica_id: new_replica_id,
                                node_id,
                                engine_config_path: format!(
                                    "/tmp/nebula/{}.yaml",
                                    plan.model_uid
                                ),
                                port,
                                gpu_index,
                                gpu_indices: gpu_indices_field,
                                extra_args: extra_args.clone(),
                                engine_type: None,
                                docker_image: None,
                            });

                            info!(
                                model_uid=%plan.model_uid,
                                replica_id=new_replica_id,
                                "added replacement assignment"
                            );
                        }
                        Err(e) => {
                            warn!(
                                model_uid=%plan.model_uid,
                                error=%e,
                                "failed to find node for replacement replica"
                            );
                            break;
                        }
                    }
                }
            }
        }

        // Write updated plan (bump version)
        let updated_plan = PlacementPlan {
            request_id: plan.request_id.clone(),
            model_uid: plan.model_uid.clone(),
            model_name: plan.model_name.clone(),
            version: now_ms(),
            assignments: new_assignments,
        };

        let placement_key = format!("/placements/{}", plan.model_uid);
        match serde_json::to_vec(&updated_plan) {
            Ok(val) => {
                if let Err(e) = store.put(&placement_key, val, None).await {
                    warn!(model_uid=%plan.model_uid, error=%e, "failed to write updated placement");
                } else {
                    info!(
                        model_uid=%plan.model_uid,
                        old_assignments=plan.assignments.len(),
                        new_assignments=updated_plan.assignments.len(),
                        "reconcile: updated placement"
                    );
                }
            }
            Err(e) => {
                warn!(model_uid=%plan.model_uid, error=%e, "failed to serialize updated placement");
            }
        }
    }

    Ok(())
}

/// Compute the desired replica count based on load signals and autoscaling bounds.
fn compute_desired_replicas(
    base_replicas: u32,
    min_replicas: u32,
    max_replicas: u32,
    plan: &PlacementPlan,
    stats: Option<&Vec<EndpointStats>>,
    now: u64,
) -> u32 {
    let current = plan.assignments.len() as u32;

    // If no autoscaling range (min == max or no stats), use base
    if min_replicas >= max_replicas {
        return base_replicas.clamp(min_replicas, max_replicas);
    }

    let stats = match stats {
        Some(s) if !s.is_empty() => s,
        _ => return current.clamp(min_replicas, max_replicas),
    };

    // Compute average KV cache usage fraction
    let mut kv_count = 0u32;
    let mut kv_usage_sum = 0.0f64;
    for s in stats.iter() {
        if let (Some(used), Some(free)) = (s.kv_cache_used_bytes, s.kv_cache_free_bytes) {
            let total = used + free;
            if total > 0 {
                kv_usage_sum += used as f64 / total as f64;
                kv_count += 1;
            }
        }
    }
    let avg_kv_usage = if kv_count > 0 {
        kv_usage_sum / kv_count as f64
    } else {
        0.0
    };

    // Compute average pending requests
    let avg_pending: f64 = stats.iter().map(|s| s.pending_requests as f64).sum::<f64>()
        / stats.len() as f64;

    // Scale-up: if avg KV cache usage > threshold OR avg pending > threshold
    if avg_kv_usage > SCALE_UP_KV_THRESHOLD || avg_pending > SCALE_UP_PENDING_THRESHOLD {
        let target = (current + 1).min(max_replicas);
        if target > current {
            info!(
                model_uid=%plan.model_uid,
                avg_kv_usage=format!("{:.2}", avg_kv_usage),
                avg_pending=format!("{:.1}", avg_pending),
                current=current,
                target=target,
                "autoscale: scaling up"
            );
            return target;
        }
    }

    // Scale-down: if avg pending == 0 and KV usage is low, and cooldown has passed
    if avg_pending <= SCALE_DOWN_IDLE_THRESHOLD && avg_kv_usage < SCALE_UP_KV_THRESHOLD * 0.5 {
        let plan_age = now.saturating_sub(plan.version);
        if plan_age > SCALE_DOWN_COOLDOWN_MS && current > min_replicas {
            let target = (current - 1).max(min_replicas);
            info!(
                model_uid=%plan.model_uid,
                avg_kv_usage=format!("{:.2}", avg_kv_usage),
                avg_pending=format!("{:.1}", avg_pending),
                current=current,
                target=target,
                plan_age_ms=plan_age,
                "autoscale: scaling down"
            );
            return target;
        }
    }

    // No change needed
    current.clamp(min_replicas, max_replicas)
}
