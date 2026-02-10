use std::sync::Arc;

use reqwest::Client;

use nebula_meta::EtcdMetaStore;

use crate::auth::AuthConfig;

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<EtcdMetaStore>,
    pub http: Client,
    pub router_url: String,
    pub auth: AuthConfig,
}
