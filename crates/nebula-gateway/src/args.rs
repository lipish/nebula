use clap::Parser;

#[derive(Debug, Parser)]
#[command(author, version, about)]
pub struct Args {
    #[arg(long, env = "NEBULA_GATEWAY_ADDR", default_value = "0.0.0.0:8081")]
    pub listen_addr: String,

    #[arg(long, env = "NEBULA_ROUTER_URL", default_value = "http://127.0.0.1:18081")]
    pub router_url: String,

    #[arg(long, env = "ETCD_ENDPOINT", default_value = "http://127.0.0.1:2379")]
    pub etcd_endpoint: String,

    #[arg(long, env = "NEBULA_GATEWAY_LOG_PATH", default_value = "/tmp/nebula-gateway.log")]
    pub log_path: String,

    #[arg(long, env = "NEBULA_ENGINE_MODEL")]
    pub engine_model: Option<String>,

    /// OTLP endpoint for exporting traces (e.g. "http://10.21.11.92:8742/api/public/otel").
    #[arg(long, env = "OBSERVE_URL")]
    pub xtrace_url: Option<String>,

    /// Bearer token for xtrace authentication.
    #[arg(long, env = "OBSERVE_TOKEN")]
    pub xtrace_token: Option<String>,

    /// Log output format: "text" (human-readable, default) or "json" (structured).
    #[arg(long, env = "NEBULA_LOG_FORMAT", default_value = "text")]
    pub log_format: String,

    /// BFF service URL for v2 API proxy.
    #[arg(long, env = "NEBULA_BFF_URL", default_value = "http://127.0.0.1:18090")]
    pub bff_url: String,
}
