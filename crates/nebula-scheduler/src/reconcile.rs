use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use tracing::{info, warn};

use nebula_common::{
    DesiredState, EndpointInfo, EndpointStats, EndpointStatus, ModelDeployment, ModelRequest,
    ModelRequestStatus, PlacementPlan,
};
use nebula_meta::{EtcdMetaStore, MetaStore};

use crate::metrics::SharedMetrics;
use crate::planner::{allocate_port, list_used_resources, select_node_and_gpus};
use crate::util::now_ms;

/// Endpoint heartbeat timeout: if last_heartbeat_ms is older than this, consider it dead.
/// Keep this generous to avoid reclaiming assignments during cold starts.
const ENDPOINT_TIMEOUT_MS: u64 = 300_000;

/// Startup grace period for assignments with no endpoint yet.
/// Large-model cold starts (download + compile + graph capture) can exceed minutes.
const STARTUP_GRACE_MS: u64 = 900_000;

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
pub async fn reconcile_loop(
    store: EtcdMetaStore,
    default_port: u16,
    xtrace: Option<xtrace_client::Client>,
    metrics: Arc<SharedMetrics>,
) {
    // Wait a bit before first reconcile to let the system stabilize.
    tokio::time::sleep(Duration::from_secs(10)).await;
    info!("reconcile loop started (interval={}s)", RECONCILE_INTERVAL.as_secs());

    loop {
        metrics.reconcile_total.fetch_add(1, Ordering::Relaxed);
        if let Err(e) = reconcile_once(&store, default_port, xtrace.as_ref(), &metrics).await {
            metrics.reconcile_errors.fetch_add(1, Ordering::Relaxed);
            warn!(error=%e, "reconcile cycle failed");
        }
        tokio::time::sleep(RECONCILE_INTERVAL).await;
    }
}

