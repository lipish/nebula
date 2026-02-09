use clap::Parser;
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;
use tokio::fs;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use futures_util::StreamExt;
use nebula_common::{
    EndpointInfo, EndpointKind, EndpointStatus, NodeStatus, PlacementAssignment, PlacementPlan,
};
use nebula_meta::{EtcdMetaStore, MetaStore};

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, default_value = "node_1")]
    node_id: String,


    #[arg(long, default_value = "http://127.0.0.1:2379")]
    etcd_endpoint: String,

    #[arg(long, default_value = "/home/ai/miniconda3/envs/Lvllm/bin/vllm")]
    vllm_bin: String,

    #[arg(long, default_value = "Lvllm/m21.yaml")]
    vllm_config: String,

    #[arg(long, default_value = "/home/ai")]
    vllm_cwd: String,

    #[arg(long, default_value_t = 10814)]
    vllm_port: u16,

    #[arg(long, default_value = "0.0.0.0")]
    vllm_host: String,

    #[arg(long, default_value = "/tmp/nebula/engine.env")]
    engine_env_path: String,

    #[arg(long, default_value_t = 10_000)]
    heartbeat_ttl_ms: u64,

    #[arg(long, default_value_t = 3_000)]
    heartbeat_interval_ms: u64,

    #[arg(long, default_value_t = 180)]
    ready_timeout_secs: u64,

    #[arg(long)]
    vllm_gpu_memory_utilization: Option<f32>,

    #[arg(long)]
    vllm_max_model_len: Option<u32>,

    #[arg(long)]
    vllm_swap_space: Option<u32>,

    #[arg(long)]
    vllm_max_num_batched_tokens: Option<u32>,

    #[arg(long)]
    vllm_max_num_seqs: Option<u32>,

    #[arg(long)]
    vllm_tensor_parallel_size: Option<u32>,
}

async fn wait_engine_ready(base_url: &str, timeout: Duration) -> anyhow::Result<String> {
    let http = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(5))
        .build()?;

    let start = tokio::time::Instant::now();
    let url = format!("{}/v1/models", base_url.trim_end_matches('/'));
    loop {
        if start.elapsed() > timeout {
            anyhow::bail!("engine not ready within timeout");
        }

        match http.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                let v: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                let model_id = v
                    .get("data")
                    .and_then(|d| d.get(0))
                    .and_then(|m| m.get("id"))
                    .and_then(|id| id.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                if !model_id.is_empty() {
                    return Ok(model_id);
                }
            }
            _ => {}
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}
fn parse_yaml_defaults(path: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    match std::fs::read_to_string(path) {
        Ok(content) => {
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some((k, v)) = line.split_once(':') {
                    let k = k.trim();
                    let v = v.trim().trim_matches('"').trim_matches('\'');
                    if !k.is_empty() && !v.is_empty() {
                        out.insert(k.to_string(), v.to_string());
                    }
                }
            }
        }
        Err(e) => {
            tracing::error!("failed to read config file {}: {}", path, e);
        }
    }
    out
}

async fn write_engine_env(path: &str, base_url: &str, model: &str) -> anyhow::Result<()> {
    if let Some(parent) = Path::new(path).parent() {
        fs::create_dir_all(parent).await?;
    }
    let content = format!(
        "NEBULA_ENGINE_BASE_URL={}\nNEBULA_ENGINE_MODEL={}\n",
        base_url, model
    );
    fs::write(path, content).await?;
    Ok(())
}

fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

