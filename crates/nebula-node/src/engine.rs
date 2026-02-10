use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use tokio::fs;
use tokio::process::{Child, Command};
use tracing::info;

use nebula_common::PlacementAssignment;

use crate::args::Args;

async fn wait_engine_ready(base_url: &str, timeout: Duration) -> anyhow::Result<String> {
    let http = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(5))
        .build()?;

    let start = tokio::time::Instant::now();
    let health_url = format!("{}/health", base_url.trim_end_matches('/'));
    let url = format!("{}/v1/models", base_url.trim_end_matches('/'));
    loop {
        if start.elapsed() > timeout {
            anyhow::bail!("engine not ready within timeout");
        }

        if let Ok(resp) = http.get(&health_url).send().await {
            if resp.status().is_success() {
                return Ok("healthy".to_string());
            }
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

async fn parse_yaml_defaults(path: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    match fs::read_to_string(path).await {
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

pub async fn write_engine_env(path: &str, base_url: &str, model: &str) -> anyhow::Result<()> {
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

pub async fn start_vllm(
    args: &Args,
    assignment: &PlacementAssignment,
) -> anyhow::Result<(Child, String, String)> {
    let cfg = parse_yaml_defaults(&assignment.engine_config_path).await;
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
    if let Some(gpu_index) = assignment.gpu_index {
        cmd.env("CUDA_VISIBLE_DEVICES", gpu_index.to_string());
    }
    info!("Setting VLLM_USE_MODELSCOPE=True for vLLM process");
    cmd.current_dir(&args.vllm_cwd);
    cmd.arg("serve")
        .arg("--config")
        .arg(&assignment.engine_config_path)
        .arg("--host")
        .arg(&args.vllm_host)
        .arg("--port")
        .arg(assignment.port.to_string());

    if let Some(extra) = assignment.extra_args.as_ref() {
        for arg in extra {
            cmd.arg(arg);
        }
    }

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
