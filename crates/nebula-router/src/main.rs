mod args;
mod handlers;
mod metrics;
mod state;
mod sync;

use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use clap::Parser;

use crate::args::Args;
use crate::handlers::{healthz, proxy_chat_completions};
use crate::metrics::{metrics_handler, track_requests};
use crate::state::AppState;
use crate::sync::{endpoints_sync_loop, placement_sync_loop, stats_sync_loop};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let _otel_guard = nebula_common::telemetry::init_tracing(
        "nebula-router",
        args.xtrace_url.as_deref(),
        args.xtrace_token.as_deref(),
        &args.log_format,
    );

    let store =
        nebula_meta::EtcdMetaStore::connect(std::slice::from_ref(&args.etcd_endpoint)).await?;

    let strategy = nebula_router::strategy::parse_strategy(&args.routing_strategy)
        .unwrap_or_else(|e| {
            tracing::error!(error=%e, "invalid routing strategy");
            std::process::exit(1);
        });
    let router = nebula_router::Router::with_strategy(strategy);

    let plan_version = Arc::new(AtomicU64::new(0));

    let router_for_sync = router.clone();
    let store_for_endpoints = store.clone();
    let store_for_placement = store.clone();
    let model_uid_for_placement = args.model_uid.clone();
    let plan_version_for_placement = plan_version.clone();

    tokio::spawn(async move {
        if let Err(e) = endpoints_sync_loop(store_for_endpoints, router_for_sync).await {
            tracing::error!(error=%e, "endpoints sync loop exited");
        }
    });

    tokio::spawn(async move {
        if let Err(e) = placement_sync_loop(
            store_for_placement,
            model_uid_for_placement,
            plan_version_for_placement,
        )
        .await
        {
            tracing::error!(error=%e, "placement sync loop exited");
        }
    });

    let router_for_stats = router.clone();
    if let (Some(url), token) = (
        args.xtrace_url.as_deref(),
        args.xtrace_token.as_deref().unwrap_or(""),
    ) {
        match xtrace_client::Client::new(url, token) {
            Ok(xtrace) => {
                tokio::spawn(async move {
                    if let Err(e) = stats_sync_loop(xtrace, router_for_stats).await {
                        tracing::error!(error=%e, "stats sync loop exited");
                    }
                });
            }
            Err(e) => {
                tracing::warn!(error=%e, "failed to create xtrace client, stats sync disabled");
            }
        }
    } else {
        tracing::warn!("xtrace_url not set, stats sync disabled");
    }

    let http = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(300))
        .build()
        .unwrap_or_else(|e| {
            tracing::error!(error=%e, "failed to build reqwest client");
            std::process::exit(1);
        });

    let metrics = Arc::new(metrics::Metrics::default());

    let auth = nebula_common::auth::parse_auth_from_env();

    let st = AppState {
        model_uid: args.model_uid,
        router,
        http,
        plan_version,
        metrics,
        auth,
    };

    // Routes that require auth (inference endpoints)
    let authed_routes = Router::new()
        .route("/v1/chat/completions", post(proxy_chat_completions))
        .route("/v1/completions", post(proxy_chat_completions))
        .route("/v1/embeddings", post(proxy_chat_completions))
        .route("/v1/rerank", post(proxy_chat_completions))
        .route(
            "/v1/models",
            post(proxy_chat_completions).get(proxy_chat_completions),
        )
        .layer(middleware::from_fn_with_state(
            st.clone(),
            nebula_common::auth::auth_middleware::<AppState>,
        ));

    // Routes that do NOT require auth (health/metrics)
    let public_routes = Router::new()
        .route("/healthz", get(healthz))
        .route("/health", get(healthz))
        .route("/metrics", get(metrics_handler));

    let app = public_routes
        .merge(authed_routes)
        .layer(middleware::from_fn_with_state(st.clone(), track_requests))
        .with_state(st);

    let listener = tokio::net::TcpListener::bind(&args.listen_addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
