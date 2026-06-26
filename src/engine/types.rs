use crate::models::types::{ModelInfo, GGUFVariant};

#[derive(Debug, Clone)]
pub struct RankResult {
    pub model: ModelInfo,
    pub variant: GGUFVariant,
    pub score: f64,
    pub vram_required_mb: u64,
    pub fit_type: FitType,
    pub estimated_tps: f64,
    pub score_marker: ScoreMarker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FitType {
    FullGpu,
    PartialOffload,
    CpuOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreMarker {
    None,
    Inferred,
    NoData,
    SelfReported,
}
