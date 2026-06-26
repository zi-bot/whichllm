use crate::hardware::types::{GpuInfo, GpuVendor};

pub fn detect_amd() -> Vec<GpuInfo> {
    #[cfg(target_os = "linux")]
    {
        detect_amd_linux()
    }

    #[cfg(target_os = "windows")]
    {
        detect_amd_windows()
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        vec![]
    }
}

#[cfg(target_os = "linux")]
fn detect_amd_linux() -> Vec<GpuInfo> {
    // ROCm sysfs: /sys/class/drm/card*/device/vendor (AMD = 0x1002)
    let mut gpus = vec![];
    let mut card_idx = 0u32;

    loop {
        let card_path = format!("/sys/class/drm/card{card_idx}");
        if !std::path::Path::new(&card_path).exists() {
            break;
        }

        let vendor_path = format!("{card_path}/device/vendor");
        if let Ok(vendor_hex) = std::fs::read_to_string(&vendor_path) {
            if vendor_hex.trim() == "0x1002" {
                let vram_mb = read_sysfs_u64(&format!("{card_path}/device/mem_info_vram_total"))
                    .or_else(|| read_sysfs_u64(&format!("{card_path}/device/mem_info_vis_vram_total")))
                    .map(|bytes| bytes / 1_048_576)
                    .unwrap_or(0);

                let name = read_sysfs_str(&format!("{card_path}/device/metrics/market_name"))
                    .or_else(|| read_sysfs_str(&format!("{card_path}/device/product_name")))
                    .unwrap_or_else(|| format!("AMD GPU ({})", card_path.rsplit('/').next().unwrap_or("?")));

                gpus.push(GpuInfo {
                    name,
                    vram_mb,
                    bandwidth_gbps: estimate_amd_bandwidth(&name),
                    vendor: GpuVendor::Amd,
                });
            }
        }

        card_idx += 1;
    }

    // Fallback: rocm-smi
    if gpus.is_empty() {
        gpus.extend(detect_amd_rocm_smi());
    }

    gpus
}

#[cfg(target_os = "linux")]
fn read_sysfs_u64(path: &str) -> Option<u64> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

#[cfg(target_os = "linux")]
fn read_sysfs_str(path: &str) -> Option<String> {
    let s = std::fs::read_to_string(path).ok()?;
    let trimmed = s.trim().to_string();
    if trimmed.is_empty() { None } else { Some(trimmed) }
}

#[allow(dead_code)]
fn detect_amd_rocm_smi() -> Vec<GpuInfo> {
    let output = std::process::Command::new("rocm-smi")
        .args(["--showmeminfo", "vram", "--showproductname"])
        .output()
        .ok();

    let output = match output {
        Some(o) if o.status.success() => o,
        _ => return vec![],
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut gpus = vec![];
    let mut current_name = String::new();
    let mut current_vram_mb = 0u64;

    for line in stdout.lines() {
        let lower = line.to_lowercase();
        if lower.contains("product name") || lower.contains("gpu id") {
            if !current_name.is_empty() && current_vram_mb > 0 {
                gpus.push(GpuInfo {
                    name: current_name.clone(),
                    vram_mb: current_vram_mb,
                    bandwidth_gbps: estimate_amd_bandwidth(&current_name),
                    vendor: GpuVendor::Amd,
                });
            }
            current_name.clear();
            current_vram_mb = 0;
        }
        if lower.contains("card") && line.contains(':')
            && let Some(name) = line.split(':').nth(1) {
                current_name = name.trim().to_string();
            }
        if lower.contains("total") {
            for part in line.split_whitespace() {
                if let Ok(v) = part.trim_end_matches(',').parse::<u64>()
                    && (100..1_000_000).contains(&v) {
                        // If >500k assume KiB, else MB
                        current_vram_mb = if v >= 500_000 { v / 1024 } else { v };
                    }
            }
        }
    }

    if !current_name.is_empty() && current_vram_mb > 0 {
        gpus.push(GpuInfo {
            name: current_name,
            vram_mb: current_vram_mb,
            bandwidth_gbps: 0.0,
            vendor: GpuVendor::Amd,
        });
    }

    gpus
}

#[cfg(target_os = "windows")]
fn detect_amd_windows() -> Vec<GpuInfo> {
    // Try PowerShell first (more reliable JSON output)
    let output = std::process::Command::new("powershell")
        .args([
            "-Command",
            "Get-CimInstance Win32_VideoController | Where-Object { $_.AdapterCompatibilityID -match 'AMD|ATI' } | Select-Object Name, AdapterRAM | ConvertTo-Json",
        ])
        .output()
        .ok();

    let output = match output {
        Some(o) if o.status.success() => o,
        _ => return vec![],
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut gpus = vec![];

    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&stdout) {
        let entries = val.as_array().cloned().unwrap_or_else(|| vec![val]);
        for entry in entries {
            let name = entry.get("Name").and_then(|v| v.as_str()).unwrap_or("AMD GPU").to_string();
            let vram_bytes = entry.get("AdapterRAM").and_then(|v| v.as_u64()).unwrap_or(0);
            gpus.push(GpuInfo {
                name,
                vram_mb: vram_bytes / 1_048_576,
                bandwidth_gbps: 0.0, // estimated below
                vendor: GpuVendor::Amd,
            });
        }
    }

    // Estimate bandwidth from names
    for gpu in &mut gpus {
        gpu.bandwidth_gbps = estimate_amd_bandwidth(&gpu.name);
    }

    gpus
}

#[allow(dead_code)]
#[allow(clippy::if_same_then_else)]
pub fn estimate_amd_bandwidth(name: &str) -> f64 {
    let lower = name.to_lowercase();
    if lower.contains("rx 7900 xtx") { 960.0 }
    else if lower.contains("rx 7900 xt") { 800.0 }
    else if lower.contains("rx 7900 gre") { 576.0 }
    else if lower.contains("rx 7800 xt") { 576.0 }
    else if lower.contains("rx 7700 xt") { 432.0 }
    else if lower.contains("rx 7600 xt") || lower.contains("rx 7600") { 288.0 }
    else if lower.contains("rx 6950 xt") { 576.0 }
    else if lower.contains("rx 6900 xt") { 512.0 }
    else if lower.contains("rx 6800 xt") || lower.contains("rx 6800") { 512.0 }
    else if lower.contains("rx 6700 xt") { 384.0 }
    else if lower.contains("rx 6600 xt") { 288.0 }
    else if lower.contains("rx 6600") { 224.0 }
    else if lower.contains("mi300x") || lower.contains("mi300a") { 5300.0 }
    else if lower.contains("mi250x") || lower.contains("mi250") { 3277.0 }
    else if lower.contains("mi210") { 1638.0 }
    else if lower.contains("mi100") { 1024.0 }
    else { 400.0 }
}
