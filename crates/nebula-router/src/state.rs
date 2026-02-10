use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use crate::metrics::Metrics;

#[derive(Clone)]
pub struct AppState {
    pub model_uid: String,
    pub router: Arc<nebula_router::Router>,
    pub http: reqwest::Client,
    pub plan_version: Arc<AtomicU64>,
    pub metrics: Arc<Metrics>,
}
