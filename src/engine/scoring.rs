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
    min_speed: Option<f64>,
    ctx_len: u64,
    gpu_only: bool,
    quant_filter: Option<&str>,
) -> Vec<RankResult> {
    let gpu_bandwidth: f64 = hw.gpus.iter().map(|g| g.bandwidth_gbps).sum();

    let mut results: Vec<RankResult> = vec![];

    for model in models {
        if model.gguf_variants.is_empty() {
            continue;
        }

        for variant in &model.gguf_variants {
            // Quant filter
            if let Some(qf) = quant_filter {
                if variant.quant.display_name() != qf {
                    continue;
                }
            }

            let vram_mb = vram::estimate_vram(model, variant, ctx_len);
            let fit = vram::fit_type(vram_mb, hw);

            // GPU-only filter
            if gpu_only && fit != FitType::FullGpu {
                continue;
            }

            let q_eff = speed::quant_efficiency(variant.quant.bits_per_weight());
            let params_b = model.params_b.unwrap_or(0.0);
            let tps = speed::estimate_tps(vram_mb, gpu_bandwidth, params_b, q_eff, fit, hw);

            // Min speed filter
            if let Some(min) = min_speed {
                if tps < min {
                    continue;
                }
            }

            let score = compute_score(model, variant, fit, tps);
            let marker = score_marker(model);

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

/// Fuzzy-find models matching a query string
pub fn find_model(models: &[ModelInfo], query: &str) -> Vec<ModelInfo> {
    let query_lower = query.to_lowercase().replace(' ', "");
    let mut matches: Vec<(ModelInfo, i32)> = vec![];

    for model in models {
        let id_lower = model.model_id.to_lowercase().replace(' ', "");
        let mut score = 0i32;

        // Exact substring match (highest priority)
        if id_lower.contains(&query_lower) {
            score = 100;
        } else {
            // Check if all words/tokens in query appear in model ID
            let query_tokens = tokenize(&query_lower);
            let id_tokens = tokenize(&id_lower);
            let matched = query_tokens.iter().filter(|qt| {
                id_tokens.iter().any(|it| it.contains(qt.as_str()) || qt.contains(it.as_str()))
            }).count();
            if matched > 0 {
                score = (matched * 100 / query_tokens.len()) as i32;
            }
        }

        if score > 0 {
            matches.push((model.clone(), score));
        }
    }

    // Sort by match score desc, then downloads
    matches.sort_by(|a, b| {
        match b.1.cmp(&a.1) {
            std::cmp::Ordering::Equal => b.0.downloads.cmp(&a.0.downloads),
            other => other,
        }
    });

    matches.into_iter().map(|(m, _)| m).collect()
}

/// Tokenize: split on non-alphanumeric, keep numeric suffixes
fn tokenize(s: &str) -> Vec<String> {
    s.split(&['-', '_', '.', ' '][..])
        .filter(|t| !t.is_empty())
        .map(|t| t.to_string())
        .collect()
}

/// Check if a model matches a profile
pub fn matches_profile(model: &ModelInfo, profile: &str) -> bool {
    let id_lower = model.model_id.to_lowercase();

    match profile {
        "coding" => {
            const PREFIXES: &[&str] = &[
                "deepseek-ai/deepseek-coder",
                "qwen/qwen2.5-coder",
                "microsoft/phi-3",
                "mistralai/codestral",
                "qwen/qwen3-coder",
            ];
            PREFIXES.iter().any(|p| id_lower.starts_with(p))
        }
        "vision" => {
            const PREFIXES: &[&str] = &[
                "llava",
                "qwen/qwen2-vl",
                "google/paligemma",
                "cogvlm",
                "internvl",
            ];
            PREFIXES.iter().any(|p| id_lower.contains(p))
        }
        "math" => {
            const PREFIXES: &[&str] = &[
                "deepseek-ai/deepseek-math",
                "mathstral",
            ];
            PREFIXES.iter().any(|p| id_lower.starts_with(p))
        }
        _ => true, // "general" and unknown pass all
    }
}

fn compute_score(model: &ModelInfo, variant: &GGUFVariant, fit: FitType, tps: f64) -> f64 {
    let benchmark = model.benchmark_score.unwrap_or(30.0);
    let evidence_weight = model.benchmark_confidence.unwrap_or(0.55);

    let params_b = model.params_b.unwrap_or(1.0);
    let size_bonus = (params_b.ln() / 2.0_f64.ln()).min(35.0);

    let quant_penalty = variant.quant.quality_penalty();

    let fit_factor = match fit {
        FitType::FullGpu => 1.0,
        FitType::PartialOffload => 0.72,
        FitType::CpuOnly => 0.50,
    };

    let speed_adj = if tps >= 30.0 { 8.0 }
    else if tps >= 10.0 { 4.0 }
    else if tps >= 4.0 { -2.0 }
    else { -8.0 };

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
