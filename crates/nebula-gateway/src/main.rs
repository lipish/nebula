mod auth;
mod engine;
mod handlers;
mod metrics;
mod responses;
mod state;
mod util;

use std::env;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use tracing_subscriber::EnvFilter;

use crate::auth::parse_auth_from_env;
use crate::engine::{EngineClient, OpenAIEngineClient};
use crate::handlers::{
    admin_cluster_status, admin_delete_request, admin_list_requests, admin_load_model, admin_logs,
    admin_ui, admin_whoami, create_responses, healthz, list_models, not_implemented, proxy_post,
};
use crate::metrics::{metrics_handler, track_requests};
use crate::state::AppState;
use crate::util::read_engine_env_file;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let router_base_url =
        env::var("NEBULA_ROUTER_URL").unwrap_or_else(|_| "http://127.0.0.1:18081".to_string());

    let engine_model = match env::var("NEBULA_ENGINE_MODEL") {
        Ok(model) => model,
        Err(_) => read_engine_env_file("/tmp/nebula/engine.env")
            .await
            .map(|(_url, model)| model)
            .unwrap_or_else(|| "unknown".to_string()),
    };

    tracing::info!(router_base_url=%router_base_url, engine_model=%engine_model, "gateway starting");

    let engine: Arc<dyn EngineClient> = Arc::new(OpenAIEngineClient::new(
        router_base_url.clone(),
        engine_model,
    ));

    let http = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(300))
        .build()
        .expect("reqwest client");

    let etcd_endpoint =
        env::var("ETCD_ENDPOINT").unwrap_or_else(|_| "http://127.0.0.1:2379".to_string());
    let store = nebula_meta::EtcdMetaStore::connect(&[etcd_endpoint])
        .await
        .expect("etcd connect");

    let auth = parse_auth_from_env();

    let metrics = Arc::new(metrics::Metrics::default());

    let log_path = env::var("NEBULA_GATEWAY_LOG_PATH")
        .unwrap_or_else(|_| "/tmp/nebula-gateway.log".to_string());

    let st = AppState {
        _noop: Arc::new(()),
        engine,
        router_base_url,
        http,
        store: Arc::new(store),
        auth,
        metrics,
        log_path,
    };

    let admin_routes = Router::new()
        .route("/models/load", post(admin_load_model))
        .route("/models/requests", get(admin_list_requests))
        .route(
            "/models/requests/:id",
            post(admin_delete_request).delete(admin_delete_request),
        )
        .route("/cluster/status", get(admin_cluster_status))
        .route("/whoami", get(admin_whoami))
        .route("/metrics", get(metrics::admin_metrics))
        .route("/logs", get(admin_logs))
        .layer(middleware::from_fn_with_state(st.clone(), auth::admin_auth))
        .with_state(st.clone());

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/health", get(healthz))
        .route("/metrics", get(metrics_handler))
        .route("/v1/responses", get(not_implemented).post(create_responses))
        .route("/v1/chat/completions", post(proxy_post))
        .route("/v1/embeddings", post(proxy_post))
        .route("/v1/rerank", post(proxy_post))
        .route("/v1/models", get(list_models))
        .route("/v1/admin/ui", get(admin_ui))
        .nest("/v1/admin", admin_routes)
        .layer(middleware::from_fn_with_state(st.clone(), track_requests))
        .with_state(st);

    let addr = env::var("NEBULA_GATEWAY_ADDR").unwrap_or_else(|_| {
        if let Ok(port) = env::var("PORT") {
            format!("0.0.0.0:{port}")
        } else {
            "0.0.0.0:8080".to_string()
        }
    });

    let listener = tokio::net::TcpListener::bind(&addr).await.expect("bind");

    axum::serve(listener, app).await.expect("serve");
}
