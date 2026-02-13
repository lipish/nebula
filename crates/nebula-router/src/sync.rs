use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;

use nebula_common::{EndpointInfo, EndpointStats, PlacementPlan};
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

pub async fn stats_sync_loop(
    xtrace: xtrace_client::Client,
    router: Arc<nebula_router::Router>,
) -> anyhow::Result<()> {
    use std::collections::HashMap;

    const POLL_INTERVAL: Duration = Duration::from_secs(10);

    loop {
        let now = chrono::Utc::now();
        let from = now - chrono::Duration::seconds(60);

        // Query latest pending_requests and kv_cache_usage from xtrace
        let mut pending_map: HashMap<(String, u32), u64> = HashMap::new();
        let mut kv_usage_map: HashMap<(String, u32), f64> = HashMap::new();

        for (metric_name, target_map_is_pending) in
            [("pending_requests", true), ("kv_cache_usage", false)]
        {
            let q = xtrace_client::MetricsQueryParams {
                name: metric_name.to_string(),
                from: Some(from),
                to: Some(now),
                step: Some("60s".to_string()),
                agg: Some("last".to_string()),
                ..Default::default()
            };

            match xtrace.query_metrics(&q).await {
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
                            if target_map_is_pending {
                                pending_map.insert(key, last.value as u64);
                            } else {
                                kv_usage_map.insert(key, last.value);
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(metric=%metric_name, error=%e, "failed to query xtrace metrics");
                }
            }
        }

        // Merge into EndpointStats and upsert
        let mut all_keys: std::collections::HashSet<(String, u32)> = std::collections::HashSet::new();
        all_keys.extend(pending_map.keys().cloned());
        all_keys.extend(kv_usage_map.keys().cloned());

        let now_ms = now.timestamp_millis() as u64;
        for key in all_keys {
            let pending = pending_map.get(&key).copied().unwrap_or(0);
            let kv_usage = kv_usage_map.get(&key).copied();

            // Convert kv_cache_usage ratio (0..1) to synthetic used/free bytes
            // so downstream Router logic can compute the ratio the same way.
            const VIRTUAL_TOTAL: u64 = 1_000_000;
            let (kv_used, kv_free) = match kv_usage {
                Some(usage) => {
                    let used = (usage * VIRTUAL_TOTAL as f64) as u64;
                    (Some(used), Some(VIRTUAL_TOTAL - used))
                }
                None => (None, None),
            };

            let stats = EndpointStats {
                model_uid: key.0,
                replica_id: key.1,
                last_updated_ms: now_ms,
                pending_requests: pending,
                prefix_cache_hit_rate: None,
                prompt_cache_hit_rate: None,
                kv_cache_used_bytes: kv_used,
                kv_cache_free_bytes: kv_free,
            };

            router.upsert_stats(stats);
        }

        tokio::time::sleep(POLL_INTERVAL).await;
    }
}
