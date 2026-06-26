use crate::hardware::types::HardwareInfo;
use super::types::FitType;

/// Estimate tokens per second
pub fn estimate_tps(
    vram_required_mb: u64,
    bandwidth_gbps: f64,
    params_b: f64,
    quant_eff: f64,
    fit_type: FitType,
    hw: &HardwareInfo,
) -> f64 {
    if bandwidth_gbps <= 0.0 || params_b <= 0.0 {
        return 0.0;
    }

    let model_size_gb = vram_required_mb as f64 / 1024.0;
    if model_size_gb <= 0.0 {
        return 0.0;
    }

    // Memory bandwidth bound
    let bandwidth_bound = bandwidth_gbps / model_size_gb;
    let base_tps = bandwidth_bound * quant_eff * 2.0;

    let fit_factor = match fit_type {
        FitType::FullGpu => 1.0,
        FitType::PartialOffload => 0.35,
        FitType::CpuOnly => 0.10,
    };

    let tps = base_tps * fit_factor;

    // CPU-only: cap at reasonable maximum
    if fit_type == FitType::CpuOnly {
        tps.min(hw.cpu.cores as f64 * 1.5)
    } else {
        tps
    }
}

/// Quantization efficiency factor
pub fn quant_efficiency(bits_per_weight: f64) -> f64 {
    if bits_per_weight <= 3.0 { 0.85 }
    else if bits_per_weight <= 4.5 { 1.0 }
    else if bits_per_weight <= 6.0 { 0.95 }
    else if bits_per_weight <= 8.0 { 0.90 }
    else { 0.85 }
}
