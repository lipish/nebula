use std::sync::Arc;

use reqwest::Client;

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
}

impl AsRef<AuthConfig> for AppState {
    fn as_ref(&self) -> &AuthConfig {
        &self.auth
    }
}
