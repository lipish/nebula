use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlacementAssignment {
    pub replica_id: u32,
    pub node_id: String,
    pub engine_config_path: String,
    pub port: u16,

    /// Legacy single-GPU field (kept for backward compatibility with existing etcd data)
    #[serde(default)]
    pub gpu_index: Option<u32>,

    /// Multi-GPU indices for tensor-parallel deployment
    #[serde(default)]
    pub gpu_indices: Option<Vec<u32>>,

    #[serde(default)]
    pub extra_args: Option<Vec<String>>,

    /// Engine type: "vllm", "sglang", etc. Defaults to "vllm" if absent.
    #[serde(default)]
    pub engine_type: Option<String>,
}

impl PlacementAssignment {
    /// Resolve effective GPU indices: prefer gpu_indices, fall back to gpu_index
    pub fn effective_gpu_indices(&self) -> Option<Vec<u32>> {
        if let Some(indices) = &self.gpu_indices {
            if !indices.is_empty() {
                return Some(indices.clone());
            }
        }
        self.gpu_index.map(|i| vec![i])
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlacementPlan {
    #[serde(default)]
    pub request_id: Option<String>,
    pub model_uid: String,
    pub model_name: String,
    pub version: u64,
    pub assignments: Vec<PlacementAssignment>,
}
