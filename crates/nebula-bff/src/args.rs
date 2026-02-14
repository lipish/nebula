use clap::{Parser, ValueEnum};

#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "lower")]
pub enum XtraceAuthMode {
    /// Use service-to-service bearer token (XTRACE_TOKEN).
    Service,
    /// Trust internal network, do not send auth header to xtrace.
    Internal,
}

#[derive(Debug, Parser)]
#[command(author, version, about)]
pub struct Args {
    #[arg(long, env = "NEBULA_BFF_ADDR", default_value = "0.0.0.0:18090")]
    pub listen_addr: String,

    #[arg(long, env = "ETCD_ENDPOINT", default_value = "http://127.0.0.1:2379")]
    pub etcd_endpoint: String,

    #[arg(long, env = "NEBULA_ROUTER_URL", default_value = "http://127.0.0.1:18081")]
    pub router_url: String,

    /// xtrace (nebula-observe) base URL for observability queries.
    #[arg(long, env = "XTRACE_URL", default_value = "http://127.0.0.1:8742")]
    pub xtrace_url: String,

    /// xtrace bearer token for authentication.
    #[arg(long, env = "XTRACE_TOKEN", default_value = "")]
    pub xtrace_token: String,

    /// xtrace auth mode: service (token) or internal (no token).
    #[arg(long, env = "XTRACE_AUTH_MODE", value_enum, default_value_t = XtraceAuthMode::Service)]
    pub xtrace_auth_mode: XtraceAuthMode,
}
