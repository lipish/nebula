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
}
