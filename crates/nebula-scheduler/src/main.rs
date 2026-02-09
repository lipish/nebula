use clap::Parser;
use futures_util::StreamExt;
use anyhow::Result;
use nebula_common::{
    ModelRequest, ModelRequestStatus, PlacementAssignment, PlacementPlan,
};
use nebula_meta::{EtcdMetaStore, MetaStore};
use std::time::Duration;
use tracing::{info, warn, error};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, default_value = "http://127.0.0.1:2379")]
    etcd_endpoint: String,

    #[arg(long, default_value = "node_gpu0")]
    default_node_id: String,

    #[arg(long, default_value_t = 10814)]
    default_port: u16,
}

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
                
                // 1. Find a free port
                let mut used_ports = std::collections::HashSet::new();
                if let Ok(kvs) = store.list_prefix("/placements/").await {
                    for (_, val, _) in kvs {
                        if let Ok(p) = serde_json::from_slice::<PlacementPlan>(&val) {
                            for a in p.assignments {
                                if a.node_id == args.default_node_id {
                                    used_ports.insert(a.port);
                                }
                            }
                        }
                    }
                }
                
                let mut port = args.default_port;
                while used_ports.contains(&port) {
                    port += 1;
                }

                // 2. Create Placement Plan
                // For MVP, always assign to default node
                let plan = PlacementPlan {
                    model_uid: req.request.model_uid.clone(),
                    version: now_ms(), // Simple versioning using timestamp
                    assignments: vec![PlacementAssignment {
                        replica_id: 0,
                        node_id: args.default_node_id.clone(),
                        engine_config_path: format!("/home/lipeng/nebula/{}.yaml", req.request.model_uid),
                        port,
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
