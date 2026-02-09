use anyhow::Result;
use clap::{Parser, Subcommand};
use nebula_common::{ClusterStatus, ModelLoadRequest, ModelRequest, EndpointStatus};
use reqwest::Client;

#[derive(Debug, Parser)]
#[command(name = "nebula")]
#[command(about = "Nebula CLI for cluster management", long_about = None)]
struct Args {
    /// Gateway URL
    #[arg(long, env = "NEBULA_GATEWAY_URL", default_value = "http://127.0.0.1:8081")]
    gateway_url: String,

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

    match args.command {
        Command::Cluster { subcommand } => match subcommand {
            ClusterCommand::Status => {
                let url = format!("{}/v1/admin/cluster/status", args.gateway_url.trim_end_matches('/'));
                let status: ClusterStatus = client.get(&url).send().await?.json().await?;
                print_cluster_status(status);
            }
        },
        Command::Model { subcommand } => match subcommand {
            ModelCommand::List => {
                let url = format!("{}/v1/admin/models/requests", args.gateway_url.trim_end_matches('/'));
                let requests: Vec<ModelRequest> = client.get(&url).send().await?.json().await?;
                print_model_requests(requests);
            }
            ModelCommand::Load { name, uid, replicas } => {
                let url = format!("{}/v1/admin/models/load", args.gateway_url.trim_end_matches('/'));
                let req = ModelLoadRequest {
                    model_name: name.clone(),
                    model_uid: uid,
                    replicas,
                    config: None,
                };
                let resp = client.post(&url).json(&req).send().await?;
                if resp.status().is_success() {
                    println!("✓ Model load request submitted for '{}'", name);
                    println!("{}", resp.text().await?);
                } else {
                    eprintln!("✗ Failed to load model: {}", resp.text().await?);
                }
            }
            ModelCommand::Unload { id } => {
                let url = format!("{}/v1/admin/models/requests/{}", args.gateway_url.trim_end_matches('/'), id);
                let resp = client.delete(&url).send().await?;
                if resp.status().is_success() {
                    println!("✓ Model unload request submitted for ID: {}", id);
                } else {
                    eprintln!("✗ Failed to unload model: {}", resp.text().await?);
                }
            }
        },
    }

    Ok(())
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
