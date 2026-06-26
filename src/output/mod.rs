use owo_colors::OwoColorize;
use crate::engine::types::{FitType, RankResult, ScoreMarker};

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
