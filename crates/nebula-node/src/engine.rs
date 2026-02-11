use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use tokio::fs;
use tokio::process::{Child, Command};
use tokio::net::TcpListener;

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

async fn find_available_port(start_port: u16, max_tries: u16) -> anyhow::Result<u16> {
    let mut port = start_port;
    for _ in 0..max_tries {
        match TcpListener::bind(("0.0.0.0", port)).await {
            Ok(listener) => {
                drop(listener);
                return Ok(port);
            }
            Err(_) => {
                port = port.saturating_add(1);
            }
        }
    }
    anyhow::bail!(
        "no available port found in range [{}, {}]",
        start_port,
        start_port.saturating_add(max_tries)
    );
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
    model_uid: &str,
    model_name: &str,
) -> anyhow::Result<(Child, String, String)> {
    let cfg = parse_yaml_defaults(&assignment.engine_config_path).await;
    let model_tag = cfg
        .get("model")
        .cloned()
        .unwrap_or_else(|| model_name.to_string());
    let runner = cfg.get("runner").cloned();
    let served_model_name = cfg
        .get("served-model-name")
        .cloned()
        .or_else(|| cfg.get("served_model_name").cloned());
    let cfg_gpu_memory_utilization: Option<f32> = cfg
        .get("gpu-memory-utilization")
        .or_else(|| cfg.get("gpu_memory_utilization"))
        .and_then(|s| s.parse::<f32>().ok());

    let selected_port = find_available_port(assignment.port, 64).await?;
    if selected_port != assignment.port {
        tracing::warn!(
            requested_port = assignment.port,
            selected_port,
            "requested port is busy, using another port"
        );
    }
    let base_url = format!("http://127.0.0.1:{}", selected_port);
    let timeout = Duration::from_secs(args.ready_timeout_secs);

    tracing::info!(
        node_id=%args.node_id,
        replica_id=assignment.replica_id,
        port=selected_port,
        config=%assignment.engine_config_path,
        model=%model_tag,
        "starting vllm engine"
    );

    // Collect common vLLM args
    let mut vllm_args: Vec<String> = Vec::new();
    if let Some(extra) = assignment.extra_args.as_ref() {
        vllm_args.extend(extra.clone());
    }
    if let Some(runner) = runner.as_deref() {
        vllm_args.push("--runner".into());
        vllm_args.push(runner.into());
    }
    if let Some(name) = served_model_name.as_deref() {
        vllm_args.push("--served-model-name".into());
        vllm_args.push(name.into());
    }
    // Determine tensor parallel size: from assignment gpu_indices, or from args
    let effective_indices = assignment.effective_gpu_indices();
    let tp_size = if let Some(ref indices) = effective_indices {
        if indices.len() > 1 { Some(indices.len() as u32) } else { args.vllm_tensor_parallel_size }
    } else {
        args.vllm_tensor_parallel_size
    };
    if let Some(tp) = tp_size {
        vllm_args.push("--tensor-parallel-size".into());
        vllm_args.push(tp.to_string());
    }
    let gpu_memory_utilization = args
        .vllm_gpu_memory_utilization
        .or(cfg_gpu_memory_utilization);
    if let Some(v) = gpu_memory_utilization {
        vllm_args.push("--gpu-memory-utilization".into());
        vllm_args.push(v.to_string());
    }
    if let Some(v) = args.vllm_max_num_batched_tokens {
        vllm_args.push("--max-num-batched-tokens".into());
        vllm_args.push(v.to_string());
    }
    if let Some(v) = args.vllm_max_num_seqs {
        vllm_args.push("--max-num-seqs".into());
        vllm_args.push(v.to_string());
    }
    if let Some(v) = args.vllm_swap_space {
        vllm_args.push("--swap-space".into());
        vllm_args.push(v.to_string());
    }

    let mut cmd;
    if let Some(image) = args.vllm_docker_image.as_deref() {
        // Docker mode
        let container_name = format!("nebula-{}-{}", model_uid, assignment.replica_id);
        // Stop & remove any previous container with the same name
        let _ = Command::new("docker")
            .args(["rm", "-f", &container_name])
            .output()
            .await;

        let gpu_device = if let Some(ref indices) = effective_indices {
            let devs: Vec<String> = indices.iter().map(|i| i.to_string()).collect();
            format!("\"device={}\"", devs.join(","))
        } else {
            "all".to_string()
        };

        // Remap model path: if model_tag starts with vllm_model_dir, replace with /model
        let container_model = if model_tag.starts_with(&args.vllm_model_dir) {
            model_tag.replacen(&args.vllm_model_dir, "/model", 1)
        } else {
            model_tag.clone()
        };

        tracing::info!(image=%image, container=%container_name, gpu=%gpu_device, model=%container_model, "launching vLLM via docker");

        cmd = Command::new("docker");
        cmd.arg("run")
            .arg("--rm")
            .arg("--name").arg(&container_name)
            .arg("--gpus").arg(&gpu_device)
            .arg("-p").arg(format!("{}:{}", selected_port, selected_port))
            .arg("-v").arg(format!("{}:/model", args.vllm_model_dir));

        if args.vllm_use_modelscope {
            cmd.arg("-e").arg("VLLM_USE_MODELSCOPE=True");
        }
        if let Some(ep) = args.vllm_hf_endpoint.as_deref() {
            cmd.arg("-e").arg(format!("HF_ENDPOINT={ep}"));
        }

        cmd.arg("-e").arg("HF_HOME=/model/.cache/huggingface");
        cmd.arg("-e").arg("TRANSFORMERS_CACHE=/model/.cache/huggingface");
        cmd.arg("-e").arg("XDG_CACHE_HOME=/model/.cache");

        cmd.arg(image);

        cmd.arg("--model").arg(&container_model)
            .arg("--host").arg("0.0.0.0")
            .arg("--port").arg(selected_port.to_string());

        for a in &vllm_args {
            cmd.arg(a);
        }
    } else {
        // Local binary mode
        tracing::info!("Using vllm binary: {}", args.vllm_bin);
        cmd = Command::new(&args.vllm_bin);
        if args.vllm_use_modelscope {
            cmd.env("VLLM_USE_MODELSCOPE", "True");
        }
        if let Some(ep) = args.vllm_hf_endpoint.as_deref() {
            cmd.env("HF_ENDPOINT", ep);
        }
        if let Some(ref indices) = effective_indices {
            let devs: Vec<String> = indices.iter().map(|i| i.to_string()).collect();
            cmd.env("CUDA_VISIBLE_DEVICES", devs.join(","));
        }
        cmd.current_dir(&args.vllm_cwd);
        cmd.arg("serve")
            .arg(&model_tag)
            .arg("--host").arg(&args.vllm_host)
            .arg("--port").arg(selected_port.to_string());

        for a in &vllm_args {
            cmd.arg(a);
        }
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
