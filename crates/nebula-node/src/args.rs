use clap::Parser;

#[derive(Debug, Parser)]
pub struct Args {
    #[arg(long, default_value = "node_1")]
    pub node_id: String,

    #[arg(long, default_value = "http://127.0.0.1:2379")]
    pub etcd_endpoint: String,

    #[arg(long, default_value = "/home/ai/miniconda3/envs/Lvllm/bin/vllm")]
    pub vllm_bin: String,

    #[arg(long, default_value = "Lvllm/m21.yaml")]
    pub vllm_config: String,

    #[arg(long, default_value = "/home/ai")]
    pub vllm_cwd: String,

    #[arg(long, default_value_t = 10814)]
    pub vllm_port: u16,

    #[arg(long, default_value = "0.0.0.0")]
    pub vllm_host: String,

    #[arg(long, default_value = "/tmp/nebula/engine.env")]
    pub engine_env_path: String,

    #[arg(long, default_value_t = 10_000)]
    pub heartbeat_ttl_ms: u64,

    #[arg(long, default_value_t = 3_000)]
    pub heartbeat_interval_ms: u64,

    #[arg(long, default_value_t = 180)]
    pub ready_timeout_secs: u64,

    #[arg(long)]
    pub vllm_gpu_memory_utilization: Option<f32>,

    #[arg(long)]
    pub vllm_max_model_len: Option<u32>,

    #[arg(long)]
    pub vllm_swap_space: Option<u32>,

    #[arg(long)]
    pub vllm_max_num_batched_tokens: Option<u32>,

    #[arg(long)]
    pub vllm_max_num_seqs: Option<u32>,

    #[arg(long)]
    pub vllm_tensor_parallel_size: Option<u32>,
}
