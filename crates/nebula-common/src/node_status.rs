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
}
