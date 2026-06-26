mod cli;
mod hardware;
mod models;
mod benchmarks;

use clap::Parser;

#[tokio::main]
async fn main() {
    let args = cli::Args::parse();
    let hw = hardware::detect(args.gpu.as_deref());

    if hw.gpus.is_empty() {
        println!("No GPU detected — CPU-only mode");
    } else {
        for gpu in &hw.gpus {
            println!("{} — {} MB VRAM, {:.0} GB/s", gpu.name, gpu.vram_mb, gpu.bandwidth_gbps);
        }
    }
    println!("CPU: {} ({} cores)", hw.cpu.name, hw.cpu.cores);
    println!("RAM: {:.1} GB", hw.ram_gb);
    println!();

    let client = reqwest::Client::new();
    let mut models = models::fetch_models(&client, args.refresh).await;
    benchmarks::merge_benchmarks(&mut models);

    let with_bench = models.iter().filter(|m| m.benchmark_score.is_some()).count();
    println!("Fetched {} models ({} with benchmarks)", models.len(), with_bench);
}
