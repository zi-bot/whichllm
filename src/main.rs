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

    match &args.command {
        Some(cli::Command::Hardware) => {
            cmd_hardware(&args);
            return;
        }
        Some(cli::Command::Plan { model, quant, context_length }) => {
            cmd_plan(&args, model, quant.as_deref(), context_length.as_deref()).await;
            return;
        }
        Some(cli::Command::Upgrade { gpus, top }) => {
            cmd_upgrade(&args, gpus, *top).await;
            return;
        }
        None => {}
    }

    cmd_rank(&args).await;
}

fn cmd_hardware(args: &cli::Args) {
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
    println!("OS: {:?}", hw.os);
}

async fn cmd_plan(args: &cli::Args, model_name: &str, quant: Option<&str>, ctx_len: Option<&str>) {
    let client = reqwest::Client::new();
    let models = models::fetch_models(&client, args.refresh).await;
    let matches = engine::find_model(&models, model_name);
    if matches.is_empty() {
        eprintln!("No models matching '{model_name}' found");
        std::process::exit(1);
    }
    let ctx = ctx_len.and_then(cli::parse_context_length).unwrap_or(4096);
    output::print_plan(&matches, quant, ctx, args.json);
}

async fn cmd_upgrade(args: &cli::Args, gpus: &[String], top: usize) {
    let client = reqwest::Client::new();
    let mut models = models::fetch_models(&client, args.refresh).await;
    benchmarks::merge_benchmarks(&mut models);

    let mut results = vec![];

    // Current machine first
    let hw_current = hardware::detect(args.gpu.as_deref());
    let ranked_current = engine::rank(&models, &hw_current, top, None, 4096, false, None);
    results.push(("Current".to_string(), hw_current, ranked_current));

    for gpu_name in gpus {
        let hw = hardware::detect(Some(gpu_name));
        let ranked = engine::rank(&models, &hw, top, None, 4096, false, None);
        results.push((gpu_name.clone(), hw, ranked));
    }

    output::print_upgrade(&results, args.json);
}

async fn cmd_rank(args: &cli::Args) {
    let mut hw = hardware::detect(args.gpu.as_deref());

    // CPU-only override
    if args.cpu_only {
        hw.gpus.clear();
    }

    // GPU-only / fit filter
    let gpu_only = args.gpu_only
        || args.fit.as_deref() == Some("full-gpu")
        || args.fit.as_deref() == Some("gpu");

    // VRAM headroom
    if let Some(ref headroom) = args.vram_headroom {
        if let Some(mb) = cli::parse_size_mb(headroom) {
            for gpu in &mut hw.gpus {
                gpu.vram_mb = gpu.vram_mb.saturating_sub(mb);
            }
        }
    }

    // RAM budget
    if let Some(ref budget) = args.ram_budget {
        if budget != "available" {
            if let Some(mb) = cli::parse_size_mb(budget) {
                hw.ram_gb = mb as f64 / 1024.0;
            }
        }
    }

    // Context length
    let ctx_len = args
        .context_length
        .as_deref()
        .and_then(cli::parse_context_length)
        .unwrap_or(4096);

    // Print hardware info
    if hw.gpus.is_empty() {
        eprintln!("No GPU detected — CPU-only mode");
    } else {
        for gpu in &hw.gpus {
            eprintln!(
                "{} — {} MB VRAM, {:.0} GB/s",
                gpu.name, gpu.vram_mb, gpu.bandwidth_gbps
            );
        }
    }
    eprintln!("CPU: {} ({} cores)", hw.cpu.name, hw.cpu.cores);
    eprintln!("RAM: {:.1} GB", hw.ram_gb);
    eprintln!();

    // Fetch models
    eprintln!("Fetching models from HuggingFace...");
    let client = reqwest::Client::new();
    let mut models = models::fetch_models(&client, args.refresh).await;
    if models.is_empty() {
        eprintln!("Error: no models fetched. Check network or try --refresh.");
        std::process::exit(1);
    }
    eprintln!("Found {} models", models.len());

    // Merge benchmarks
    benchmarks::merge_benchmarks(&mut models);

    // Profile filter
    if let Some(ref profile) = args.profile {
        let before = models.len();
        models.retain(|m| engine::matches_profile(m, profile));
        eprintln!("Profile '{}' filtered to {} models", profile, models.len());
        if models.len() < before {
            // already logged
        }
    }

    // Min speed (combine --speed and --min-speed)
    let min_speed = args.min_speed.or_else(|| {
        args.speed.as_deref().map(|s| match s {
            "usable" => 10.0,
            "fast" => 30.0,
            _ => 0.0,
        })
    });

    // Quant filter
    let quant_filter = args.quant.as_deref();

    // Rank
    let results = engine::rank(&models, &hw, args.top, min_speed, ctx_len, gpu_only, quant_filter);

    // Output
    if args.json {
        output::print_json(&results);
    } else if args.markdown {
        output::print_markdown(&results);
    } else {
        output::print_ranking(&results);
    }
}
