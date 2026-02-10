use std::sync::Arc;

use nebula_meta::EtcdMetaStore;

use crate::auth::AuthState;
use crate::engine::EngineClient;
use crate::metrics::Metrics;

#[derive(Clone)]
pub struct AppState {
    pub _noop: Arc<()>,
    pub engine: Arc<dyn EngineClient>,
    pub router_base_url: String,
    pub http: reqwest::Client,
    pub store: Arc<EtcdMetaStore>,
    pub auth: AuthState,
    pub metrics: Arc<Metrics>,
    pub log_path: String,
}
