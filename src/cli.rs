use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "whichllm", about = "Find the best local LLM for your hardware")]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Simulate a specific GPU (e.g. "RTX 4090")
    #[arg(long)]
    pub gpu: Option<String>,

    /// Number of results to show
    #[arg(long, default_value = "10")]
    pub top: usize,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Output as GitHub-Flavored Markdown table
    #[arg(long, short = 'm')]
    pub markdown: bool,

    /// Bypass cache, fetch fresh data
    #[arg(long)]
    pub refresh: bool,

    /// Speed filter: "usable" (>=10 t/s) or "fast" (>=30 t/s)
    #[arg(long)]
    pub speed: Option<String>,

    /// Exact minimum tok/s floor
    #[arg(long)]
    pub min_speed: Option<f64>,

    /// Only show models that fit entirely in GPU VRAM
    #[arg(long)]
    pub gpu_only: bool,

    /// Fit filter: full-gpu, gpu (same as full-gpu), any (default)
    #[arg(long)]
    pub fit: Option<String>,

    /// Force CPU-only mode
    #[arg(long)]
    pub cpu_only: bool,

    /// Reserve extra VRAM (e.g. "1GB", "1.5GB")
    #[arg(long)]
    pub vram_headroom: Option<String>,

    /// Limit system RAM usable for offload (e.g. "8GB", "available")
    #[arg(long)]
    pub ram_budget: Option<String>,

    /// Task profile: general, coding, vision, math
    #[arg(long)]
    pub profile: Option<String>,

    /// Filter by quantization (e.g. "Q4_K_M")
    #[arg(long)]
    pub quant: Option<String>,

    /// Override context length for KV cache (e.g. "4096", "4k", "64k")
    #[arg(long)]
    pub context_length: Option<String>,

    /// Show download metadata instead of runtime columns
    #[arg(long)]
    pub details: bool,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Command {
    /// Show hardware info only
    Hardware,
    /// What GPU do I need for a specific model?
    Plan {
        /// Model name (fuzzy match, e.g. "llama 3 70b")
        model: String,
        /// Quantization filter
        #[arg(long)]
        quant: Option<String>,
        /// Context length override
        #[arg(long)]
        context_length: Option<String>,
    },
    /// Compare upgrade GPU candidates
    Upgrade {
        /// GPU names to compare
        gpus: Vec<String>,
        /// Number of top results per GPU
        #[arg(long, default_value = "3")]
        top: usize,
    },
}

/// Parse a size string like "1GB", "1.5GB" into MB
pub fn parse_size_mb(s: &str) -> Option<u64> {
    let s = s.to_uppercase();
    if let Some(num_str) = s.strip_suffix("GB") {
        num_str.parse::<f64>().ok().map(|v| (v * 1024.0) as u64)
    } else if let Some(num_str) = s.strip_suffix("MB") {
        num_str.parse::<u64>().ok()
    } else {
        None
    }
}

/// Parse a context length like "4096", "4k", "64k"
pub fn parse_context_length(s: &str) -> Option<u64> {
    let s = s.to_lowercase();
    if let Some(num_str) = s.strip_suffix('k') {
        num_str.parse::<f64>().ok().map(|v| (v * 1024.0) as u64)
    } else if let Some(num_str) = s.strip_suffix('m') {
        num_str.parse::<f64>().ok().map(|v| (v * 1_048_576.0) as u64)
    } else {
        s.parse::<u64>().ok()
    }
}
