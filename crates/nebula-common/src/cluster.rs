use crate::{EndpointInfo, ModelRequest, NodeStatus, PlacementPlan};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterStatus {
    pub nodes: Vec<NodeStatus>,
    pub endpoints: Vec<EndpointInfo>,
    pub placements: Vec<PlacementPlan>,
    pub model_requests: Vec<ModelRequest>,
}
