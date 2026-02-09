use serde::{Deserialize, Serialize};
use crate::{EndpointInfo, NodeStatus, PlacementPlan, ModelRequest};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterStatus {
    pub nodes: Vec<NodeStatus>,
    pub endpoints: Vec<EndpointInfo>,
    pub placements: Vec<PlacementPlan>,
    pub model_requests: Vec<ModelRequest>,
}
