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
    Q2_K,
    Q3_K_S,
    Q3_K_M,
    Q3_K_L,
    Q4_0,
    Q4_K_S,
    Q4_K_M,
    Q5_0,
    Q5_K_S,
    Q5_K_M,
    Q6_K,
    Q8_0,
    Fp16,
    Fp32,
    Unknown,
}

impl QuantType {
    pub fn from_filename(filename: &str) -> Self {
        let lower = filename.to_lowercase();
        if lower.contains("q2_k") { Self::Q2_K }
        else if lower.contains("q3_k_l") { Self::Q3_K_L }
        else if lower.contains("q3_k_m") { Self::Q3_K_M }
        else if lower.contains("q3_k_s") { Self::Q3_K_S }
        else if lower.contains("q4_k_m") { Self::Q4_K_M }
        else if lower.contains("q4_k_s") { Self::Q4_K_S }
        else if lower.contains("q4_0") { Self::Q4_0 }
        else if lower.contains("q5_k_m") { Self::Q5_K_M }
        else if lower.contains("q5_k_s") { Self::Q5_K_S }
        else if lower.contains("q5_0") { Self::Q5_0 }
        else if lower.contains("q6_k") { Self::Q6_K }
        else if lower.contains("q8_0") { Self::Q8_0 }
        else if lower.contains("fp16") || lower.contains("f16") { Self::Fp16 }
        else if lower.contains("fp32") || lower.contains("f32") { Self::Fp32 }
        else { Self::Unknown }
    }

    pub fn bits_per_weight(&self) -> f64 {
        match self {
            Self::Q2_K => 2.56,
            Self::Q3_K_S => 3.0,
            Self::Q3_K_M => 3.25,
            Self::Q3_K_L => 3.5,
            Self::Q4_0 => 4.0,
            Self::Q4_K_S => 4.25,
            Self::Q4_K_M => 4.5,
            Self::Q5_0 => 5.0,
            Self::Q5_K_S => 5.25,
            Self::Q5_K_M => 5.5,
            Self::Q6_K => 6.0,
            Self::Q8_0 => 8.0,
            Self::Fp16 => 16.0,
            Self::Fp32 => 32.0,
            Self::Unknown => 4.5,
        }
    }

    pub fn quality_penalty(&self) -> f64 {
        match self {
            Self::Q2_K => 0.60,
            Self::Q3_K_S | Self::Q3_K_M | Self::Q3_K_L => 0.75,
            Self::Q4_0 | Self::Q4_K_S | Self::Q4_K_M => 0.88,
            Self::Q5_0 | Self::Q5_K_S | Self::Q5_K_M => 0.95,
            Self::Q6_K => 0.97,
            Self::Q8_0 => 0.98,
            Self::Fp16 | Self::Fp32 => 1.0,
            Self::Unknown => 0.85,
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            Self::Q2_K => "Q2_K",
            Self::Q3_K_S => "Q3_K_S",
            Self::Q3_K_M => "Q3_K_M",
            Self::Q3_K_L => "Q3_K_L",
            Self::Q4_0 => "Q4_0",
            Self::Q4_K_S => "Q4_K_S",
            Self::Q4_K_M => "Q4_K_M",
            Self::Q5_0 => "Q5_0",
            Self::Q5_K_S => "Q5_K_S",
            Self::Q5_K_M => "Q5_K_M",
            Self::Q6_K => "Q6_K",
            Self::Q8_0 => "Q8_0",
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
