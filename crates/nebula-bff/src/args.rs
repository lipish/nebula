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
}
