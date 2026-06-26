use crate::models::types::ModelInfo;

/// Deduplicate models by ID, preferring the one with more GGUF variants
pub fn merge_models(
    text_gen: Vec<super::hf_api::HfModelRow>,
    gguf: Vec<super::hf_api::HfModelRow>,
) -> Vec<ModelInfo> {
    let mut seen: std::collections::HashMap<String, ModelInfo> = std::collections::HashMap::new();

    for row in text_gen.into_iter().chain(gguf) {
        let entry = seen.entry(row.id.clone()).or_insert_with(|| ModelInfo {
            model_id: row.id.clone(),
            author: row.author.clone(),
            downloads: row.downloads,
            likes: row.likes,
            params_b: row.params_b,
            architecture: None,
            base_model: row.base_model.clone(),
            family: None,
            gguf_variants: vec![],
            pipeline_tag: row.pipeline_tag.clone(),
            eval_results: if row.eval_scores.is_empty() {
                None
            } else {
                Some(super::types::EvalResults {
                    scores: row.eval_scores.clone(),
                    source: "huggingface_card".to_string(),
                })
            },
            benchmark_score: None,
            benchmark_confidence: None,
            benchmark_source: None,
        });

        // Merge GGUF variants (prefer more variants or ones from GGUF repo)
        if row.gguf_variants.len() > entry.gguf_variants.len() || (row.is_gguf_repo && !entry.gguf_variants.is_empty()) {
            entry.gguf_variants = row.gguf_variants;
        }
        entry.downloads = entry.downloads.max(row.downloads);
    }

    let mut models: Vec<ModelInfo> = seen.into_values().collect();
    models.sort_by_key(|b| std::cmp::Reverse(b.downloads));
    models
}
