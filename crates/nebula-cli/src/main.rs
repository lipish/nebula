use anyhow::Result;
use clap::{Parser, Subcommand};
use nebula_common::{ClusterStatus, ModelConfig, ModelLoadRequest, ModelRequest, EndpointStatus};
use reqwest::Client;

#[derive(Debug, Parser)]
#[command(name = "nebula")]
#[command(about = "Nebula CLI for cluster management", long_about = None)]
struct Args {
    /// Gateway URL
    #[arg(long, env = "NEBULA_GATEWAY_URL", default_value = "http://127.0.0.1:8081")]
    gateway_url: String,

    /// Gateway API token (Authorization: Bearer)
    #[arg(long, env = "NEBULA_GATEWAY_TOKEN")]
    token: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Cluster management
    Cluster {
        #[command(subcommand)]
        subcommand: ClusterCommand,
    },
    /// Model management
    Model {
        #[command(subcommand)]
        subcommand: ModelCommand,
    },
    /// Show current auth identity
    Whoami,
    /// Fetch gateway metrics
    Metrics,
    /// Tail gateway logs
    Logs {
        /// Lines to return (default 200, max 2000)
        #[arg(long)]
        lines: Option<u32>,
    },
}

#[derive(Debug, Subcommand)]
enum ClusterCommand {
    /// Show cluster status
    Status,
}

