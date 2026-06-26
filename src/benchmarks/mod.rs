pub mod types;
pub mod static_rs;

use types::{BenchmarkEntry, Evidence};
use crate::models::types::ModelInfo;

/// Merge static and live benchmarks into model metadata.
pub fn merge_benchmarks(models: &mut [ModelInfo]) {
    let static_entries = static_rs::load_static();
    let lookup: std::collections::HashMap<&str, &BenchmarkEntry> = static_entries
        .iter()
        .map(|e| (e.model_id.as_str(), e))
        .collect();

    for model in models.iter_mut() {
        // Exact ID match
        if let Some(entry) = lookup.get(model.model_id.as_str()) {
            model.benchmark_score = Some(entry.score);
            model.benchmark_confidence = Some(entry.confidence.weight());
            model.benchmark_source = Some(entry.source.clone());
            continue;
        }

        // Base model match
        if let Some(ref base) = model.base_model {
            if let Some(entry) = lookup.get(base.as_str()) {
                model.benchmark_score = Some(entry.score);
                model.benchmark_confidence = Some(Evidence::BaseModel.weight());
                model.benchmark_source = Some(entry.source.clone());
                continue;
            }
        }

        // Variant: strip -Instruct, -GGUF, etc.
        let stripped = strip_suffixes(&model.model_id);
        if stripped != model.model_id {
            if let Some(entry) = lookup.get(stripped.as_str()) {
                model.benchmark_score = Some(entry.score);
                model.benchmark_confidence = Some(Evidence::Variant.weight());
                model.benchmark_source = Some(entry.source.clone());
                continue;
            }
        }

        // HuggingFace evalResults fallback
        if let Some(ref eval) = model.eval_results {
            if !eval.scores.is_empty() {
                let avg: f64 = eval.scores.iter().map(|(_, s)| s).sum::<f64>() / eval.scores.len() as f64;
                model.benchmark_score = Some(avg);
                model.benchmark_confidence = Some(Evidence::SelfReported.weight());
                model.benchmark_source = Some("huggingface_card".to_string());
            }
        }
    }
}

fn strip_suffixes(id: &str) -> String {
    let mut s = id.to_string();
    loop {
        let before = s.clone();
        for suffix in &["-Instruct", "-GGUF", "-gguf", "-chat", "-Chat"] {
            if s.ends_with(suffix) {
                s = s[..s.len() - suffix.len()].to_string();
            }
        }
        if s == before { break; }
    }
    s
}
