use clap::Parser;

#[derive(Debug, Parser)]
pub struct Args {
    #[arg(long, default_value = "http://127.0.0.1:2379")]
    pub etcd_endpoint: String,

    #[arg(long, default_value = "node_gpu0")]
    pub default_node_id: String,

    #[arg(long, default_value_t = 10814)]
    pub default_port: u16,

    /// xtrace URL for querying engine stats (e.g. "http://10.21.11.92:8742/").
    #[arg(long, env = "OBSERVE_URL")]
    pub xtrace_url: Option<String>,

    /// Bearer token for xtrace authentication.
    #[arg(long, env = "OBSERVE_TOKEN")]
    pub xtrace_token: Option<String>,

    /// Address for the metrics / health HTTP server.
    #[arg(long, default_value = "0.0.0.0:18082")]
    pub listen_addr: String,

    /// Log output format: "text" (human-readable, default) or "json" (structured).
    #[arg(long, env = "NEBULA_LOG_FORMAT", default_value = "text")]
    pub log_format: String,
}
