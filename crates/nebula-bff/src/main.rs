mod args;
mod auth;
mod handlers;
mod state;

use std::sync::Arc;
use std::time::Duration;

use axum::{
    middleware,
    routing::{delete, get, post},
    Router,
};
use clap::Parser;
use tracing_subscriber::EnvFilter;

use crate::args::Args;
use crate::auth::parse_auth_from_env;
use crate::handlers::{
    healthz, list_requests, load_model, logs, metrics, overview, unload_model, whoami,
};
use crate::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args = Args::parse();

    let store = nebula_meta::EtcdMetaStore::connect(std::slice::from_ref(&args.etcd_endpoint))
        .await?;

    let http = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap_or_else(|e| {
            tracing::error!(error=%e, "failed to build reqwest client");
            std::process::exit(1);
        });

    let auth = parse_auth_from_env();

    let st = AppState {
        store: Arc::new(store),
        http,
        router_url: args.router_url,
        auth,
    };

    let protected_routes = Router::new()
        .route("/whoami", get(whoami))
        .route("/overview", get(overview))
        .route("/requests", get(list_requests))
        .route("/models/load", post(load_model))
        .route("/models/requests/:id", delete(unload_model))
        .route("/metrics", get(metrics))
        .route("/logs", get(logs))
        .layer(middleware::from_fn_with_state(st.clone(), auth::auth_middleware))
        .with_state(st.clone());

    let api_routes = Router::new()
        .route("/healthz", get(healthz))
        .merge(protected_routes);

    let app = Router::new().nest("/api", api_routes);

    let listener = tokio::net::TcpListener::bind(&args.listen_addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
