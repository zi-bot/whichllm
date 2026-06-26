mod cli;
mod hardware;
mod models;
mod benchmarks;
mod engine;
mod output;

use clap::Parser;

#[tokio::main]
async fn main() {
    let args = cli::Args::parse();
    let hw = hardware::detect(args.gpu.as_deref());

    if hw.gpus.is_empty() {
        eprintln!("No GPU detected — CPU-only mode");
    } else {
        for gpu in &hw.gpus {
            eprintln!("{} — {} MB VRAM, {:.0} GB/s", gpu.name, gpu.vram_mb, gpu.bandwidth_gbps);
        }
    }
    eprintln!("CPU: {} ({} cores)", hw.cpu.name, hw.cpu.cores);
    eprintln!("RAM: {:.1} GB", hw.ram_gb);
    eprintln!();

    eprintln!("Fetching models from HuggingFace...");
    let client = reqwest::Client::new();
    let mut models = models::fetch_models(&client, args.refresh).await;
    if models.is_empty() {
        eprintln!("Error: no models fetched. Check network or try --refresh.");
        std::process::exit(1);
    }
    eprintln!("Found {} models", models.len());

    benchmarks::merge_benchmarks(&mut models);

    let results = engine::rank(&models, &hw, args.top, args.speed.as_deref());

    if args.json {
        output::print_json(&results);
    } else {
        output::print_ranking(&results);
    }
}
