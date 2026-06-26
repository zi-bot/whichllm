use crate::hardware::types::{GpuInfo, GpuVendor};

pub fn detect_intel() -> Vec<GpuInfo> {
    #[cfg(target_os = "linux")]
    {
        detect_intel_linux()
    }

    #[cfg(target_os = "windows")]
    {
        detect_intel_windows()
    }

    #[cfg(target_os = "macos")]
    {
        detect_intel_macos()
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        vec![]
    }
}

#[cfg(target_os = "linux")]
fn detect_intel_linux() -> Vec<GpuInfo> {
    // Intel GPU vendor ID = 0x8086
    let mut gpus = vec![];
    let mut card_idx = 0u32;

    loop {
        let card_path = format!("/sys/class/drm/card{card_idx}");
        if !std::path::Path::new(&card_path).exists() {
            break;
        }

        let vendor_path = format!("{card_path}/device/vendor");
        if let Ok(vendor_hex) = std::fs::read_to_string(&vendor_path) {
            if vendor_hex.trim() == "0x8086" {
                let vram_mb = read_sysfs_u64(&format!("{card_path}/device/mem_info_vram_total"))
                    .map(|bytes| bytes / 1_048_576)
                    .unwrap_or(0);

                let name = read_sysfs_str(&format!("{card_path}/device/product_name"))
                    .unwrap_or_else(|| {
                        let dev_id = std::fs::read_to_string(format!("{card_path}/device/device"))
                            .ok()
                            .map(|s| intel_name_from_device_id(s.trim()))
                            .unwrap_or_else(|| "Intel GPU".to_string());
                        dev_id
                    });

                gpus.push(GpuInfo {
                    name,
                    vram_mb: if vram_mb > 0 { vram_mb } else { estimate_intel_vram(&name) },
                    bandwidth_gbps: estimate_intel_bandwidth(&name),
                    vendor: GpuVendor::Intel,
                });
            }
        }

        card_idx += 1;
    }

    // Fallback: xe-smi (Intel GPU Sysman for Arc/BMG)
    if gpus.is_empty() {
        gpus.extend(detect_intel_xe_smi());
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
fn intel_name_from_device_id(dev_id: &str) -> String {
    match dev_id {
        // Arc A-Series
        "0x5690" | "0x5691" | "0x5692" => "Intel Arc A770",
        "0x56A0" | "0x56A1" => "Intel Arc A750",
        "0x56B0" | "0x56B1" => "Intel Arc A580",
        "0x56C0" | "0x56C1" => "Intel Arc A380",
        // Arc B-Series (Battlemage)
        "0xE202" | "0xE20B" => "Intel Arc B580",
        "0xE20C" | "0xE20D" => "Intel Arc B570",
        // Meteor Lake iGPU
        "0x7D55" | "0x7DD5" => "Intel Meteor Lake iGPU",
        // Raptor Lake
        "0xA780" | "0xA788" | "0x4680" | "0x4690" => "Intel UHD Graphics 770",
        "0xA720" | "0xA721" | "0x46A0" | "0x46A2" => "Intel UHD Graphics 730",
        // Iris Xe (Tiger Lake + Alder Lake)
        "0x9A49" | "0x9A59" | "0x9A60" | "0x9A68" | "0x9A70" | "0x9A78" | "0x9A40" | "0x9A48"
        | "0x9C49" | "0x9C59" | "0x9C60" | "0x9C68" => "Intel Iris Xe",
        _ => return format!("Intel GPU ({dev_id})"),
    }.to_string()
}

#[allow(dead_code)]
fn detect_intel_xe_smi() -> Vec<GpuInfo> {
    let output = std::process::Command::new("xe-smi")
        .args(["-d", "1"])
        .output()
        .ok();

    let output = match output {
        Some(o) if o.status.success() => o,
        _ => return vec![],
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut gpus = vec![];

    for line in stdout.lines() {
        let lower = line.to_lowercase();
        if (lower.contains("gpu name") || lower.contains("device name"))
            && let Some(name) = line.split(':').nth(1) {
                let name = name.trim().to_string();
                if !name.is_empty() && !name.to_lowercase().contains("n/a") {
                    gpus.push(GpuInfo {
                        name: name.clone(),
                        vram_mb: estimate_intel_vram(&name),
                        bandwidth_gbps: estimate_intel_bandwidth(&name),
                        vendor: GpuVendor::Intel,
                    });
                }
            }
        // Update VRAM if we find a memory line
        if (lower.contains("vram") || lower.contains("memory"))
            && let Some(last_gpu) = gpus.last_mut() {
                for part in line.split_whitespace() {
                    if let Ok(v) = part.trim_end_matches(',').parse::<u64>()
                        && (100..1_000_000).contains(&v) {
                            last_gpu.vram_mb = v;
                        }
                }
            }
    }

    gpus
}

#[cfg(target_os = "windows")]
fn detect_intel_windows() -> Vec<GpuInfo> {
    let output = std::process::Command::new("powershell")
        .args([
            "-Command",
            "Get-CimInstance Win32_VideoController | Where-Object { $_.AdapterCompatibilityID -match 'Intel' } | Select-Object Name, AdapterRAM | ConvertTo-Json",
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
            let name = entry.get("Name").and_then(|v| v.as_str()).unwrap_or("Intel GPU").to_string();
            let vram_bytes = entry.get("AdapterRAM").and_then(|v| v.as_u64()).unwrap_or(0);
            let vram_mb = vram_bytes / 1_048_576;

            // Only include discrete (Arc) or GPUs with reported VRAM
            let is_discrete = name.to_lowercase().contains("arc");
            if is_discrete || vram_mb > 0 {
                gpus.push(GpuInfo {
                    name,
                    vram_mb: if vram_mb > 0 { vram_mb } else { estimate_intel_vram(&gpus.last().map(|g| g.name.as_str()).unwrap_or("")) },
                    bandwidth_gbps: 0.0,
                    vendor: GpuVendor::Intel,
                });
            }
        }
    }

    for gpu in &mut gpus {
        gpu.bandwidth_gbps = estimate_intel_bandwidth(&gpu.name);
        if gpu.vram_mb == 0 {
            gpu.vram_mb = estimate_intel_vram(&gpu.name);
        }
    }

    gpus
}

#[cfg(target_os = "macos")]
fn detect_intel_macos() -> Vec<GpuInfo> {
    let output = std::process::Command::new("system_profiler")
        .args(["SPDisplaysDataType"])
        .output()
        .ok();

    let stdout = match output {
        Some(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => return vec![],
    };

    let mut found_intel = false;
    let mut name = String::new();
    let mut vram_mb = 0u64;

    for line in stdout.lines() {
        let lower = line.to_lowercase();
        if lower.contains("intel") && (lower.contains("vendor") || lower.contains("chipset") || lower.contains("iris") || lower.contains("uhd") || lower.contains("hd graphics")) {
            found_intel = true;
        }
        if line.contains("Chipset Model")
            && let Some(n) = line.split(':').nth(1) {
                let n = n.trim();
                if n.to_lowercase().contains("intel") {
                    found_intel = true;
                    name = n.to_string();
                }
            }
        if line.contains("VRAM")
            && let Some(v) = line.split(':').nth(1) {
                vram_mb = parse_macos_vram(v.trim());
            }
    }

    if found_intel {
        vec![GpuInfo {
            name: if name.is_empty() { "Intel iGPU".to_string() } else { name.clone() },
            vram_mb,
            bandwidth_gbps: estimate_intel_bandwidth(&name),
            vendor: GpuVendor::Intel,
        }]
    } else {
        vec![]
    }
}

#[cfg(target_os = "macos")]
fn parse_macos_vram(s: &str) -> u64 {
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() >= 2
        && let Ok(num) = parts[0].parse::<u64>() {
            return if parts[1].to_lowercase().starts_with('g') { num * 1024 } else { num };
        }
    0
}

#[allow(dead_code)]
fn estimate_intel_vram(name: &str) -> u64 {
    let lower = name.to_lowercase();
    if lower.contains("arc b580") { 12288 }
    else if lower.contains("arc b570") { 10240 }
    else if lower.contains("arc a770") || lower.contains("arc a750") || lower.contains("arc a580") { 8192 }
    else if lower.contains("arc a380") { 6144 }
    else { 512 } // iGPU shared
}

#[allow(dead_code)]
#[allow(clippy::if_same_then_else)]
pub fn estimate_intel_bandwidth(name: &str) -> f64 {
    let lower = name.to_lowercase();
    if lower.contains("arc a770") || lower.contains("arc a750") || lower.contains("arc a580") { 512.0 }
    else if lower.contains("arc a380") { 288.0 }
    else if lower.contains("arc b580") { 272.0 }
    else if lower.contains("arc b570") { 240.0 }
    else if lower.contains("iris xe") { 68.0 }
    else if lower.contains("uhd") { 50.0 }
    else { 50.0 }
}
