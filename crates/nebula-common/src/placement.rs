use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlacementAssignment {
    pub replica_id: u32,
    pub node_id: String,
    pub engine_config_path: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlacementPlan {
    pub model_uid: String,
    pub version: u64,
    pub assignments: Vec<PlacementAssignment>,
}
