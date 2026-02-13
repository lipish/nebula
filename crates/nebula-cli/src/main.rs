mod args;
mod chat;
mod client;
mod config;
mod output;

use anyhow::Result;
use clap::Parser;
use reqwest::Client;

use nebula_common::{ClusterStatus, ModelLoadRequest, ModelRequest};

use crate::args::{Args, ClusterCommand, Command, ModelCommand};
use crate::client::auth;
use crate::config::build_config;
use crate::output::{print_cluster_status, print_model_requests};

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let client = Client::new();
    let token = args.token;

    match args.command {
        Command::Cluster { subcommand } => match subcommand {
            ClusterCommand::Status => {
                let url = format!(
                    "{}/v1/admin/cluster/status",
                    args.gateway_url.trim_end_matches('/')
                );
                let status: ClusterStatus = auth(client.get(&url), token.as_ref())
                    .send()
                    .await?
                    .json()
                    .await?;
                print_cluster_status(status);
            }
        },
        Command::Model { subcommand } => match subcommand {
            ModelCommand::List => {
                let url = format!(
                    "{}/v1/admin/models/requests",
                    args.gateway_url.trim_end_matches('/')
                );
                let requests: Vec<ModelRequest> = auth(client.get(&url), token.as_ref())
                    .send()
                    .await?
                    .json()
                    .await?;
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
                let url = format!(
                    "{}/v1/admin/models/load",
                    args.gateway_url.trim_end_matches('/')
                );
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
                    node_id: None,
                    gpu_index: None,
                    gpu_indices: None,
                    min_replicas: None,
                    max_replicas: None,
                };
                let resp = auth(client.post(&url), token.as_ref())
                    .json(&req)
                    .send()
                    .await?;
                if resp.status().is_success() {
                    println!("✓ Model load request submitted for '{}'", name);
                    println!("{}", resp.text().await?);
                } else {
                    eprintln!("✗ Failed to load model: {}", resp.text().await?);
                }
            }
            ModelCommand::Unload { id } => {
                let url = format!(
                    "{}/v1/admin/models/requests/{}",
                    args.gateway_url.trim_end_matches('/'),
                    id
                );
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
            let url = format!(
                "{}/v1/admin/metrics",
                args.gateway_url.trim_end_matches('/')
            );
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
        Command::Chat {
            model,
            system,
            message,
            max_tokens,
        } => {
            let base = args.gateway_url.trim_end_matches('/').to_string();
            chat::run_chat(&client, &base, token.as_ref(), model, system, message, max_tokens)
                .await?;
        }
        Command::Scale { id, replicas } => {
            let url = format!(
                "{}/v1/admin/models/requests/{}/scale",
                args.gateway_url.trim_end_matches('/'),
                id
            );
            let body = serde_json::json!({ "replicas": replicas });
            let resp = auth(client.put(&url), token.as_ref())
                .json(&body)
                .send()
                .await?;
            if resp.status().is_success() {
                println!("✓ Scaled request '{}' to {} replicas", id, replicas);
            } else {
                eprintln!("✗ Failed to scale: {}", resp.text().await?);
            }
        }
        Command::Drain {
            model_uid,
            replica_id,
        } => {
            let url = format!(
                "{}/v1/admin/endpoints/drain",
                args.gateway_url.trim_end_matches('/')
            );
            let body = serde_json::json!({
                "model_uid": model_uid,
                "replica_id": replica_id
            });
            let resp = auth(client.post(&url), token.as_ref())
                .json(&body)
                .send()
                .await?;
            if resp.status().is_success() {
                println!(
                    "✓ Draining endpoint {}/replica-{}",
                    model_uid, replica_id
                );
            } else {
                eprintln!("✗ Failed to drain: {}", resp.text().await?);
            }
        }
    }

    Ok(())
}
