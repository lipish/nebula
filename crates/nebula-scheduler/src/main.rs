mod args;
mod metrics;
mod planner;
mod reconcile;
mod util;

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use axum::routing::get;
use axum::Router;
use clap::Parser;
use futures_util::StreamExt;
use tracing::{error, info, warn};

use nebula_common::{ModelRequest, ModelRequestStatus};
use nebula_meta::{EtcdMetaStore, MetaStore};

use crate::args::Args;
use crate::metrics::{healthz_handler, metrics_handler, SharedMetrics};
use crate::planner::{build_plan_multi, list_used_resources};

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let _otel_guard = nebula_common::telemetry::init_tracing(
        "nebula-scheduler",
        args.xtrace_url.as_deref(),
        args.xtrace_token.as_deref(),
        &args.log_format,
    );
    info!("nebula-scheduler starting...");

    let store = EtcdMetaStore::connect(std::slice::from_ref(&args.etcd_endpoint)).await?;
    info!("connected to etcd at {}", args.etcd_endpoint);

    // Shared metrics for Prometheus exposition
    let shared_metrics = Arc::new(SharedMetrics::default());

    // Spawn metrics / health HTTP server
    let listen_addr = args.listen_addr.clone();
    let metrics_state = Arc::clone(&shared_metrics);
    tokio::spawn(async move {
        let app = Router::new()
            .route("/metrics", get(metrics_handler))
            .route("/healthz", get(healthz_handler))
            .with_state(metrics_state);

        let listener = match tokio::net::TcpListener::bind(&listen_addr).await {
            Ok(l) => l,
            Err(e) => {
                error!("failed to bind metrics server on {}: {}", listen_addr, e);
                return;
            }
        };
        info!("metrics server listening on {}", listen_addr);
        if let Err(e) = axum::serve(listener, app).await {
            error!("metrics server error: {}", e);
        }
    });

    // Create xtrace client for stats queries
    let xtrace = args.xtrace_url.as_deref().and_then(|url| {
        let token = args.xtrace_token.as_deref().unwrap_or("");
        match xtrace_client::Client::new(url, token) {
            Ok(c) => {
                info!("xtrace client created for stats queries (url={})", url);
                Some(c)
            }
            Err(e) => {
                warn!(error=%e, "failed to create xtrace client, autoscaling stats disabled");
                None
            }
        }
    });

    // Spawn reconcile loop (health self-healing)
    let store_for_reconcile = store.clone();
    let default_port_for_reconcile = args.default_port;
    let metrics_for_reconcile = Arc::clone(&shared_metrics);
    tokio::spawn(async move {
        reconcile::reconcile_loop(
            store_for_reconcile,
            default_port_for_reconcile,
            xtrace,
            metrics_for_reconcile,
        )
        .await;
    });

    // Watch for model requests
    let prefix = "/model_requests/";

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

                let plan = match build_plan_multi(
                    &store, &req, args.default_port, used_ports, used_gpus,
                ).await {
                    Ok(p) => p,
                    Err(e) => {
                        error!("failed to build placement plan: {}", e);
                        continue;
                    }
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
