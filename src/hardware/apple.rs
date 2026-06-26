use crate::hardware::types::{GpuInfo, GpuVendor};

#[cfg(target_os = "macos")]
pub fn detect_apple() -> Vec<GpuInfo> {
    let output = std::process::Command::new("system_profiler")
        .args(["SPDisplaysDataType"])
        .output()
        .ok();

    let stdout = match output {
        Some(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => return vec![],
    };

    let mem_output = std::process::Command::new("sysctl")
        .args(["-n", "hw.memsize"])
        .output()
        .ok();

    let vram_mb = mem_output
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
        .map(|bytes| bytes / 1_048_576)
        .unwrap_or(0);

    let name = stdout
        .lines()
        .find(|l| l.contains("Chipset Model"))
        .and_then(|l| l.split(':').nth(1))
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "Apple Silicon".to_string());

    if vram_mb > 0 {
        vec![GpuInfo {
            name,
            vram_mb,
            bandwidth_gbps: estimate_apple_bandwidth(vram_mb),
            vendor: GpuVendor::Apple,
        }]
    } else {
        vec![]
    }
}

#[cfg(not(target_os = "macos"))]
pub fn detect_apple() -> Vec<GpuInfo> {
    vec![]
}

fn estimate_apple_bandwidth(vram_mb: u64) -> f64 {
    let vram_gb = vram_mb as f64 / 1024.0;
    if vram_gb >= 128.0 { 400.0 }
    else if vram_gb >= 64.0 { 400.0 }
    else if vram_gb >= 36.0 { 300.0 }
    else if vram_gb >= 18.0 { 200.0 }
    else { 100.0 }
}