#[derive(Debug, Subcommand)]
enum ModelCommand {
    /// List all model requests and their status
    List,
    /// Load a new model
    Load {
        /// User-facing model name
        #[arg(long)]
        name: String,
        /// Internal model UID
        #[arg(long)]
        uid: String,
        /// Number of replicas
        #[arg(long, default_value_t = 1)]
        replicas: u32,

        /// Required VRAM in MB (capacity check)
        #[arg(long)]
        required_vram_mb: Option<u64>,

        /// vLLM tensor parallel size
        #[arg(long)]
        tensor_parallel_size: Option<u32>,

        /// vLLM GPU memory utilization (0-1)
        #[arg(long)]
        gpu_memory_utilization: Option<f32>,

        /// vLLM max model length
        #[arg(long)]
        max_model_len: Option<u32>,

        /// LoRA modules (repeatable)
        #[arg(long, value_delimiter = ',')]
        lora: Option<Vec<String>>,
    },
    /// Unload a model by request ID
    Unload {
        /// Request ID to unload
        id: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let client = Client::new();
    let token = args.token;

    match args.command {
        Command::Cluster { subcommand } => match subcommand {
            ClusterCommand::Status => {
                let url = format!("{}/v1/admin/cluster/status", args.gateway_url.trim_end_matches('/'));
                let status: ClusterStatus = auth(client.get(&url), token.as_ref()).send().await?.json().await?;
                print_cluster_status(status);
            }
        },
        Command::Model { subcommand } => match subcommand {
            ModelCommand::List => {
                let url = format!("{}/v1/admin/models/requests", args.gateway_url.trim_end_matches('/'));
                let requests: Vec<ModelRequest> = auth(client.get(&url), token.as_ref()).send().await?.json().await?;
                print_model_requests(requests);
            }
            ModelCommand::Load {
                name,
                uid,
                replicas,
                required_vram_mb,
                tensor_parallel_size,
                gpu_memory_utilization,
                max_model_len,
                lora,
            } => {
                let url = format!("{}/v1/admin/models/load", args.gateway_url.trim_end_matches('/'));
                let config = build_config(
                    tensor_parallel_size,
                    gpu_memory_utilization,
                    max_model_len,
                    required_vram_mb,
                    lora,
                );
                let req = ModelLoadRequest {
                    model_name: name.clone(),
                    model_uid: uid,
                    replicas,
                    config,
                };
                let resp = auth(client.post(&url), token.as_ref()).json(&req).send().await?;
                if resp.status().is_success() {
                    println!("✓ Model load request submitted for '{}'", name);
                    println!("{}", resp.text().await?);
                } else {
                    eprintln!("✗ Failed to load model: {}", resp.text().await?);
                }
            }
            ModelCommand::Unload { id } => {
                let url = format!("{}/v1/admin/models/requests/{}", args.gateway_url.trim_end_matches('/'), id);
                let resp = auth(client.delete(&url), token.as_ref()).send().await?;
                if resp.status().is_success() {
                    println!("✓ Model unload request submitted for ID: {}", id);
                } else {
                    eprintln!("✗ Failed to unload model: {}", resp.text().await?);
                }
            }
        },
        Command::Whoami => {
            let url = format!("{}/v1/admin/whoami", args.gateway_url.trim_end_matches('/'));
            let resp = auth(client.get(&url), token.as_ref()).send().await?;
            println!("{}", resp.text().await?);
        }
        Command::Metrics => {
            let url = format!("{}/v1/admin/metrics", args.gateway_url.trim_end_matches('/'));
            let resp = auth(client.get(&url), token.as_ref()).send().await?;
            println!("{}", resp.text().await?);
        }
        Command::Logs { lines } => {
            let mut url = format!("{}/v1/admin/logs", args.gateway_url.trim_end_matches('/'));
            if let Some(n) = lines {
                url = format!("{}?lines={}", url, n);
            }
            let resp = auth(client.get(&url), token.as_ref()).send().await?;
            println!("{}", resp.text().await?);
        }
    }

    Ok(())
}

fn auth(builder: reqwest::RequestBuilder, token: Option<&String>) -> reqwest::RequestBuilder {
    match token {
        Some(t) => builder.bearer_auth(t),
        None => builder,
    }
}

fn build_config(
    tensor_parallel_size: Option<u32>,
    gpu_memory_utilization: Option<f32>,
    max_model_len: Option<u32>,
    required_vram_mb: Option<u64>,
    lora_modules: Option<Vec<String>>,
) -> Option<ModelConfig> {
    if tensor_parallel_size.is_none()
        && gpu_memory_utilization.is_none()
        && max_model_len.is_none()
        && required_vram_mb.is_none()
        && lora_modules.as_ref().map_or(true, |v| v.is_empty())
    {
        return None;
    }

    Some(ModelConfig {
        tensor_parallel_size,
        gpu_memory_utilization,
        max_model_len,
        required_vram_mb,
        lora_modules,
    })
}

fn print_cluster_status(status: ClusterStatus) {
    println!("\n=== Nebula Cluster Status ===");
    
    println!("\n[Nodes]");
    if status.nodes.is_empty() {
        println!("  (No nodes registered)");
    } else {
        println!("  {:<20} {:<20}", "Node ID", "Last Heartbeat");
        for node in status.nodes {
            println!("  {:<20} {:>10}ms ago", node.node_id, last_seen(node.last_heartbeat_ms));
        }
    }

    println!("\n[Endpoints]");
    if status.endpoints.is_empty() {
        println!("  (No active endpoints)");
    } else {
        println!("  {:<20} {:<10} {:<10} {:<30}", "Model UID", "Replica", "Status", "Base URL");
        for ep in status.endpoints {
            let status_str = match ep.status {
                EndpointStatus::Ready => "READY",
                EndpointStatus::Starting => "STARTING",
                EndpointStatus::Unhealthy => "UNHEALTHY",
                EndpointStatus::Draining => "DRAINING",
            };
            println!(
                "  {:<20} {:<10} {:<10} {:<30}",
                ep.model_uid,
                ep.replica_id,
                status_str,
                ep.base_url.as_deref().unwrap_or("N/A")
            );
        }
    }
    println!("");
}

fn print_model_requests(requests: Vec<ModelRequest>) {
    println!("\n=== Nebula Model Requests ===");
    if requests.is_empty() {
        println!("No active or past model requests found.");
        return;
    }
    
    println!("{:<40} {:<25} {:<15} {:<10}", "Request ID", "Model Name", "Status", "Replicas");
    println!("{:-<100}", "");
    for req in requests {
        let status_str = format!("{:?}", req.status);
        println!(
            "{:<40} {:<25} {:<15} {:<10}",
            req.id,
            req.request.model_name,
            status_str,
            req.request.replicas
        );
    }
    println!("");
}

fn last_seen(ts_ms: u64) -> u64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    now.saturating_sub(ts_ms)
}
