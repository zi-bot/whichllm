use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "whichllm", about = "Find the best local LLM for your hardware")]
pub struct Args {
    /// Simulate a specific GPU (e.g. "RTX 4090")
    #[arg(long)]
    pub gpu: Option<String>,

    /// Number of results to show
    #[arg(long, default_value = "10")]
    pub top: usize,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Bypass cache, fetch fresh data
    #[arg(long)]
    pub refresh: bool,

    /// Speed filter: "usable" (>=10 t/s) or "fast" (>=30 t/s)
    #[arg(long)]
    pub speed: Option<String>,
}
