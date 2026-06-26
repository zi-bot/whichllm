use crate::models::types::{ModelInfo, GGUFVariant};
use crate::hardware::types::HardwareInfo;
use super::types::{FitType, RankResult, ScoreMarker};
use super::vram;
use super::speed;

/// Score and rank models, returning top N results
pub fn rank(
    models: &[ModelInfo],
    hw: &HardwareInfo,
    top: usize,
    speed_filter: Option<&str>,
) -> Vec<RankResult> {
    let gpu_bandwidth: f64 = hw.gpus.iter().map(|g| g.bandwidth_gbps).sum();

    let mut results: Vec<RankResult> = vec![];

    for model in models {
        if model.gguf_variants.is_empty() {
            continue;
        }

        // Try each GGUF variant, pick the best score
        for variant in &model.gguf_variants {
            let vram_mb = vram::estimate_vram(model, variant);
            let fit = vram::fit_type(vram_mb, hw);
            let q_eff = speed::quant_efficiency(variant.quant.bits_per_weight());
            let params_b = model.params_b.unwrap_or(0.0);
            let tps = speed::estimate_tps(vram_mb, gpu_bandwidth, params_b, q_eff, fit, hw);

            let score = compute_score(model, variant, fit, tps);
            let marker = score_marker(model);

            // Speed filter
            if let Some(filter) = speed_filter {
                let min_tps = match filter {
                    "usable" => 10.0,
                    "fast" => 30.0,
                    _ => 0.0,
                };
                if tps < min_tps {
                    continue;
                }
            }

            results.push(RankResult {
                model: model.clone(),
                variant: variant.clone(),
                score,
                vram_required_mb: vram_mb,
                fit_type: fit,
                estimated_tps: tps,
                score_marker: marker,
            });
        }
    }

    // Sort by score descending
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    // Deduplicate by model_id, keeping highest score variant
    let mut seen = std::collections::HashSet::new();
    results.retain(|r| seen.insert(r.model.model_id.clone()));

    results.truncate(top);
    results
}

fn compute_score(model: &ModelInfo, variant: &GGUFVariant, fit: FitType, tps: f64) -> f64 {
    let benchmark = model.benchmark_score.unwrap_or(30.0);
    let evidence_weight = model.benchmark_confidence.unwrap_or(0.55);

    // Size bonus: log2(params_gb), capped at 35
    let params_b = model.params_b.unwrap_or(1.0);
    let size_bonus = (params_b.ln() / 2.0_f64.ln()).min(35.0);

    // Quantization quality penalty
    let quant_penalty = variant.quant.quality_penalty();

    // Fit type factor
    let fit_factor = match fit {
        FitType::FullGpu => 1.0,
        FitType::PartialOffload => 0.72,
        FitType::CpuOnly => 0.50,
    };

    // Speed adjustment: -8 to +8
    let speed_adj = if tps >= 30.0 { 8.0 }
    else if tps >= 10.0 { 4.0 }
    else if tps >= 4.0 { -2.0 }
    else { -8.0 };

    // Source trust bonus
    let trust_adj = if is_official_org(model) { 3.0 } else { 0.0 };

    let score = benchmark * evidence_weight * (1.0 + size_bonus / 100.0)
        * quant_penalty * fit_factor + speed_adj + trust_adj;

    score.clamp(0.0, 100.0)
}

fn score_marker(model: &ModelInfo) -> ScoreMarker {
    if model.benchmark_score.is_none() {
        ScoreMarker::NoData
    } else if let Some(conf) = model.benchmark_confidence {
        if conf <= 0.55 { ScoreMarker::SelfReported }
        else if conf < 1.0 { ScoreMarker::Inferred }
        else { ScoreMarker::None }
    } else {
        ScoreMarker::NoData
    }
}

fn is_official_org(model: &ModelInfo) -> bool {
    let parts: Vec<&str> = model.model_id.split('/').collect();
    parts.len() >= 2 && parts[0] == model.author
}
