mod args;
mod planner;
mod util;

use anyhow::Result;
use clap::Parser;
use futures_util::StreamExt;
use std::time::Duration;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

use nebula_common::{ModelRequest, ModelRequestStatus};
use nebula_meta::{EtcdMetaStore, MetaStore};

use crate::args::Args;
use crate::planner::{build_extra_args, build_plan, list_used_resources, select_node_and_gpus};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args = Args::parse();
    info!("nebula-scheduler starting...");

    let store = EtcdMetaStore::connect(std::slice::from_ref(&args.etcd_endpoint)).await?;
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
                info!(
                    "processing pending request: {} (model={})",
                    req.id, req.request.model_name
                );

                let (used_ports, used_gpus) = match list_used_resources(&store).await {
                    Ok(v) => v,
                    Err(e) => {
                        warn!("failed to list placements: {}", e);
                        (
                            std::collections::HashSet::new(),
                            std::collections::HashMap::new(),
                        )
                    }
                };

                let (node_id, gpu_indices) = select_node_and_gpus(&store, &req, &used_gpus)
                    .await
                    .unwrap_or((args.default_node_id.clone(), vec![]));

                let mut port = args.default_port;
                while used_ports.contains(&port) {
                    port += 1;
                }

                let extra_args = build_extra_args(&req);
                let plan = build_plan(&req, node_id, port, gpu_indices, extra_args);

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
                info!(
                    "processing unloading request: {} (model={})",
                    req.id, req.request.model_name
                );

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
