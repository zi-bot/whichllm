use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub model_id: String,
    pub author: String,
    pub downloads: u64,
    pub likes: u64,
    pub params_b: Option<f64>,
    pub architecture: Option<String>,
    pub base_model: Option<String>,
    pub family: Option<String>,
    pub gguf_variants: Vec<GGUFVariant>,
    pub pipeline_tag: Option<String>,
    pub eval_results: Option<EvalResults>,
    pub benchmark_score: Option<f64>,
    pub benchmark_confidence: Option<f64>,
    pub benchmark_source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GGUFVariant {
    pub filename: String,
    pub size_bytes: u64,
    pub quant: QuantType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuantType {
    Q2K,
    Q3KS,
    Q3KM,
    Q3KL,
    Q40,
    Q4KS,
    Q4KM,
    Q50,
    Q5KS,
    Q5KM,
    Q6K,
    Q80,
    Fp16,
    Fp32,
    Unknown,
}

impl QuantType {
    pub fn from_filename(filename: &str) -> Self {
        let lower = filename.to_lowercase();
        if lower.contains("q2k") { Self::Q2K }
        else if lower.contains("q3_k_l") { Self::Q3KL }
        else if lower.contains("q3_k_m") { Self::Q3KM }
        else if lower.contains("q3_k_s") { Self::Q3KS }
        else if lower.contains("q4_k_m") { Self::Q4KM }
        else if lower.contains("q4_k_s") { Self::Q4KS }
        else if lower.contains("q4_0") { Self::Q40 }
        else if lower.contains("q5_k_m") { Self::Q5KM }
        else if lower.contains("q5_k_s") { Self::Q5KS }
        else if lower.contains("q5_0") { Self::Q50 }
        else if lower.contains("q6_k") { Self::Q6K }
        else if lower.contains("q8_0") { Self::Q80 }
        else if lower.contains("fp16") || lower.contains("f16") { Self::Fp16 }
        else if lower.contains("fp32") || lower.contains("f32") { Self::Fp32 }
        else { Self::Unknown }
    }

    pub fn bits_per_weight(&self) -> f64 {
        match self {
            Self::Q2K => 2.56,
            Self::Q3KS => 3.0,
            Self::Q3KM => 3.25,
            Self::Q3KL => 3.5,
            Self::Q40 => 4.0,
            Self::Q4KS => 4.25,
            Self::Q4KM => 4.5,
            Self::Q50 => 5.0,
            Self::Q5KS => 5.25,
            Self::Q5KM => 5.5,
            Self::Q6K => 6.0,
            Self::Q80 => 8.0,
            Self::Fp16 => 16.0,
            Self::Fp32 => 32.0,
            Self::Unknown => 4.5,
        }
    }

    pub fn quality_penalty(&self) -> f64 {
        match self {
            Self::Q2K => 0.60,
            Self::Q3KS | Self::Q3KM | Self::Q3KL => 0.75,
            Self::Q40 | Self::Q4KS | Self::Q4KM => 0.88,
            Self::Q50 | Self::Q5KS | Self::Q5KM => 0.95,
            Self::Q6K => 0.97,
            Self::Q80 => 0.98,
            Self::Fp16 | Self::Fp32 => 1.0,
            Self::Unknown => 0.85,
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            Self::Q2K => "Q2K",
            Self::Q3KS => "Q3KS",
            Self::Q3KM => "Q3KM",
            Self::Q3KL => "Q3KL",
            Self::Q40 => "Q40",
            Self::Q4KS => "Q4KS",
            Self::Q4KM => "Q4KM",
            Self::Q50 => "Q50",
            Self::Q5KS => "Q5KS",
            Self::Q5KM => "Q5KM",
            Self::Q6K => "Q6K",
            Self::Q80 => "Q80",
            Self::Fp16 => "FP16",
            Self::Fp32 => "FP32",
            Self::Unknown => "???",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalResults {
    pub scores: Vec<(String, f64)>,
    pub source: String,
}
