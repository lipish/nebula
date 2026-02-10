use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "nebula")]
#[command(about = "Nebula CLI for cluster management", long_about = None)]
pub struct Args {
    /// Gateway URL
    #[arg(
        long,
        env = "NEBULA_GATEWAY_URL",
        default_value = "http://127.0.0.1:8081"
    )]
    pub gateway_url: String,

    /// Gateway API token (Authorization: Bearer)
    #[arg(long, env = "NEBULA_GATEWAY_TOKEN")]
    pub token: Option<String>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Cluster management
    Cluster {
        #[command(subcommand)]
        subcommand: ClusterCommand,
    },
    /// Model management
    Model {
        #[command(subcommand)]
        subcommand: ModelCommand,
    },
    /// Show current auth identity
    Whoami,
    /// Fetch gateway metrics
    Metrics,
    /// Tail gateway logs
    Logs {
        /// Lines to return (default 200, max 2000)
        #[arg(long)]
        lines: Option<u32>,
    },
}

#[derive(Debug, Subcommand)]
pub enum ClusterCommand {
    /// Show cluster status
    Status,
}

#[derive(Debug, Subcommand)]
pub enum ModelCommand {
    /// List all model requests and their status
    List,
    /// Load a new model
    Load {
        /// User-facing model name
        #[arg(long)]
        name: String,
        /// Internal model UID
        #[arg(long)]
        uid: String,
        /// Number of replicas
        #[arg(long, default_value_t = 1)]
        replicas: u32,

        /// Required VRAM in MB (capacity check)
        #[arg(long)]
        required_vram_mb: Option<u64>,

        /// vLLM tensor parallel size
        #[arg(long)]
        tensor_parallel_size: Option<u32>,

        /// vLLM GPU memory utilization (0-1)
        #[arg(long)]
        gpu_memory_utilization: Option<f32>,

        /// vLLM max model length
        #[arg(long)]
        max_model_len: Option<u32>,

        /// LoRA modules (repeatable)
        #[arg(long, value_delimiter = ',')]
        lora: Option<Vec<String>>,
    },
    /// Unload a model by request ID
    Unload {
        /// Request ID to unload
        id: String,
    },
}