async fn reconcile_once(store: &EtcdMetaStore, default_port: u16, xtrace: Option<&xtrace_client::Client>, metrics: &SharedMetrics) -> anyhow::Result<()> {
    let now = now_ms();

    // 1. Load all placements
    let placement_kvs = store.list_prefix("/placements/").await?;
    let mut plans: Vec<PlacementPlan> = Vec::new();
    for (_, val, _) in &placement_kvs {
        if let Ok(plan) = serde_json::from_slice::<PlacementPlan>(val) {
            plans.push(plan);
        }
    }

    // Update placement gauge
    metrics.placements_total.store(plans.len() as u64, Ordering::Relaxed);

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

    // 3. Load all model requests (to know desired replica count — legacy path)
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

    // 3b. Also load deployments (new declarative path)
    let deployment_kvs = store.list_prefix("/deployments/").await?;
    let mut deployments: HashMap<String, ModelDeployment> = HashMap::new();
    for (_, val, _) in &deployment_kvs {
        if let Ok(dep) = serde_json::from_slice::<ModelDeployment>(val) {
            if dep.desired_state == DesiredState::Running {
                deployments.insert(dep.model_uid.clone(), dep);
            }
        }
    }

    // 4. Load stats from xtrace (for autoscaling decisions)
    let stats_by_model = fetch_stats_from_xtrace(xtrace).await;

    // 5. For each placement, reconcile
    for plan in &plans {
        // Check deployment first (new path), fallback to old model_request
        let (base_replicas, min_replicas, max_replicas) =
            if let Some(dep) = deployments.get(&plan.model_uid) {
                let base = dep.replicas.max(1);
                let min = dep.min_replicas.unwrap_or(1).max(1);
                let max = dep.max_replicas.unwrap_or(base);
                (base, min, max)
            } else if let Some(req) = requests.get(&plan.model_uid) {
                let base = req.request.replicas.max(1);
                let min = req.request.min_replicas.unwrap_or(1).max(1);
                let max = req.request.max_replicas.unwrap_or(base);
                (base, min, max)
            } else {
                (
                    plan.assignments.len() as u32,
                    1,
                    plan.assignments.len() as u32,
                )
            };

        // Compute desired replicas: start from base, adjust by load signals
        let current_replicas = plan.assignments.len() as u32;
        let desired_replicas = compute_desired_replicas(
            base_replicas,
            min_replicas,
            max_replicas,
            plan,
            stats_by_model.get(&plan.model_uid),
            now,
        );

        if desired_replicas > current_replicas {
            metrics.scale_up_total.fetch_add(1, Ordering::Relaxed);
        } else if desired_replicas < current_replicas {
            metrics.scale_down_total.fetch_add(1, Ordering::Relaxed);
        }

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
                        metrics.unhealthy_endpoints_total.fetch_add(1, Ordering::Relaxed);
                    } else {
                        healthy_assignments.push(assignment.clone());
                    }
                }
                None => {
                    // No endpoint registered yet — could be still starting.
                    // Only remove if the plan is old enough (keep a generous startup grace).
                    let plan_age = now.saturating_sub(plan.version);
                    if plan_age > STARTUP_GRACE_MS {
                        warn!(
                            model_uid=%plan.model_uid,
                            replica_id=assignment.replica_id,
                            plan_age_ms=plan_age,
                            "no endpoint registered after startup grace period, removing assignment"
                        );
                        stale_replica_ids.push(assignment.replica_id);
                        metrics.unhealthy_endpoints_total.fetch_add(1, Ordering::Relaxed);
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

            // Stats are now in xtrace, no etcd key to clean up.
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

            // Get scheduling parameters from deployment (new path) or request (legacy)
            if let Some(req) = requests.get(&plan.model_uid) {
                add_replacement_replicas_from_request(
                    store,
                    plan,
                    req,
                    deficit,
                    default_port,
                    &mut new_assignments,
                )
                .await;
            } else if deployments.contains_key(&plan.model_uid) {
                add_replacement_replicas_from_plan(
                    store,
                    plan,
                    deficit,
                    default_port,
                    &mut new_assignments,
                )
                .await;
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

/// Add replacement replicas using the original ModelRequest for scheduling parameters (legacy path).
async fn add_replacement_replicas_from_request(
    store: &EtcdMetaStore,
    plan: &PlacementPlan,
    req: &ModelRequest,
    deficit: u32,
    default_port: u16,
    new_assignments: &mut Vec<nebula_common::PlacementAssignment>,
) {
    let (mut used_ports, mut used_gpus) = list_used_resources(store).await.unwrap_or_default();

    for a in new_assignments.iter() {
        used_ports.insert(a.port);
        if let Some(indices) = a.effective_gpu_indices() {
            let entry = used_gpus.entry(a.node_id.clone()).or_default();
            for idx in indices {
                entry.insert(idx);
            }
        }
    }

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
                    engine_config_path: format!("/tmp/nebula/{}.yaml", plan.model_uid),
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
                    "added replacement assignment (from request)"
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

/// Add replacement replicas for deployment-managed plans.
/// Re-uses existing plan assignments' extra_args/engine_type/docker_image as template.
async fn add_replacement_replicas_from_plan(
    store: &EtcdMetaStore,
    plan: &PlacementPlan,
    deficit: u32,
    default_port: u16,
    new_assignments: &mut Vec<nebula_common::PlacementAssignment>,
) {
    let (mut used_ports, mut used_gpus) = list_used_resources(store).await.unwrap_or_default();

    for a in new_assignments.iter() {
        used_ports.insert(a.port);
        if let Some(indices) = a.effective_gpu_indices() {
            let entry = used_gpus.entry(a.node_id.clone()).or_default();
            for idx in indices {
                entry.insert(idx);
            }
        }
    }

    let max_existing_id = new_assignments
        .iter()
        .map(|a| a.replica_id)
        .max()
        .unwrap_or(0);

    // Use the first existing assignment as a template for extra_args/engine_type/docker_image
    let template = plan.assignments.first();
    let extra_args = template.and_then(|a| a.extra_args.clone());
    let engine_type = template.and_then(|a| a.engine_type.clone());
    let docker_image = template.and_then(|a| a.docker_image.clone());

    // Build a minimal ModelRequest-like struct for node selection.
    // We use the plan's existing config (from extra_args) to infer required resources.
    let dummy_req = ModelRequest {
        id: String::new(),
        request: nebula_common::ModelLoadRequest {
            model_name: plan.model_name.clone(),
            model_uid: plan.model_uid.clone(),
            replicas: 1,
            config: None,
            node_id: None,
            gpu_index: None,
            gpu_indices: None,
            engine_type: engine_type.clone(),
            docker_image: docker_image.clone(),
            min_replicas: None,
            max_replicas: None,
        },
        status: ModelRequestStatus::Scheduled,
        created_at_ms: 0,
    };

    for i in 0..deficit {
        let new_replica_id = max_existing_id + 1 + i;

        match select_node_and_gpus(store, &dummy_req, &used_gpus).await {
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
                    engine_config_path: format!("/tmp/nebula/{}.yaml", plan.model_uid),
                    port,
                    gpu_index,
                    gpu_indices: gpu_indices_field,
                    extra_args: extra_args.clone(),
                    engine_type: engine_type.clone(),
                    docker_image: docker_image.clone(),
                });

                info!(
                    model_uid=%plan.model_uid,
                    replica_id=new_replica_id,
                    "added replacement assignment (from deployment)"
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

/// Fetch latest engine stats from xtrace, grouped by model_uid.
async fn fetch_stats_from_xtrace(
    xtrace: Option<&xtrace_client::Client>,
) -> HashMap<String, Vec<EndpointStats>> {
    let mut stats_by_model: HashMap<String, Vec<EndpointStats>> = HashMap::new();

    let client = match xtrace {
        Some(c) => c,
        None => return stats_by_model,
    };

    let now = chrono::Utc::now();
    let from = now - chrono::Duration::seconds(120);

    // Collect pending_requests and kv_cache_usage per (model_uid, replica_id)
    let mut pending_map: HashMap<(String, u32), u64> = HashMap::new();
    let mut kv_usage_map: HashMap<(String, u32), f64> = HashMap::new();

    for (metric_name, is_pending) in [("pending_requests", true), ("kv_cache_usage", false)] {
        let q = xtrace_client::MetricsQueryParams {
            name: metric_name.to_string(),
            from: Some(from),
            to: Some(now),
            step: Some("120s".to_string()),
            agg: Some("last".to_string()),
            ..Default::default()
        };

        match client.query_metrics(&q).await {
            Ok(resp) => {
                for series in &resp.data {
                    let model_uid = series.labels.get("model_uid")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let replica_id: u32 = series.labels.get("replica_id")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);

                    if model_uid.is_empty() {
                        continue;
                    }

                    if let Some(last) = series.values.last() {
                        let key = (model_uid, replica_id);
                        if is_pending {
                            pending_map.insert(key, last.value as u64);
                        } else {
                            kv_usage_map.insert(key, last.value);
                        }
                    }
                }
            }
            Err(e) => {
                warn!(metric=%metric_name, error=%e, "failed to query xtrace metrics for reconcile");
            }
        }
    }

    // Merge into EndpointStats
    let mut all_keys: std::collections::HashSet<(String, u32)> = std::collections::HashSet::new();
    all_keys.extend(pending_map.keys().cloned());
    all_keys.extend(kv_usage_map.keys().cloned());

    let now_ms = now.timestamp_millis() as u64;
    for key in all_keys {
        let pending = pending_map.get(&key).copied().unwrap_or(0);
        let kv_usage = kv_usage_map.get(&key).copied();

        const VIRTUAL_TOTAL: u64 = 1_000_000;
        let (kv_used, kv_free) = match kv_usage {
            Some(usage) => {
                let used = (usage * VIRTUAL_TOTAL as f64) as u64;
                (Some(used), Some(VIRTUAL_TOTAL - used))
            }
            None => (None, None),
        };

        let stats = EndpointStats {
            model_uid: key.0.clone(),
            replica_id: key.1,
            last_updated_ms: now_ms,
            pending_requests: pending,
            prefix_cache_hit_rate: None,
            prompt_cache_hit_rate: None,
            kv_cache_used_bytes: kv_used,
            kv_cache_free_bytes: kv_free,
        };

        stats_by_model
            .entry(key.0)
            .or_default()
            .push(stats);
    }

    stats_by_model
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
