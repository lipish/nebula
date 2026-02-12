use nebula_common::EndpointStats;

use crate::util::now_ms;

/// Scrape vLLM /metrics endpoint and parse into EndpointStats.
pub async fn scrape_engine_stats(
    http: &reqwest::Client,
    base_url: &str,
    model_uid: &str,
    replica_id: u32,
) -> Option<EndpointStats> {
    let url = format!("{}/metrics", base_url.trim_end_matches('/'));
    let text = match http.get(&url).send().await {
        Ok(resp) => match resp.text().await {
            Ok(t) => t,
            Err(e) => {
                tracing::debug!(error=%e, %base_url, "failed to read metrics body");
                return None;
            }
        },
        Err(e) => {
            tracing::debug!(error=%e, %base_url, "failed to scrape engine metrics");
            return None;
        }
    };

    let mut pending_requests: u64 = 0;
    let mut running_requests: u64 = 0;
    let mut kv_cache_usage: Option<f64> = None;
    let mut prefix_cache_hit_rate: Option<f64> = None;
    let mut prefix_cache_hits: Option<f64> = None;
    let mut prefix_cache_queries: Option<f64> = None;

    for line in text.lines() {
        if line.starts_with('#') {
            continue;
        }

        // vLLM metric formats (v0.11+):
        //   vllm:num_requests_waiting{...} 3
        //   vllm:num_requests_running{...} 1
        //   vllm:kv_cache_usage_perc{...} 0.45
        //   vllm:prefix_cache_hits_total{...} 100
        //   vllm:prefix_cache_queries_total{...} 200
        //
        // Older versions may use:
        //   vllm:gpu_cache_usage_perc{...} 0.45
        //   vllm:gpu_prefix_cache_hit_rate{...} 0.8
        //
        // Also handle underscore variants without colon:
        //   vllm_num_requests_waiting{...} 3

        if let Some(val) = extract_metric(line, "num_requests_waiting") {
            pending_requests = val as u64;
        } else if let Some(val) = extract_metric(line, "num_requests_running") {
            running_requests = val as u64;
        } else if let Some(val) = extract_metric(line, "kv_cache_usage_perc") {
            kv_cache_usage = Some(val);
        } else if kv_cache_usage.is_none() {
            // Fallback for older vLLM versions
            if let Some(val) = extract_metric(line, "gpu_cache_usage_perc") {
                kv_cache_usage = Some(val);
            }
        }

        // prefix cache: prefer direct hit_rate gauge, else compute from counters
        if let Some(val) = extract_metric(line, "gpu_prefix_cache_hit_rate") {
            prefix_cache_hit_rate = Some(val);
        } else if let Some(val) = extract_metric(line, "cpu_prefix_cache_hit_rate") {
            if prefix_cache_hit_rate.is_none() {
                prefix_cache_hit_rate = Some(val);
            }
        }
        if let Some(val) = extract_metric(line, "prefix_cache_hits_total") {
            prefix_cache_hits = Some(val);
        } else if let Some(val) = extract_metric(line, "prefix_cache_queries_total") {
            prefix_cache_queries = Some(val);
        }
    }

    // Compute prefix cache hit rate from counters if no direct gauge was found
    if prefix_cache_hit_rate.is_none() {
        if let (Some(hits), Some(queries)) = (prefix_cache_hits, prefix_cache_queries) {
            if queries > 0.0 {
                prefix_cache_hit_rate = Some(hits / queries);
            }
        }
    }

    // Convert kv_cache_usage percentage (0.0-1.0) to used/free in permille units.
    // (e.g., 0.45 → used=450, free=550, treating 1000 as full capacity).
    let (kv_cache_used, kv_cache_free) = match kv_cache_usage {
        Some(pct) => {
            let used = (pct * 1000.0) as u64;
            let free = 1000u64.saturating_sub(used);
            (Some(used), Some(free))
        }
        None => (None, None),
    };

    Some(EndpointStats {
        model_uid: model_uid.to_string(),
        replica_id,
        last_updated_ms: now_ms(),
        pending_requests: pending_requests + running_requests,
        prefix_cache_hit_rate,
        prompt_cache_hit_rate: None,
        kv_cache_used_bytes: kv_cache_used,
        kv_cache_free_bytes: kv_cache_free,
    })
}

/// Extract a numeric value from a Prometheus metric line.
/// Matches lines like:
///   vllm:metric_name{labels...} 123.45
///   vllm_metric_name{labels...} 123.45
///   vllm:metric_name 123.45
fn extract_metric(line: &str, metric_suffix: &str) -> Option<f64> {
    // Check if line contains the metric name
    let has_metric = line.contains(&format!(":{metric_suffix}"))
        || line.contains(&format!("_{metric_suffix}"));

    if !has_metric {
        return None;
    }

    // Value is the last whitespace-separated token
    let value_str = line.rsplit_once(|c: char| c.is_whitespace())?.1;
    value_str.parse::<f64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_metric() {
        assert_eq!(
            extract_metric("vllm:num_requests_waiting{model=\"m\"} 3", "num_requests_waiting"),
            Some(3.0)
        );
        assert_eq!(
            extract_metric("vllm:kv_cache_usage_perc{engine=\"0\"} 0.45", "kv_cache_usage_perc"),
            Some(0.45)
        );
        assert_eq!(
            extract_metric("vllm_gpu_cache_usage_perc{} 0.45", "gpu_cache_usage_perc"),
            Some(0.45)
        );
        assert_eq!(
            extract_metric("vllm:prefix_cache_hits_total{engine=\"0\"} 100.0", "prefix_cache_hits_total"),
            Some(100.0)
        );
        assert_eq!(
            extract_metric("vllm:prefix_cache_queries_total{engine=\"0\"} 200.0", "prefix_cache_queries_total"),
            Some(200.0)
        );
        assert_eq!(
            extract_metric("# HELP vllm:num_requests_waiting help text", "num_requests_waiting"),
            None, // comment lines are skipped before calling this
        );
        assert_eq!(
            extract_metric("unrelated_metric{} 1.0", "num_requests_waiting"),
            None,
        );
    }
}
