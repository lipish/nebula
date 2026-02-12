use axum::{routing::get, Json, Router};
use serde::Serialize;
use tokio::process::Command;

#[derive(Debug, Clone, Serialize)]
pub struct ContainerInfo {
    pub name: String,
    pub image: String,
    pub image_id: String,
    pub status: String,
    pub state: String,
    pub ports: String,
    pub created: String,
    pub model_uid: Option<String>,
    pub replica_id: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImageInfo {
    pub repository: String,
    pub tag: String,
    pub image_id: String,
    pub size: String,
    pub created: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeDockerStatus {
    pub containers: Vec<ContainerInfo>,
    pub images: Vec<ImageInfo>,
    pub docker_version: Option<String>,
}

/// Parse container name like "nebula-qwen2-5-0-5b-instruct-0" into (model_uid, replica_id).
fn parse_nebula_container_name(name: &str) -> (Option<String>, Option<u32>) {
    let stripped = name.strip_prefix('/').unwrap_or(name);
    if let Some(rest) = stripped.strip_prefix("nebula-") {
        // Last segment after the final '-' is the replica_id
        if let Some(pos) = rest.rfind('-') {
            let model_uid = &rest[..pos];
            let replica_str = &rest[pos + 1..];
            if let Ok(replica_id) = replica_str.parse::<u32>() {
                return (Some(model_uid.to_string()), Some(replica_id));
            }
        }
    }
    (None, None)
}

async fn list_containers() -> Vec<ContainerInfo> {
    let output = Command::new("docker")
        .args([
            "ps", "-a",
            "--format", "{{.Names}}\t{{.Image}}\t{{.ID}}\t{{.Status}}\t{{.State}}\t{{.Ports}}\t{{.CreatedAt}}",
            "--filter", "name=nebula-",
        ])
        .output()
        .await;

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .filter(|l| !l.is_empty())
        .map(|line| {
            let parts: Vec<&str> = line.splitn(7, '\t').collect();
            let name = parts.first().unwrap_or(&"").to_string();
            let (model_uid, replica_id) = parse_nebula_container_name(&name);
            ContainerInfo {
                name,
                image: parts.get(1).unwrap_or(&"").to_string(),
                image_id: parts.get(2).unwrap_or(&"").to_string(),
                status: parts.get(3).unwrap_or(&"").to_string(),
                state: parts.get(4).unwrap_or(&"").to_string(),
                ports: parts.get(5).unwrap_or(&"").to_string(),
                created: parts.get(6).unwrap_or(&"").to_string(),
                model_uid,
                replica_id,
            }
        })
        .collect()
}

async fn list_images() -> Vec<ImageInfo> {
    let output = Command::new("docker")
        .args([
            "images",
            "--format", "{{.Repository}}\t{{.Tag}}\t{{.ID}}\t{{.Size}}\t{{.CreatedAt}}",
            "--filter", "dangling=false",
        ])
        .output()
        .await;

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .filter(|l| !l.is_empty())
        .map(|line| {
            let parts: Vec<&str> = line.splitn(5, '\t').collect();
            ImageInfo {
                repository: parts.first().unwrap_or(&"").to_string(),
                tag: parts.get(1).unwrap_or(&"").to_string(),
                image_id: parts.get(2).unwrap_or(&"").to_string(),
                size: parts.get(3).unwrap_or(&"").to_string(),
                created: parts.get(4).unwrap_or(&"").to_string(),
            }
        })
        .collect()
}

async fn docker_version() -> Option<String> {
    let output = Command::new("docker")
        .args(["version", "--format", "{{.Server.Version}}"])
        .output()
        .await
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

// -- Handlers --

async fn get_containers() -> Json<Vec<ContainerInfo>> {
    Json(list_containers().await)
}

async fn get_images() -> Json<Vec<ImageInfo>> {
    Json(list_images().await)
}

async fn get_docker_status() -> Json<NodeDockerStatus> {
    let (containers, images, version) =
        tokio::join!(list_containers(), list_images(), docker_version());
    Json(NodeDockerStatus {
        containers,
        images,
        docker_version: version,
    })
}

/// Build the Node API router.
pub fn node_api_router() -> Router {
    Router::new()
        .route("/api/containers", get(get_containers))
        .route("/api/images", get(get_images))
        .route("/api/docker", get(get_docker_status))
}
