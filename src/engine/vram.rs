use crate::models::types::{ModelInfo, GGUFVariant};
use crate::hardware::types::HardwareInfo;
use super::types::FitType;

const FRAMEWORK_OVERHEAD_MB: u64 = 500;

/// Estimate VRAM required for a specific GGUF variant
pub fn estimate_vram(model: &ModelInfo, variant: &GGUFVariant) -> u64 {
    let params_b = model.params_b.unwrap_or(7.0);

    // If we have a real file size, use it
    if variant.size_bytes > 0 {
        let weights_mb = variant.size_bytes / 1_048_576;
        let kv_mb = (params_b * 0.5 * 4096.0 / 1024.0) as u64;
        return weights_mb + kv_mb + FRAMEWORK_OVERHEAD_MB;
    }

    // Otherwise estimate from params and quant
    // weights_bytes ≈ params × bits_per_weight / 8
    let bits = variant.quant.bits_per_weight();
    let weights_mb = (params_b * 1_000_000_000.0 * bits / 8.0 / 1_048_576.0) as u64;
    let kv_mb = (params_b * 0.5 * 4096.0 / 1024.0) as u64;

    weights_mb + kv_mb + FRAMEWORK_OVERHEAD_MB
}

/// Determine fit type based on available hardware
pub fn fit_type(vram_required_mb: u64, hw: &HardwareInfo) -> FitType {
    let gpu_vram_mb: u64 = hw.gpus.iter().map(|g| g.vram_mb).sum();

    if vram_required_mb <= gpu_vram_mb {
        FitType::FullGpu
    } else if vram_required_mb <= gpu_vram_mb + (hw.ram_gb * 1024.0) as u64 {
        FitType::PartialOffload
    } else {
        FitType::CpuOnly
    }
}
