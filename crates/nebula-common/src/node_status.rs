use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GpuStatus {
    pub index: u32,
    pub memory_total_mb: u64,
    pub memory_used_mb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeStatus {
    pub node_id: String,
    pub last_heartbeat_ms: u64,

    #[serde(default)]
    pub gpus: Vec<GpuStatus>,

    /// Node HTTP API address (e.g. "http://10.21.11.92:9090") for BFF to query containers/images.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_addr: Option<String>,
}
