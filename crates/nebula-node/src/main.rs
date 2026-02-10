mod args;
mod engine;
mod gpu;
mod heartbeat;
mod reconcile;
mod util;

use clap::Parser;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing_subscriber::EnvFilter;

use futures_util::StreamExt;
use nebula_common::PlacementPlan;
use nebula_meta::{EtcdMetaStore, MetaStore};

use crate::args::Args;
use crate::heartbeat::heartbeat_loop;
use crate::reconcile::{reconcile_model, RunningModel};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args = Args::parse();
    println!(
        "DEBUG: nebula-node process started! node_id={}",
        args.node_id
    );
    tracing::info!(node_id=%args.node_id, "nebula-node starting...");

    let store = EtcdMetaStore::connect(std::slice::from_ref(&args.etcd_endpoint)).await?;

    let endpoint_state: Arc<Mutex<HashMap<String, nebula_common::EndpointInfo>>> =
        Arc::new(Mutex::new(HashMap::new()));

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
        for (_key, val, rev) in kvs {
            if rev > start_rev {
                start_rev = rev;
            }

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
                    )
                    .await;
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

            let plan: Option<PlacementPlan> =
                ev.value.and_then(|val| serde_json::from_slice(&val).ok());

            match plan {
                Some(p) => {
                    let mid = p.model_uid.clone();
                    let _ = reconcile_model(
                        &store,
                        &args,
                        &mut running,
                        &endpoint_state,
                        &mid,
                        Some(p),
                    )
                    .await;
                }
                None => {
                    let key = ev.key;
                    let model_uid = key.strip_prefix(prefix).unwrap_or(&key);
                    tracing::info!(model=%model_uid, "placement deleted event");
                    let model_uid = model_uid.to_string();
                    if running.contains_key(&model_uid) {
                        tracing::info!(%model_uid, "stopping model due to deletion");
                        reconcile_model(
                            &store,
                            &args,
                            &mut running,
                            &endpoint_state,
                            &model_uid,
                            None,
                        )
                        .await?;
                    }
                }
            }
        }

        tracing::warn!("watch stream ended, reconnecting");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