async fn start_vllm(
    args: &Args,
    assignment: &PlacementAssignment,
) -> anyhow::Result<(Child, String, String)> {
    let cfg = parse_yaml_defaults(&assignment.engine_config_path);
    let model_tag = cfg
        .get("model")
        .cloned()
        .unwrap_or_else(|| "unknown".to_string());
    let runner = cfg.get("runner").cloned();
    let served_model_name = cfg
        .get("served-model-name")
        .cloned()
        .or_else(|| cfg.get("served_model_name").cloned());
    let cfg_gpu_memory_utilization: Option<f32> = cfg
        .get("gpu-memory-utilization")
        .or_else(|| cfg.get("gpu_memory_utilization"))
        .and_then(|s| s.parse::<f32>().ok());

    let base_url = format!("http://127.0.0.1:{}", assignment.port);
    let timeout = Duration::from_secs(args.ready_timeout_secs);

    tracing::info!(
        node_id=%args.node_id,
        replica_id=assignment.replica_id,
        port=assignment.port,
        config=%assignment.engine_config_path,
        model=%model_tag,
        "starting vllm engine"
    );

    info!("Using vllm binary: {}", args.vllm_bin);
    let mut cmd = Command::new(&args.vllm_bin);
    cmd.env("VLLM_USE_MODELSCOPE", "True");
    info!("Setting VLLM_USE_MODELSCOPE=True for vLLM process");
    cmd.current_dir(&args.vllm_cwd);
    cmd.arg("serve")
        .arg("--config")
        .arg(&assignment.engine_config_path)
        .arg("--host")
        .arg(&args.vllm_host)
        .arg("--port")
        .arg(assignment.port.to_string());

    if let Some(runner) = runner.as_deref() {
        cmd.arg("--runner").arg(runner);
    }

    if let Some(name) = served_model_name.as_deref() {
        cmd.arg("--served-model-name").arg(name);
    }

    if let Some(tp) = args.vllm_tensor_parallel_size {
        cmd.arg("--tensor-parallel-size").arg(tp.to_string());
    }
    let gpu_memory_utilization = args
        .vllm_gpu_memory_utilization
        .or(cfg_gpu_memory_utilization);
    if let Some(v) = gpu_memory_utilization {
        cmd.arg("--gpu-memory-utilization").arg(v.to_string());
    }
    if let Some(v) = args.vllm_max_model_len {
        cmd.arg("--max-model-len").arg(v.to_string());
    }
    if let Some(v) = args.vllm_max_num_batched_tokens {
        cmd.arg("--max-num-batched-tokens").arg(v.to_string());
    }
    if let Some(v) = args.vllm_max_num_seqs {
        cmd.arg("--max-num-seqs").arg(v.to_string());
    }
    if let Some(v) = args.vllm_swap_space {
        cmd.arg("--swap-space").arg(v.to_string());
    }

    let mut child = cmd.spawn()?;

    let ready = tokio::select! {
        r = wait_engine_ready(&base_url, timeout) => r,
        status = child.wait() => {
            let status = status?;
            anyhow::bail!("vllm exited early: {status}");
        }
    }?;

    Ok((child, base_url, ready))
}

async fn stop_child(child: &mut Child) {
    let _ = child.kill().await;
}

async fn register_endpoint(
    store: &EtcdMetaStore,
    info: &EndpointInfo,
    ttl_ms: u64,
) -> anyhow::Result<()> {
    let key = format!("/endpoints/{}/{}", info.model_uid, info.replica_id);
    let bytes = serde_json::to_vec(info)?;
    let _ = store.put(&key, bytes, Some(ttl_ms)).await?;
    Ok(())
}

async fn delete_endpoint(store: &EtcdMetaStore, model_uid: &str, replica_id: u32) -> anyhow::Result<()> {
    let key = format!("/endpoints/{}/{}", model_uid, replica_id);
    let _ = store.delete(&key).await?;
    Ok(())
}


