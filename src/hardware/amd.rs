use crate::hardware::types::{GpuInfo, GpuVendor};

pub fn detect_amd() -> Vec<GpuInfo> {
    #[cfg(target_os = "linux")]
    {
        detect_amd_linux()
    }

    #[cfg(target_os = "windows")]
    {
        // Windows: use wmic or PowerShell
        detect_amd_windows()
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        vec![]
    }
}

#[cfg(target_os = "linux")]
fn detect_amd_linux() -> Vec<GpuInfo> {
    // Try ROCm sysfs first: /sys/class/drm/card*/device/vendor
    // AMD vendor ID = 0x1002
    let mut gpus = vec![];
    let mut card_idx = 0u32;

    loop {
        let card_path = format!("/sys/class/drm/card{card_idx}");
        if !std::path::Path::new(&card_path).exists() {
            break;
        }

        let vendor_path = format!("{card_path}/device/vendor");
        if let Ok(vendor_hex) = std::fs::read_to_string(&vendor_path) {
            let vendor_hex = vendor_hex.trim();
            if vendor_hex == "0x1002" {
                // AMD GPU found — read VRAM and name
                let vram_mb = read_amd_vram_mb(&card_path);
                let name = read_amd_name(&card_path);
                let bandwidth = estimate_amd_bandwidth(&name);
                gpus.push(GpuInfo {
                    name,
                    vram_mb,
                    bandwidth_gbps: bandwidth,
                    vendor: GpuVendor::Amd,
                });
            }
        }

        card_idx += 1;
    }

    // Fallback: try rocm-smi if sysfs found nothing
    if gpus.is_empty() {
        gpus.extend(detect_amd_rocm_smi());
    }

    gpus
}

#[cfg(target_os = "linux")]
fn read_amd_vram_mb(card_path: &str) -> u64 {
    // /sys/class/drm/cardX/device/mem_info_vram_total (bytes)
    let vram_path = format!("{card_path}/device/mem_info_vram_total");
    if let Ok(bytes_str) = std::fs::read_to_string(&vram_path) {
        if let Ok(bytes) = bytes_str.trim().parse::<u64>() {
            return bytes / 1_048_576;
        }
    }

    // Fallback: mem_info_vis_vram_total
    let vis_path = format!("{card_path}/device/mem_info_vis_vram_total");
    if let Ok(bytes_str) = std::fs::read_to_string(&vis_path) {
        if let Ok(bytes) = bytes_str.trim().parse::<u64>() {
            return bytes / 1_048_576;
        }
    }

    0
}

#[cfg(target_os = "linux")]
fn read_amd_name(card_path: &str) -> String {
    // Try metrics file for marketing name
    let name_path = format!("{card_path}/device/metrics/market_name");
    if let Ok(name) = std::fs::read_to_string(&name_path) {
        let trimmed = name.trim().to_string();
        if !trimmed.is_empty() {
            return trimmed;
        }
    }

    // Try product_name
    let prod_path = format!("{card_path}/device/product_name");
    if let Ok(name) = std::fs::read_to_string(&prod_path) {
        let trimmed = name.trim().to_string();
        if !trimmed.is_empty() {
            return trimmed;
        }
    }

    format!("AMD GPU ({}{})",
        card_path.rsplit('/').next().unwrap_or("card?"),
        ""
    )
}

fn detect_amd_rocm_smi() -> Vec<GpuInfo> {
    let output = std::process::Command::new("rocm-smi")
        .args(["--showmeminfo", "vram", "--showproductname"])
        .output()
        .ok();

    let output = match output {
        Some(o) if o.status.success() => o,
        _ => return vec![],
    };

    // rocm-smi output is text-based, parse VRAM lines
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut gpus = vec![];
    let mut current_name = String::new();
    let mut current_vram = 0u64;

    for line in stdout.lines() {
        let lower = line.to_lowercase();
        if lower.contains("product name") || lower.contains("gpu id") {
            // Save previous GPU if we have data
            if !current_name.is_empty() && current_vram > 0 {
                gpus.push(GpuInfo {
                    name: current_name.clone(),
                    vram_mb: current_vram,
                    bandwidth_gbps: estimate_amd_bandwidth(&current_name),
                    vendor: GpuVendor::Amd,
                });
            }
            current_name = String::new();
            current_vram = 0;
        }
        // Extract name
        if lower.contains("card") && line.contains(':') {
            if let Some(name) = line.split(':').nth(1) {
                current_name = name.trim().to_string();
            }
        }
        // Extract VRAM (Total: XXXX MIB or similar)
        if lower.contains("total") {
            // Try to find a number followed by MIB/MB/GB
            for part in line.split_whitespace() {
                if let Ok(v) = part.trim_end_matches(',').parse::<u64>() {
                    if v > 100 && v < 1_000_000 {
                        // Could be MB
                        current_vram = v / 1024; // if in KiB
                        if v < 500_000 {
                            current_vram = v; // already in MB
                        }
                    }
                }
            }
        }
    }

    // Push last GPU
    if !current_name.is_empty() && current_vram > 0 {
        gpus.push(GpuInfo {
            name: current_name,
            vram_mb: current_vram,
            bandwidth_gbps: 0.0,
            vendor: GpuVendor::Amd,
        });
    }

    gpus
}

