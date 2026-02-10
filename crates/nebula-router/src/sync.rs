use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;

use nebula_common::{EndpointInfo, PlacementPlan};
use nebula_meta::{EtcdMetaStore, MetaStore};

pub async fn endpoints_sync_loop(
    store: EtcdMetaStore,
    router: Arc<nebula_router::Router>,
) -> anyhow::Result<()> {
    loop {
        let mut snapshot: Vec<EndpointInfo> = Vec::new();
        match store.list_prefix("/endpoints/").await {
            Ok(items) => {
                for (_k, v, _rev) in items {
                    if let Ok(info) = serde_json::from_slice::<EndpointInfo>(&v) {
                        snapshot.push(info);
                    }
                }
                router.replace_all_endpoints(snapshot);
            }
            Err(e) => {
                tracing::warn!(error=%e, "failed to list endpoints, will retry");
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
        }

        let mut stream = match store.watch_prefix("/endpoints/", None).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(error=%e, "failed to watch endpoints, will retry");
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
        };

        while let Some(ev) = stream.next().await {
            if let Some(v) = ev.value {
                if let Ok(info) = serde_json::from_slice::<EndpointInfo>(&v) {
                    router.upsert_endpoint(info);
                }
            } else {
                let parts: Vec<&str> = ev.key.split('/').collect();
                if parts.len() >= 4 {
                    if let Ok(replica_id) = parts[3].parse::<u32>() {
                        router.remove_endpoint(parts[2], replica_id);
                    }
                }
            }
        }

        tracing::warn!("endpoints watch stream ended, reconnecting");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

pub async fn placement_sync_loop(
    store: EtcdMetaStore,
    model_uid: String,
    plan_version: Arc<AtomicU64>,
) -> anyhow::Result<()> {
    let key = format!("/placements/{model_uid}");
    loop {
        match store.get(&key).await {
            Ok(Some((bytes, _rev))) => {
                if let Ok(plan) = serde_json::from_slice::<PlacementPlan>(&bytes) {
                    if plan.model_uid == model_uid {
                        plan_version.store(plan.version, Ordering::Relaxed);
                    }
                }
            }
            Ok(None) => {
                plan_version.store(0, Ordering::Relaxed);
            }
            Err(e) => {
                tracing::warn!(error=%e, "failed to get placement, will retry");
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
        }

        let mut stream = match store.watch_prefix("/placements/", None).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(error=%e, "failed to watch placements, will retry");
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
        };
        while let Some(ev) = stream.next().await {
            let Some(v) = ev.value else {
                continue;
            };
            let Ok(plan) = serde_json::from_slice::<PlacementPlan>(&v) else {
                continue;
            };
            if plan.model_uid != model_uid {
                continue;
            }
            plan_version.store(plan.version, Ordering::Relaxed);
        }

        tracing::warn!("placements watch stream ended, reconnecting");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
