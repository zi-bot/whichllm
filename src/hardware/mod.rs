pub mod types;
pub mod nvidia;
pub mod amd;
pub mod intel;
pub mod apple;
pub mod cpu;
pub mod memory;

use types::{GpuInfo, GpuVendor, HardwareInfo, OsType};

/// Known GPU specs for --gpu simulation
const GPU_TABLE: &[(&str, u64, f64)] = &[
    // NVIDIA
    ("RTX 5090",     32768, 1792.0),
    ("RTX 5080",     16384,  960.0),
    ("RTX 4090",     24576, 1008.0),
    ("RTX 3090",     24576,  936.0),
    ("RTX 4080",     16384,  716.8),
    ("RTX 3080",     12288,  760.0),
    ("RTX 4070",     12288,  504.0),
    ("RTX 4060",      8192,  272.0),
    ("RTX 3060",     12288,  360.0),
    ("RTX 4060 Ti",  16384,  288.0),
    ("A100 40GB",    40960, 1555.0),
    ("A100 80GB",    81920, 2039.0),
    ("H100",         81920, 3352.0),
    ("L40S",         49152,  864.0),
    // AMD
    ("RX 7900 XTX",  24576,  960.0),
    ("RX 7900 XT",   20480,  800.0),
    ("RX 7900 GRE",  16384,  576.0),
    ("RX 7800 XT",   16384,  576.0),
    ("RX 7700 XT",   12288,  432.0),
    ("RX 7600 XT",    8192,  288.0),
    ("RX 7600",       8192,  288.0),
    ("RX 6950 XT",   16384,  576.0),
    ("RX 6900 XT",   16384,  512.0),
    ("RX 6800 XT",   16384,  512.0),
    ("RX 6800",      16384,  512.0),
    ("RX 6700 XT",   12288,  384.0),
    ("RX 6600 XT",    8192,  288.0),
    ("RX 6600",       8192,  224.0),
    ("MI300X",       196608, 5300.0),
    ("MI250X",       131072, 3277.0),
    ("MI250",        131072, 3277.0),
    ("MI100",        32768,  1024.0),
    // Intel
    ("Arc B580",     12288,  272.0),
    ("Arc B570",     10240,  240.0),
    ("Arc A770",      8192,  512.0),
    ("Arc A750",      8192,  512.0),
    ("Arc A580",      8192,  512.0),
    ("Arc A380",      6144,  288.0),
];

fn simulate_gpu(name: &str) -> Option<GpuInfo> {
    let name_lower = name.to_lowercase();

    // Handle "Nx" prefix for multi-GPU
    let (count, search_name) = if let Some(rest) = name_lower.strip_prefix('2') {
        if rest.starts_with('x') || rest.starts_with('X') {
            (2u32, rest[1..].trim())
        } else {
            (1, name_lower.as_str())
        }
    } else {
        (1, name_lower.as_str())
    };

    let entry = GPU_TABLE.iter().find(|(gpu_name, _, _)| {
        gpu_name.to_lowercase().contains(search_name)
    });

    entry.map(|(gpu_name, vram_mb, bw)| {
        let total_vram = *vram_mb * count as u64;
        let vendor = if gpu_name.starts_with("RX ") || gpu_name.starts_with("MI") {
            GpuVendor::Amd
        } else if gpu_name.starts_with("Arc ") {
            GpuVendor::Intel
        } else {
            GpuVendor::Nvidia
        };
        GpuInfo {
            name: if count > 1 {
                format!("{count}x {gpu_name}")
            } else {
                gpu_name.to_string()
            },
            vram_mb: total_vram,
            bandwidth_gbps: *bw * count as f64,
            vendor,
        }
    })
}

pub fn detect(gpu_override: Option<&str>) -> HardwareInfo {
    let gpus = if let Some(gpu_name) = gpu_override {
        simulate_gpu(gpu_name)
            .map(|g| vec![g])
            .unwrap_or_else(|| {
                eprintln!("Warning: unknown GPU '{gpu_name}', falling back to auto-detect");
                auto_detect_gpus()
            })
    } else {
        auto_detect_gpus()
    };

    let cpu = cpu::detect_cpu();
    let ram_gb = memory::detect_ram_gb();

    let os = if cfg!(target_os = "linux") { OsType::Linux }
    else if cfg!(target_os = "macos") { OsType::MacOS }
    else if cfg!(target_os = "windows") { OsType::Windows }
    else { OsType::Unknown };

    HardwareInfo { gpus, cpu, ram_gb, os }
}

/// Detect all GPUs — collect from all vendors (not just first match)
fn auto_detect_gpus() -> Vec<GpuInfo> {
    let mut gpus = vec![];
    gpus.extend(nvidia::detect_nvidia());
    gpus.extend(amd::detect_amd());
    gpus.extend(intel::detect_intel());
    gpus.extend(apple::detect_apple());

    // Deduplicate by name (some systems may report same GPU via multiple methods)
    let mut seen = std::collections::HashSet::new();
    gpus.retain(|g| seen.insert(g.name.clone()));

    gpus
}
