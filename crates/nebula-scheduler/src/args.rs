use clap::Parser;

#[derive(Debug, Parser)]
pub struct Args {
    #[arg(long, default_value = "http://127.0.0.1:2379")]
    pub etcd_endpoint: String,

    #[arg(long, default_value = "node_gpu0")]
    pub default_node_id: String,

    #[arg(long, default_value_t = 10814)]
    pub default_port: u16,
}
