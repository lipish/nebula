mod args;
mod auth;
mod handlers;
mod handlers_v2;
mod state;

use std::sync::Arc;
use std::time::Duration;

use axum::{
    middleware,
    routing::{delete, get, post, put},
    Router,
};
use clap::Parser;
use tracing_subscriber::EnvFilter;

use crate::args::Args;
use crate::auth::parse_bff_auth_from_env;
use crate::handlers::{
    audit_logs, delete_image, engine_stats, get_image, healthz, list_image_status, list_images,
    list_requests, load_model, logs, metrics, observe_metrics_names, observe_metrics_query,
    observe_trace_detail, observe_traces, overview, put_image, search_models, unload_model, whoami,
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

    let auth = parse_bff_auth_from_env();

    let st = AppState {
        store: Arc::new(store),
        http,
        router_url: args.router_url,
        auth,
        xtrace_url: args.xtrace_url,
        xtrace_token: args.xtrace_token,
    };

    let protected_routes = Router::new()
        .route("/whoami", get(whoami))
        .route("/overview", get(overview))
        .route("/requests", get(list_requests))
        .route("/models/load", post(load_model))
        .route("/models/requests/:id", delete(unload_model))
        .route("/metrics", get(metrics))
        .route("/engine-stats", get(engine_stats))
        .route("/logs", get(logs))
        .route("/models/search", get(search_models))
        .route("/observe/traces", get(observe_traces))
        .route("/observe/traces/:traceId", get(observe_trace_detail))
        .route("/observe/metrics/query", get(observe_metrics_query))
        .route("/observe/metrics/names", get(observe_metrics_names))
        .route("/audit-logs", get(audit_logs))
        // Image registry
        .route("/images", get(list_images))
        .route("/images/status", get(list_image_status))
        .route("/images/:id", get(get_image).put(put_image).delete(delete_image))
        .layer(middleware::from_fn_with_state(st.clone(), nebula_common::auth::auth_middleware::<AppState>))
        .with_state(st.clone());

    let v2_routes = Router::new()
        .route("/models", get(handlers_v2::list_models).post(handlers_v2::create_model))
        .route("/models/:model_uid", get(handlers_v2::get_model).put(handlers_v2::update_model).delete(handlers_v2::delete_model))
        .route("/models/:model_uid/start", post(handlers_v2::start_model))
        .route("/models/:model_uid/stop", post(handlers_v2::stop_model))
        .route("/models/:model_uid/scale", put(handlers_v2::scale_model))
        .route("/models/:model_uid/save-as-template", post(handlers_v2::save_as_template))
        .route("/templates", get(handlers_v2::list_templates).post(handlers_v2::create_template))
        .route("/templates/:id", get(handlers_v2::get_template).put(handlers_v2::update_template).delete(handlers_v2::delete_template))
        .route("/templates/:id/deploy", post(handlers_v2::deploy_template))
        .route("/nodes/:node_id/cache", get(handlers_v2::node_cache))
        .route("/nodes/:node_id/disk", get(handlers_v2::node_disk))
        .route("/cache/summary", get(handlers_v2::cache_summary))
        .route("/alerts", get(handlers_v2::list_alerts))
        .layer(middleware::from_fn_with_state(st.clone(), nebula_common::auth::auth_middleware::<AppState>))
        .with_state(st.clone());

    let api_routes = Router::new()
        .route("/healthz", get(healthz))
        .merge(protected_routes);

    let app = Router::new()
        .nest("/api", api_routes)
        .nest("/api/v2", v2_routes);

    let listener = tokio::net::TcpListener::bind(&args.listen_addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
