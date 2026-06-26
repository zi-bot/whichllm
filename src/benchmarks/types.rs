use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkEntry {
    pub model_id: String,
    pub score: f64,
    pub source: String,
    pub confidence: Evidence,
    pub date: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Evidence {
    Direct = 5,
    Variant = 4,
    BaseModel = 3,
    Interpolated = 2,
    SelfReported = 1,
}

impl Evidence {
    pub fn weight(&self) -> f64 {
        match self {
            Self::Direct => 1.0,
            Self::Variant => 0.88,
            Self::BaseModel => 0.78,
            Self::Interpolated => 0.65,
            Self::SelfReported => 0.55,
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "direct" => Self::Direct,
            "variant" => Self::Variant,
            "base_model" => Self::BaseModel,
            "interpolated" => Self::Interpolated,
            "self_reported" => Self::SelfReported,
            _ => Self::SelfReported,
        }
    }
}
