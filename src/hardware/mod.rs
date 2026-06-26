pub mod types;
pub mod nvidia;
#[allow(dead_code)]
pub mod amd;
pub mod apple;
pub mod cpu;
pub mod memory;

use types::{GpuInfo, GpuVendor, HardwareInfo, OsType};

/// Known GPU specs for --gpu simulation
const GPU_TABLE: &[(&str, u64, f64)] = &[
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
        GpuInfo {
            name: if count > 1 {
                format!("{count}x {gpu_name}")
            } else {
                gpu_name.to_string()
            },
            vram_mb: total_vram,
            bandwidth_gbps: *bw * count as f64,
            vendor: GpuVendor::Nvidia,
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

fn auto_detect_gpus() -> Vec<GpuInfo> {
    let mut gpus = vec![];
    gpus.extend(nvidia::detect_nvidia());
    if gpus.is_empty() {
        gpus.extend(amd::detect_amd());
    }
    if gpus.is_empty() {
        gpus.extend(apple::detect_apple());
    }
    gpus
}
