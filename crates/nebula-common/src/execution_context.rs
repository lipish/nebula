use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ExecutionContext {
    pub request_id: String,
    pub session_id: Option<String>,
    pub tenant_id: Option<String>,
    pub priority: Option<i32>,
    pub deadline_ms: Option<u64>,
    pub budget_tokens: Option<u32>,
}
