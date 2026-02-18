use clap::Parser;

#[derive(Debug, Parser)]
pub struct Args {
    #[arg(long, default_value = "0.0.0.0:18081")]
    pub listen_addr: String,

    #[arg(long, default_value = "http://127.0.0.1:2379")]
    pub etcd_endpoint: String,

    #[arg(long, default_value = "qwen2_5_0_5b")]
    pub model_uid: String,

    #[arg(long, default_value = "least_pending")]
    pub routing_strategy: String,

    /// OTLP endpoint for exporting traces (e.g. "http://10.21.11.92:8742/api/public/otel").
    #[arg(long, env = "OBSERVE_URL")]
    pub xtrace_url: Option<String>,

    /// Bearer token for xtrace authentication.
    #[arg(long, env = "OBSERVE_TOKEN")]
    pub xtrace_token: Option<String>,

    /// Log output format: "text" (human-readable, default) or "json" (structured).
    #[arg(long, env = "NEBULA_LOG_FORMAT", default_value = "text")]
    pub log_format: String,
}
