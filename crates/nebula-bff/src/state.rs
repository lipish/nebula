use std::sync::Arc;

use reqwest::Client;
use sqlx::PgPool;

use crate::args::XtraceAuthMode;
use nebula_meta::EtcdMetaStore;

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<EtcdMetaStore>,
    pub db: PgPool,
    pub http: Client,
    pub router_url: String,
    pub session_ttl_hours: i64,
    pub xtrace_url: String,
    pub xtrace_token: String,
    pub xtrace_auth_mode: XtraceAuthMode,
}
