use owo_colors::OwoColorize;
use crate::engine::types::{FitType, RankResult, ScoreMarker};
use crate::models::types::ModelInfo;
use crate::engine::vram;

// GPU table for plan command
const GPU_TABLE: &[(&str, u64)] = &[
    ("RTX 5090",     32768),
    ("RTX 5080",     16384),
    ("RTX 4090",     24576),
    ("RTX 3090",     24576),
    ("RTX 4080",     16384),
    ("RTX 3080",     12288),
    ("RTX 4070",     12288),
    ("RTX 4060",      8192),
    ("RTX 3060",     12288),
    ("A100 40GB",    40960),
    ("A100 80GB",    81920),
    ("H100",         81920),
    ("L40S",         49152),
];

pub fn print_ranking(results: &[RankResult]) {
    for (i, r) in results.iter().enumerate() {
        let rank = format!("#{}", i + 1);
        let id = &r.model.model_id;
        let params = r.model.params_b.map(|p| format!("{p:.1}B")).unwrap_or_else(|| "?".to_string());
        let quant = r.variant.quant.display_name();
        let score = format!("{:.1}", r.score);
        let fit = match r.fit_type {
            FitType::FullGpu => "GPU".green().to_string(),
            FitType::PartialOffload => "OFFLOAD".yellow().to_string(),
            FitType::CpuOnly => "CPU".red().to_string(),
        };

        let speed_str = format_speed(r.estimated_tps);
        let marker = match r.score_marker {
            ScoreMarker::None => String::new(),
            ScoreMarker::Inferred => " ~".yellow().to_string(),
            ScoreMarker::NoData => " ?".red().to_string(),
            ScoreMarker::SelfReported => " !sr".bright_yellow().to_string(),
        };

        println!("{rank:>4}  {id}  {params}  {quant}  {fit}  score {score}{marker}  {speed_str}");
    }
}

pub fn print_markdown(results: &[RankResult]) {
    println!("| # | Model | Params | Quant | Fit | Score | Speed |");
    println!("|---|-------|--------|-------|-----|-------|-------|");
    for (i, r) in results.iter().enumerate() {
        let params = r.model.params_b.map(|p| format!("{p:.1}B")).unwrap_or_else(|| "?".to_string());
        let fit = match r.fit_type {
            FitType::FullGpu => "GPU",
            FitType::PartialOffload => "OFFLOAD",
            FitType::CpuOnly => "CPU",
        };
        let speed = if r.estimated_tps > 0.0 {
            format!("{:.0} t/s", r.estimated_tps)
        } else {
            "?".to_string()
        };
        let marker = match r.score_marker {
            ScoreMarker::None => "",
            ScoreMarker::Inferred => " ~",
            ScoreMarker::NoData => " ?",
            ScoreMarker::SelfReported => " !sr",
        };
        println!("| {} | {} | {} | {} | {} | {:.1}{} | {} |",
            i + 1, r.model.model_id, params, r.variant.quant.display_name(),
            fit, r.score, marker, speed);
    }
}

pub fn print_json(results: &[RankResult]) {
    let json_results: Vec<serde_json::Value> = results
        .iter()
        .enumerate()
        .map(|(i, r)| {
            serde_json::json!({
                "rank": i + 1,
                "model_id": r.model.model_id,
                "params_b": r.model.params_b,
                "quant": r.variant.quant.display_name(),
                "score": (r.score * 10.0).round() / 10.0,
                "fit_type": match r.fit_type {
                    FitType::FullGpu => "full_gpu",
                    FitType::PartialOffload => "partial_offload",
                    FitType::CpuOnly => "cpu_only",
                },
                "vram_required_mb": r.vram_required_mb,
                "estimated_tps": (r.estimated_tps * 10.0).round() / 10.0,
                "benchmark_source": r.model.benchmark_source,
                "benchmark_confidence": r.model.benchmark_confidence,
            })
        })
        .collect();

    let output = serde_json::json!({ "models": json_results });
    println!("{}", serde_json::to_string_pretty(&output).unwrap_or_default());
}

pub fn print_plan(matches: &[ModelInfo], quant: Option<&str>, ctx_len: u64, json: bool) {
    if matches.is_empty() {
        println!("No matching models found.");
        return;
    }

    if json {
        print_plan_json(matches, quant, ctx_len);
        return;
    }

    for model in matches {
        println!("{} ({})",
            model.model_id.bold(),
            model.params_b.map(|p| format!("{p:.1}B params")).unwrap_or_else(|| "unknown params".to_string())
        );
        println!();

        // Get variants
        let variants: Vec<_> = model.gguf_variants.iter()
            .filter(|v| quant.is_none_or(|q| v.quant.display_name() == q))
            .collect();

        if variants.is_empty() {
            println!("  No GGUF variants available");
            continue;
        }

        println!("  {:12} {:>15}  Fits", "Quant", "VRAM Required");
        println!("  {:12} {:>15}  ----", "-----", "-------------");

        for variant in variants {
            let vram_mb = vram::estimate_vram(model, variant, ctx_len);
            let fitting_gpus: Vec<&str> = GPU_TABLE.iter()
                .filter(|(_, vram)| vram_mb <= *vram)
                .map(|(name, _)| *name)
                .collect();

            let fits_str = if fitting_gpus.is_empty() {
                "None (needs more VRAM)".red().to_string()
            } else {
                fitting_gpus.join(", ").green().to_string()
            };

            println!("  {:12} {:>12} MB  {}",
                variant.quant.display_name(), vram_mb, fits_str);
        }
        println!();
    }
}

