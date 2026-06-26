pub mod types;
pub mod hf_api;
pub mod gguf;
pub mod cache;

use types::ModelInfo;

pub async fn fetch_models(client: &reqwest::Client, refresh: bool) -> Vec<ModelInfo> {
    let cache = cache::Cache::new();

    let text_gen = hf_api::fetch_text_generation(client, cache.as_ref(), refresh).await;
    let gguf = hf_api::fetch_gguf(client, cache.as_ref(), refresh).await;
    gguf::merge_models(text_gen, gguf)
}
