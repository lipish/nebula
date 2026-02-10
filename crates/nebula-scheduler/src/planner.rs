use std::collections::{HashMap, HashSet};

use nebula_common::{ModelRequest, NodeStatus, PlacementAssignment, PlacementPlan};
use nebula_meta::{EtcdMetaStore, MetaStore};

use crate::util::now_ms;

const NODE_STALE_MS: u64 = 10_000;

pub async fn list_used_resources(
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

pub async fn select_node_and_gpu(
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
    let mut best: Option<(String, u32, u64)> = None;

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

    for node in &nodes {
        if now.saturating_sub(node.last_heartbeat_ms) > NODE_STALE_MS {
            continue;
        }
        return Ok((node.node_id.clone(), None));
    }

    anyhow::bail!("no healthy nodes available")
}

pub fn build_extra_args(req: &ModelRequest) -> Option<Vec<String>> {
    let cfg = req.request.config.as_ref()?;

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

pub fn build_plan(
    req: &ModelRequest,
    node_id: String,
    port: u16,
    gpu_index: Option<u32>,
    extra_args: Option<Vec<String>>,
) -> PlacementPlan {
    PlacementPlan {
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
    }
}