#[cfg(target_os = "windows")]
fn detect_amd_windows() -> Vec<GpuInfo> {
    let output = std::process::Command::new("wmic")
        .args([
            "path", "win32_VideoController",
            "where", "AdapterCompatibilityID='AMD,ATI'",
            "get", "Name,AdapterRAM",
            "/format:csv",
        ])
        .output()
        .ok();

    let output = match output {
        Some(o) if o.status.success() => o,
        _ => {
            // Fallback: try PowerShell
            return detect_amd_powershell();
        }
    };

    parse_wmic_amd_output(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(target_os = "windows")]
fn detect_amd_powershell() -> Vec<GpuInfo> {
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
    // Parse JSON output
    let mut gpus = vec![];
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&stdout) {
        let entries = if let Some(arr) = val.as_array() { arr.clone() } else { vec![val] };
        for entry in entries {
            let name = entry.get("Name").and_then(|v| v.as_str()).unwrap_or("AMD GPU").to_string();
            let vram_bytes = entry.get("AdapterRAM").and_then(|v| v.as_u64()).unwrap_or(0);
            let vram_mb = vram_bytes / 1_048_576;
            gpus.push(GpuInfo {
                name,
                vram_mb,
                bandwidth_gbps: 0.0,
                vendor: GpuVendor::Amd,
            });
        }
    }
    gpus
}

#[cfg(target_os = "windows")]
fn parse_wmic_amd_output(stdout: &str) -> Vec<GpuInfo> {
    let mut gpus = vec![];
    for line in stdout.lines().skip(1) { // skip header
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() >= 3 {
            let name = parts.get(parts.len() - 2).unwrap_or(&"").trim().to_string();
            let vram_str = parts.last().unwrap_or(&"0").trim();
            let vram_mb = vram_str.parse::<u64>().unwrap_or(0) / 1_048_576;
            if !name.is_empty() && name.to_lowercase().contains("amd") || name.to_lowercase().contains("radeon") {
                gpus.push(GpuInfo {
                    name,
                    vram_mb,
                    bandwidth_gbps: estimate_amd_bandwidth(&gpus.last().map(|g| g.name.as_str()).unwrap_or("")),
                    vendor: GpuVendor::Amd,
                });
            }
        }
    }
    // Fix: bandwidth should use the right name. Re-estimate.
    for gpu in &mut gpus {
        gpu.bandwidth_gbps = estimate_amd_bandwidth(&gpu.name);
    }
    gpus
}

fn estimate_amd_bandwidth(name: &str) -> f64 {
    let lower = name.to_lowercase();
    // AMD Radeon RX series
    if lower.contains("rx 7900 xtx") { 960.0 }
    else if lower.contains("rx 7900 xt") { 800.0 }
    else if lower.contains("rx 7900 gre") { 576.0 }
    else if lower.contains("rx 7800 xt") { 576.0 }
    else if lower.contains("rx 7700 xt") { 432.0 }
    else if lower.contains("rx 7600 xt") { 288.0 }
    else if lower.contains("rx 7600") { 288.0 }
    else if lower.contains("rx 6950 xt") { 576.0 }
    else if lower.contains("rx 6900 xt") { 512.0 }
    else if lower.contains("rx 6800 xt") { 512.0 }
    else if lower.contains("rx 6800") { 512.0 }
    else if lower.contains("rx 6700 xt") { 384.0 }
    else if lower.contains("rx 6600 xt") { 288.0 }
    else if lower.contains("rx 6600") { 224.0 }
    // AMD Instinct (MI-series)
    else if lower.contains("mi300x") { 5300.0 }
    else if lower.contains("mi300a") { 5300.0 }
    else if lower.contains("mi250x") { 3277.0 }
    else if lower.contains("mi250") { 3277.0 }
    else if lower.contains("mi210") { 1638.0 }
    else if lower.contains("mi100") { 1024.0 }
    else { 400.0 } // generic estimate
}
