use clap::Parser;

#[derive(Debug, Parser)]
pub struct Args {
    #[arg(long, default_value = "0.0.0.0:18081")]
    pub listen_addr: String,

    #[arg(long, default_value = "http://127.0.0.1:2379")]
    pub etcd_endpoint: String,

    #[arg(long, default_value = "qwen2_5_0_5b")]
    pub model_uid: String,
}
