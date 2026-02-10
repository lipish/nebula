use clap::Parser;
use futures_util::StreamExt;
use anyhow::Result;
use nebula_common::{
    ModelRequest, ModelRequestStatus, NodeStatus, PlacementAssignment, PlacementPlan,
};
use nebula_meta::{EtcdMetaStore, MetaStore};
use std::time::Duration;
use tracing::{info, warn, error};
use tracing_subscriber::EnvFilter;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, default_value = "http://127.0.0.1:2379")]
    etcd_endpoint: String,

    #[arg(long, default_value = "node_gpu0")]
    default_node_id: String,

    #[arg(long, default_value_t = 10814)]
    default_port: u16,
}

const NODE_STALE_MS: u64 = 10_000;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args = Args::parse();
    info!("nebula-scheduler starting...");

    let store = EtcdMetaStore::connect(&vec![args.etcd_endpoint.clone()]).await?;
    info!("connected to etcd at {}", args.etcd_endpoint);

    // Watch for model requests
    let prefix = "/model_requests/";
    
    // In a real system, we'd list existing pending requests first.
    // For MVP, we'll just start watching or process list once.
    // Let's do a simple watch loop with reconnection logic.

    loop {
        info!("watching prefix: {}", prefix);
        let mut stream = match store.watch_prefix(prefix, None).await {
            Ok(s) => s,
            Err(e) => {
                error!("failed to watch prefix: {}, retrying in 5s", e);
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        while let Some(event) = stream.next().await {
            let Some(value) = event.value else { continue };
            
            let mut req: ModelRequest = match serde_json::from_slice(&value) {
                Ok(r) => r,
                Err(e) => {
                    warn!("failed to deserialize model request: {}", e);
                    continue;
                }
            };

            if req.status == ModelRequestStatus::Pending {
                info!("processing pending request: {} (model={})", req.id, req.request.model_name);

                let (used_ports, used_gpus) = match list_used_resources(&store).await {
                    Ok(v) => v,
                    Err(e) => {
                        warn!("failed to list placements: {}", e);
                        (HashSet::new(), HashMap::new())
                    }
                };

                let (node_id, gpu_index) = select_node_and_gpu(&store, &req, &used_gpus).await
                    .unwrap_or((args.default_node_id.clone(), None));

                let mut port = args.default_port;
                while used_ports.contains(&port) {
                    port += 1;
                }

                let extra_args = build_extra_args(&req);

                let plan = PlacementPlan {
                    model_uid: req.request.model_uid.clone(),
                    version: now_ms(),
                    assignments: vec![PlacementAssignment {
                        replica_id: 0,
                        node_id,
                        engine_config_path: format!("/home/lipeng/nebula/{}.yaml", req.request.model_uid),
                        port,
                        gpu_index,
                        extra_args,
                    }],
                };

                // 3. Write Placement
                let placement_key = format!("/placements/{}", plan.model_uid);
                let placement_val = serde_json::to_vec(&plan)?;
                
                if let Err(e) = store.put(&placement_key, placement_val, None).await {
                    error!("failed to write placement: {}", e);
                    continue;
                }
                info!("wrote placement to {}", placement_key);

                // 3. Update Request Status
                req.status = ModelRequestStatus::Scheduled;
                 if let Ok(updated_val) = serde_json::to_vec(&req) {
                    let req_key = format!("{}{}", prefix, req.id);
                    let _ = store.put(&req_key, updated_val, None).await;
                    info!("updated request {} status to Scheduled", req.id);
                 }
            } else if req.status == ModelRequestStatus::Unloading {
                info!("processing unloading request: {} (model={})", req.id, req.request.model_name);
                
                let placement_key = format!("/placements/{}", req.request.model_uid);
                if let Err(e) = store.delete(&placement_key).await {
                    warn!("failed to delete placement {}: {}", placement_key, e);
                } else {
                    info!("deleted placement {}", placement_key);
                }

                let req_key = format!("{}{}", prefix, req.id);
                if let Err(e) = store.delete(&req_key).await {
                    error!("failed to delete request key {}: {}", req_key, e);
                } else {
                    info!("successfully cleaned up request {}", req.id);
                }
            }
        }
        
        warn!("watch stream ended, reconnecting...");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

async fn list_used_resources(
    store: &EtcdMetaStore,
) -> anyhow::Result<(HashSet<u16>, HashMap<String, HashSet<u32>>)> {
    let mut used_ports = HashSet::new();
    let mut used_gpus: HashMap<String, HashSet<u32>> = HashMap::new();
    if let Ok(kvs) = store.list_prefix("/placements/").await {
        for (_, val, _) in kvs {
            if let Ok(p) = serde_json::from_slice::<PlacementPlan>(&val) {
                for a in p.assignments {
                    used_ports.insert(a.port);
                    if let Some(gpu_index) = a.gpu_index {
                        used_gpus
                            .entry(a.node_id.clone())
                            .or_default()
                            .insert(gpu_index);
                    }
                }
            }
        }
    }
    Ok((used_ports, used_gpus))
}

async fn select_node_and_gpu(
    store: &EtcdMetaStore,
    req: &ModelRequest,
    used_gpus: &HashMap<String, HashSet<u32>>,
) -> anyhow::Result<(String, Option<u32>)> {
    let required_vram_mb = req
        .request
        .config
        .as_ref()
        .and_then(|c| c.required_vram_mb)
        .unwrap_or(0);

    let mut nodes: Vec<NodeStatus> = Vec::new();
    if let Ok(kvs) = store.list_prefix("/nodes/").await {
        for (_, val, _) in kvs {
            if let Ok(status) = serde_json::from_slice::<NodeStatus>(&val) {
                nodes.push(status);
            }
        }
    }

    let now = now_ms();
    let mut best: Option<(String, u32, u64)> = None; // node_id, gpu_index, free_mb

    for node in &nodes {
        if now.saturating_sub(node.last_heartbeat_ms) > NODE_STALE_MS {
            continue;
        }

        let used = used_gpus.get(&node.node_id);
        for gpu in node.gpus.iter() {
            if let Some(used_set) = used {
                if used_set.contains(&gpu.index) {
                    continue;
                }
            }

            let free = gpu.memory_total_mb.saturating_sub(gpu.memory_used_mb);
            if free < required_vram_mb {
                continue;
            }

            match best {
                Some((_, _, best_free)) if free <= best_free => {}
                _ => {
                    best = Some((node.node_id.clone(), gpu.index, free));
                }
            }
        }
    }

    if let Some((node_id, gpu_index, _)) = best {
        return Ok((node_id, Some(gpu_index)));
    }

    // Fallback: pick any fresh node without GPU info
    for node in &nodes {
        if now.saturating_sub(node.last_heartbeat_ms) > NODE_STALE_MS {
            continue;
        }
        return Ok((node.node_id.clone(), None));
    }

    anyhow::bail!("no healthy nodes available")
}

fn build_extra_args(req: &ModelRequest) -> Option<Vec<String>> {
    let Some(cfg) = req.request.config.as_ref() else {
        return None;
    };

    let mut args = Vec::new();
    if let Some(mods) = cfg.lora_modules.as_ref() {
        if !mods.is_empty() {
            args.push("--enable-lora".to_string());
            args.push("--lora-modules".to_string());
            args.push(mods.join(","));
        }
    }

    if args.is_empty() {
        None
    } else {
        Some(args)
    }
}