async fn heartbeat_loop(
    store: EtcdMetaStore,
    node_id: String,
    ttl_ms: u64,
    interval_ms: u64,
    endpoint: std::sync::Arc<Mutex<HashMap<String, EndpointInfo>>>,
) {
    let key = format!("/nodes/{}/status", node_id);
    loop {
        let status = NodeStatus {
            node_id: node_id.clone(),
            last_heartbeat_ms: now_ms(),
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
        // Clean up expired or stale endpoints? 
        // For now, just heartbeat what we have.
        for info in guard.values_mut() {
            info.last_heartbeat_ms = now_ms();
            if let Err(e) = register_endpoint(&store, info, ttl_ms).await {
                tracing::warn!(error=%e, "failed to refresh endpoint");
            }
        }

        tokio::time::sleep(Duration::from_millis(interval_ms)).await;
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args = Args::parse();
    println!("DEBUG: nebula-node process started! node_id={}", args.node_id);
    tracing::info!(node_id=%args.node_id, "nebula-node starting...");

    let store = EtcdMetaStore::connect(&vec![args.etcd_endpoint.clone()]).await?;

    let endpoint_state: std::sync::Arc<Mutex<HashMap<String, EndpointInfo>>> =
        std::sync::Arc::new(Mutex::new(HashMap::new()));

    tokio::spawn(heartbeat_loop(
        store.clone(),
        args.node_id.clone(),
        args.heartbeat_ttl_ms,
        args.heartbeat_interval_ms,
        endpoint_state.clone(),
    ));

    // local running state
    let mut running: HashMap<String, RunningModel> = HashMap::new();

    // 1. List existing placements to find if any are assigned to us
    let prefix = "/placements/";
    let mut start_rev = 0;
    
    if let Ok(kvs) = store.list_prefix(prefix).await {
        for (key, val, rev) in kvs {
             if rev > start_rev { start_rev = rev; }
             
             if let Ok(plan) = serde_json::from_slice::<PlacementPlan>(&val) {
                 let assigned = plan.assignments.iter().any(|a| a.node_id == args.node_id);
                 if assigned {
                     tracing::info!(model=%plan.model_uid, "found existing assignment");
                     let mid = plan.model_uid.clone();
                     let _ = reconcile_model(
                        &store,
                        &args,
                        &mut running,
                        &endpoint_state,
                        &mid,
                        Some(plan),
                     ).await;
                 }
             }
        }
    }

    loop {
        tracing::info!("watching placements from rev {}", start_rev);
        let mut watch = match store.watch_prefix(prefix, Some(start_rev)).await {
            Ok(w) => w,
            Err(e) => {
                tracing::warn!(error=%e, "failed to watch placements, will retry");
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        while let Some(ev) = watch.next().await {
            if ev.revision > start_rev {
                start_rev = ev.revision;
            }

            let plan: Option<PlacementPlan> = if let Some(val) = ev.value {
                 serde_json::from_slice(&val).ok()
            } else {
                 None
            };
            
            match plan {
                Some(p) => {
                    let assigned = p.assignments.iter().any(|a| a.node_id == args.node_id);
                    let mid = p.model_uid.clone();
                    let _ = reconcile_model(&store, &args, &mut running, &endpoint_state, &mid, Some(p)).await;
                },
                None => {
                    // Plan deleted. Get model_uid from key if possible
                    let key = ev.key;
                    let model_uid = key.strip_prefix(prefix).unwrap_or(&key);
                    tracing::info!(model=%model_uid, "placement deleted event");
                    // We construct a dummy plan or just pass None to a helper that takes model_uid?
                    // reconcile_from_plan expects Option<PlacementPlan>.
                    // But if plan is None, it stops EVERYTHING if we don't know the model map logic inside?
                    // Actually reconcile_from_plan logic needs to be robust for "Plan for Model X".
                    // The current signature takes `plan: Option<PlacementPlan>`. 
                    // If None, it previously stopped THE running model.
                    // now we need to know WHICH model to stop.
                    
                    // We need to change the signature of reconcile_from_plan to explicitly handle model_uid.
                    // But wait, if plan is None, we don't have model_uid from the plan.
                    // We key off `model_uid`.
                    
                    // Let's rely on the parsing logic:
                    let model_uid = model_uid.to_string();
                    if running.contains_key(&model_uid) {
                         tracing::info!(%model_uid, "stopping model due to deletion");
                         reconcile_model(&store, &args, &mut running, &endpoint_state, &model_uid, None).await?;
                    }
                }
            }
        }
        
        tracing::warn!("watch stream ended, reconnecting");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

// Helper struct for internal state
struct RunningModel {
    model_uid: String,
    replica_id: u32,
    plan_version: u64,
    child: Child,
}


async fn reconcile_model(
    store: &EtcdMetaStore,
    args: &Args,
    running: &mut HashMap<String, RunningModel>,
    endpoint_state: &std::sync::Arc<Mutex<HashMap<String, EndpointInfo>>>,
    model_uid: &str,
    plan: Option<PlacementPlan>,
) -> anyhow::Result<()> {
    let plan = match plan {
        Some(p) => p,
        None => {
            // Stop logic
            if let Some(mut rm) = running.remove(model_uid) {
                tracing::info!(%model_uid, "stopping engine");
                stop_child(&mut rm.child).await;
                let _ = delete_endpoint(store, &rm.model_uid, rm.replica_id).await;
                endpoint_state.lock().await.remove(model_uid);
            }
            return Ok(());
        }
    };

    let desired = plan
        .assignments
        .iter()
        .find(|a| a.node_id == args.node_id);

    let Some(assignment) = desired else {
        // Not assigned to us, stop if running
        if let Some(mut rm) = running.remove(model_uid) {
             tracing::info!(%model_uid, "no longer assigned, stopping engine");
             stop_child(&mut rm.child).await;
             let _ = delete_endpoint(store, &rm.model_uid, rm.replica_id).await;
             endpoint_state.lock().await.remove(model_uid);
        }
        return Ok(());
    };

    // Check if needs restart
    let needs_restart = match running.get(model_uid) {
        Some(rm) => {
            rm.replica_id != assignment.replica_id || rm.plan_version != plan.version
        }
        None => true,
    };

    if !needs_restart {
        return Ok(());
    }

    // Stop existing if any (restart logic)
    if let Some(mut rm) = running.remove(model_uid) {
         tracing::info!(%model_uid, "restarting engine due to placement update");
         stop_child(&mut rm.child).await;
         let _ = delete_endpoint(store, &rm.model_uid, rm.replica_id).await;
         endpoint_state.lock().await.remove(model_uid);
    }

    let (child, base_url, engine_model) = start_vllm(args, assignment).await?;
    // Note: this overwrites the env file. For multi-model, we might need separate env files or just log it.
    // For now, let's keep it but it might be racey or last-write-wins.
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

    endpoint_state.lock().await.insert(model_uid.to_string(), info);
    running.insert(model_uid.to_string(), RunningModel {
        model_uid: plan.model_uid,
        replica_id: assignment.replica_id,
        plan_version: plan.version,
        child,
    });
    
    Ok(())
}
