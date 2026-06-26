use crate::hardware::types::{GpuInfo, GpuVendor};

pub fn detect_nvidia() -> Vec<GpuInfo> {
    let output = std::process::Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,memory.total",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .ok();

    let output = match output {
        Some(o) if o.status.success() => o,
        _ => return vec![],
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() >= 2 {
                let name = parts[0].trim().to_string();
                let vram_mb = parts[1].trim().parse::<u64>().ok()?;
                let bandwidth = estimate_bandwidth(&name);
                Some(GpuInfo {
                    name,
                    vram_mb,
                    bandwidth_gbps: bandwidth,
                    vendor: GpuVendor::Nvidia,
                })
            } else {
                None
            }
        })
        .collect()
}

fn estimate_bandwidth(name: &str) -> f64 {
    let name_lower = name.to_lowercase();
    if name_lower.contains("5090") { 1792.0 }
    else if name_lower.contains("4090") { 1008.0 }
    else if name_lower.contains("3090") { 936.0 }
    else if name_lower.contains("4080") { 716.8 }
    else if name_lower.contains("3080") { 760.0 }
    else if name_lower.contains("5080") { 960.0 }
    else if name_lower.contains("4070") { 504.0 }
    else if name_lower.contains("4060") { 272.0 }
    else if name_lower.contains("3060") { 360.0 }
    else if name_lower.contains("a100") { 2039.0 }
    else if name_lower.contains("h100") { 3352.0 }
    else { 500.0 }
}
