use nebula_common::{ClusterStatus, EndpointStatus, ModelRequest};

pub fn print_cluster_status(status: ClusterStatus) {
    println!("\n=== Nebula Cluster Status ===");

    println!("\n[Nodes]");
    if status.nodes.is_empty() {
        println!("  (No nodes registered)");
    } else {
        println!("  {:<20} {:<20}", "Node ID", "Last Heartbeat");
        for node in status.nodes {
            println!(
                "  {:<20} {:>10}ms ago",
                node.node_id,
                last_seen(node.last_heartbeat_ms)
            );
        }
    }

    println!("\n[Endpoints]");
    if status.endpoints.is_empty() {
        println!("  (No active endpoints)");
    } else {
        println!(
            "  {:<20} {:<10} {:<10} {:<30}",
            "Model UID", "Replica", "Status", "Base URL"
        );
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
    println!();
}

pub fn print_model_requests(requests: Vec<ModelRequest>) {
    println!("\n=== Nebula Model Requests ===");
    if requests.is_empty() {
        println!("No active or past model requests found.");
        return;
    }

    println!(
        "{:<40} {:<25} {:<15} {:<10}",
        "Request ID", "Model Name", "Status", "Replicas"
    );
    println!("{:-<100}", "");
    for req in requests {
        let status_str = format!("{:?}", req.status);
        println!(
            "{:<40} {:<25} {:<15} {:<10}",
            req.id, req.request.model_name, status_str, req.request.replicas
        );
    }
    println!();
}

fn last_seen(ts_ms: u64) -> u64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    now.saturating_sub(ts_ms)
}
