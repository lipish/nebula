use std::sync::Arc;

use reqwest::Client;

use crate::args::XtraceAuthMode;
use nebula_common::auth::AuthConfig;
use nebula_meta::EtcdMetaStore;

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<EtcdMetaStore>,
    pub http: Client,
    pub router_url: String,
    pub auth: AuthConfig,
    pub xtrace_url: String,
    pub xtrace_token: String,
    pub xtrace_auth_mode: XtraceAuthMode,
}

impl AsRef<AuthConfig> for AppState {
    fn as_ref(&self) -> &AuthConfig {
        &self.auth
    }
}