fn print_plan_json(matches: &[ModelInfo], quant: Option<&str>, ctx_len: u64) {
    let entries: Vec<serde_json::Value> = matches.iter().map(|model| {
        let variants: Vec<serde_json::Value> = model.gguf_variants.iter()
            .filter(|v| quant.is_none_or(|q| v.quant.display_name() == q))
            .map(|variant| {
                let vram_mb = vram::estimate_vram(model, variant, ctx_len);
                let fitting_gpus: Vec<&str> = GPU_TABLE.iter()
                    .filter(|(_, vram)| vram_mb <= *vram)
                    .map(|(name, _)| *name)
                    .collect();
                serde_json::json!({
                    "quant": variant.quant.display_name(),
                    "vram_required_mb": vram_mb,
                    "fitting_gpus": fitting_gpus,
                })
            })
            .collect();

        serde_json::json!({
            "model_id": model.model_id,
            "params_b": model.params_b,
            "variants": variants,
        })
    }).collect();

    println!("{}", serde_json::to_string_pretty(&serde_json::json!({ "models": entries })).unwrap_or_default());
}

pub fn print_upgrade(results: &[(String, crate::hardware::types::HardwareInfo, Vec<RankResult>)], json: bool) {
    if json {
        print_upgrade_json(results);
        return;
    }

    // Header
    let names: Vec<&str> = results.iter().map(|(name, _, _)| name.as_str()).collect();
    let header = names.iter().map(|n| format!("{n:>25}")).collect::<Vec<_>>().join("");
    println!("{:>25}{}", "", header);

    // VRAM row
    let vram_row: Vec<String> = results.iter().map(|(_, hw, _)| {
        let vram: u64 = hw.gpus.iter().map(|g| g.vram_mb).sum();
        if vram > 0 { format!("{vram:>25} MB") } else { format!("{:>25}", "CPU-only") }
    }).collect();
    println!("{:>25}{}", "VRAM", vram_row.join(""));

    // BW row
    let bw_row: Vec<String> = results.iter().map(|(_, hw, _)| {
        let bw: f64 = hw.gpus.iter().map(|g| g.bandwidth_gbps).sum();
        if bw > 0.0 { format!("{bw:>25.0} GB/s") } else { format!("{:>25}", "-") }
    }).collect();
    println!("{:>25}{}", "Bandwidth", bw_row.join(""));

    // Top model per GPU
    for idx in 0..results.iter().map(|(_, _, r)| r.len()).max().unwrap_or(0) {
        let row: Vec<String> = results.iter().map(|(_, _, ranked)| {
            if let Some(r) = ranked.get(idx) {
                let short_id = r.model.model_id.split('/').next_back().unwrap_or(&r.model.model_id);
                format!("{short_id} ({:.1})", r.score)
            } else {
                String::new()
            }
        }).collect();
        let label = format!("Top #{}", idx + 1);
        let formatted: Vec<String> = row.iter().map(|s| format!("{s:>25}")).collect();
        println!("{label:>25}{}", formatted.join(""));
    }

    // Speed row
    let speed_row: Vec<String> = results.iter().map(|(_, _, ranked)| {
        if let Some(r) = ranked.first() {
            if r.estimated_tps > 0.0 { format!("{:.0} t/s", r.estimated_tps) } else { "?".to_string() }
        } else {
            "-".to_string()
        }
    }).collect();
    let speed_formatted: Vec<String> = speed_row.iter().map(|s| format!("{s:>25}")).collect();
    println!("{:>25}{}", "Top speed", speed_formatted.join(""));
}

fn print_upgrade_json(results: &[(String, crate::hardware::types::HardwareInfo, Vec<RankResult>)]) {
    let entries: Vec<serde_json::Value> = results.iter().map(|(name, hw, ranked)| {
        let vram: u64 = hw.gpus.iter().map(|g| g.vram_mb).sum();
        let bw: f64 = hw.gpus.iter().map(|g| g.bandwidth_gbps).sum();
        let top: Vec<serde_json::Value> = ranked.iter().take(3).map(|r| {
            serde_json::json!({
                "model_id": r.model.model_id,
                "score": r.score,
                "estimated_tps": r.estimated_tps,
            })
        }).collect();
        serde_json::json!({
            "name": name,
            "vram_mb": vram,
            "bandwidth_gbps": bw,
            "top_models": top,
        })
    }).collect();

    println!("{}", serde_json::to_string_pretty(&serde_json::json!({ "upgrade_comparison": entries })).unwrap_or_default());
}

fn format_speed(tps: f64) -> String {
    if tps <= 0.0 {
        return "? t/s".to_string();
    }
    let rounded = if tps >= 100.0 { format!("{tps:.0}") } else { format!("{tps:.1}") };
    if tps >= 30.0 { format!("{rounded} t/s").bright_green().to_string() }
    else if tps >= 10.0 { format!("{rounded} t/s").green().to_string() }
    else if tps >= 4.0 { format!("{rounded} t/s").yellow().to_string() }
    else { format!("{rounded} t/s").red().to_string() }
}
