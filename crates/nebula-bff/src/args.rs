use clap::Parser;

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
}
