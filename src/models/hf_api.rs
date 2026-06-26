use crate::models::types::{GGUFVariant, QuantType};
use serde::{Deserialize, Serialize};

const HF_API: &str = "https://huggingface.co/api";
const CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(6 * 3600);

/// Default quant variants to assume for GGUF repos
const DEFAULT_QUANTS: &[QuantType] = &[
    QuantType::Q4KM,
    QuantType::Q5KM,
    QuantType::Q80,
];

#[derive(Debug, Deserialize)]
struct HfModel {
    id: String,
    author: Option<String>,
    downloads: u64,
    likes: u64,
    #[serde(default)]
    siblings: Vec<HfSibling>,
    #[serde(default)]
    tags: Vec<String>,
    pipeline_tag: Option<String>,
    #[serde(rename = "cardData", default)]
    card_data: Option<HfCardData>,
}

#[derive(Debug, Deserialize)]
struct HfSibling {
    rfilename: String,
}

#[derive(Debug, Deserialize)]
struct HfCardData {
    #[serde(rename = "base_model", default)]
    base_model: Option<serde_json::Value>,
    #[serde(rename = "evalResults", default)]
    eval_results: Option<serde_json::Value>,
}

/// Processed model row for caching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HfModelRow {
    pub id: String,
    pub author: String,
    pub downloads: u64,
    pub likes: u64,
    pub gguf_variants: Vec<GGUFVariant>,
    pub params_b: Option<f64>,
    pub base_model: Option<String>,
    pub pipeline_tag: Option<String>,
    pub eval_scores: Vec<(String, f64)>,
    pub is_gguf_repo: bool,
}

pub async fn fetch_text_generation(
    client: &reqwest::Client,
    cache: Option<&super::cache::Cache>,
    refresh: bool,
) -> Vec<HfModelRow> {
    let key = "hf_text_generation";
    if !refresh {
        if let Some(c) = cache {
            if let Some(data) = c.get::<Vec<HfModelRow>>(key, CACHE_TTL) {
                return data;
            }
        }
    }

    let url = format!(
        "{HF_API}/models?pipeline_tag=text-generation&sort=downloads&direction=-1&limit=100"
    );
    let result = fetch_with_retry(client, &url).await;

    match result {
        Ok(models) => {
            if let Some(c) = cache {
                c.set(key, &models);
            }
            models
        }
        Err(e) => {
            eprintln!("Warning: HuggingFace API error (text-generation): {e}");
            cache
                .and_then(|c| c.get::<Vec<HfModelRow>>(key, std::time::Duration::MAX))
                .unwrap_or_default()
        }
    }
}

pub async fn fetch_gguf(
    client: &reqwest::Client,
    cache: Option<&super::cache::Cache>,
    refresh: bool,
) -> Vec<HfModelRow> {
    let key = "hf_gguf";
    if !refresh {
        if let Some(c) = cache {
            if let Some(data) = c.get::<Vec<HfModelRow>>(key, CACHE_TTL) {
                return data;
            }
        }
    }

    let url = format!("{HF_API}/models?search=gguf&sort=downloads&direction=-1&limit=100");
    let result = fetch_with_retry(client, &url).await;

    match result {
        Ok(models) => {
            if let Some(c) = cache {
                c.set(key, &models);
            }
            models
        }
        Err(e) => {
            eprintln!("Warning: HuggingFace API error (gguf): {e}");
            cache
                .and_then(|c| c.get::<Vec<HfModelRow>>(key, std::time::Duration::MAX))
                .unwrap_or_default()
        }
    }
}

async fn fetch_with_retry(client: &reqwest::Client, url: &str) -> Result<Vec<HfModelRow>, String> {
    for attempt in 0..3u32 {
        let resp = client.get(url).send().await.map_err(|e| e.to_string())?;
        if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let wait = std::time::Duration::from_secs(2u64.pow(attempt));
            tokio::time::sleep(wait).await;
            continue;
        }
        if !resp.status().is_success() {
            return Err(format!("HTTP {}", resp.status()));
        }
        let raw_models: Vec<HfModel> = resp.json().await.map_err(|e| e.to_string())?;
        let rows: Vec<HfModelRow> = raw_models.into_iter().map(HfModelRow::from_hf).collect();
        return Ok(rows);
    }
    Err("rate limited after 3 retries".to_string())
}

impl HfModelRow {
    fn from_hf(m: HfModel) -> Self {
        let is_gguf_repo = m.id.to_lowercase().contains("gguf");

        // Parse actual siblings if available
        let mut gguf_variants: Vec<GGUFVariant> = m
            .siblings
            .iter()
            .filter(|s| s.rfilename.ends_with(".gguf"))
            .map(|s| {
                let quant = QuantType::from_filename(&s.rfilename);
                GGUFVariant {
                    filename: s.rfilename.clone(),
                    size_bytes: 0, // list API doesn't provide sizes
                    quant,
                }
            })
            .collect();

        // If no siblings but it's a GGUF repo, add default variants
        if gguf_variants.is_empty() && is_gguf_repo {
            for &quant in DEFAULT_QUANTS {
                gguf_variants.push(GGUFVariant {
                    filename: format!("model-{}.gguf", quant.display_name().to_lowercase()),
                    size_bytes: 0,
                    quant,
                });
            }
        }

        // For text-gen (non-GGUF) models with no variants, add Q4_K_M and Q5_K_M
        if gguf_variants.is_empty() && !is_gguf_repo {
            for &quant in &[QuantType::Q4KM, QuantType::Q5KM] {
                gguf_variants.push(GGUFVariant {
                    filename: format!("model-{}.gguf", quant.display_name().to_lowercase()),
                    size_bytes: 0,
                    quant,
                });
            }
        }

        let params_b = extract_params(&m.id, &m.tags);
        let base_model = m
            .card_data
            .as_ref()
            .and_then(|c| c.base_model.as_ref())
            .and_then(|v| {
                if let Some(s) = v.as_str() {
                    Some(s.to_string())
                } else if let Some(arr) = v.as_array() {
                    arr.first().and_then(|s| s.as_str()).map(|s| s.to_string())
                } else {
                    None
                }
            });

        let eval_scores = m
            .card_data
            .as_ref()
            .and_then(|c| c.eval_results.as_ref())
            .and_then(parse_eval_results)
            .unwrap_or_default();

        HfModelRow {
            id: m.id,
            author: m.author.unwrap_or_default(),
            downloads: m.downloads,
            likes: m.likes,
            gguf_variants,
            params_b,
            base_model,
            pipeline_tag: m.pipeline_tag,
            eval_scores,
            is_gguf_repo,
        }
    }
}

fn extract_params(id: &str, tags: &[String]) -> Option<f64> {
    for tag in tags {
        let lower = tag.to_lowercase();
        if lower.contains("billion") || lower.contains("b parameters") {
            if let Some(num) = lower.split_whitespace().next() {
                if let Ok(v) = num.parse::<f64>() {
                    return Some(v);
                }
            }
        }
    }
    let lower = id.to_lowercase();
    for part in lower.split(&['-', '_', '.', ' '][..]) {
        if part.ends_with('b') {
            if let Ok(v) = part.trim_end_matches('b').parse::<f64>() {
                return Some(v);
            }
        }
    }
    None
}

fn parse_eval_results(v: &serde_json::Value) -> Option<Vec<(String, f64)>> {
    let arr = v.as_array()?;
    let mut scores = vec![];
    for entry in arr {
        let task = entry
            .get("task")
            .and_then(|t| t.as_str())
            .unwrap_or("unknown");
        let value = entry.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
        scores.push((task.to_string(), value));
    }
    if scores.is_empty() {
        None
    } else {
        Some(scores)
    }
}
