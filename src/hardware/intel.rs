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
        // macOS: Intel iGPUs on older Macs show up via system_profiler
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
    // Check /sys/class/drm/card*/device/vendor
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
                let vram_mb = read_intel_vram_mb(&card_path);
                let name = read_intel_name(&card_path);
                let bandwidth = estimate_intel_bandwidth(&name);
                gpus.push(GpuInfo {
                    name,
                    vram_mb,
                    bandwidth_gbps: bandwidth,
                    vendor: GpuVendor::Intel,
                });
            }
        }

        card_idx += 1;
    }

    // Fallback: check for xe-smi (Intel GPU Sysman Interface for Arc/BMG)
    if gpus.is_empty() {
        gpus.extend(detect_intel_xe_smi());
    }

    gpus
}

#[cfg(target_os = "linux")]
fn read_intel_vram_mb(card_path: &str) -> u64 {
    // Intel i915 exposes VRAM via mem_info_vram_total (discrete Arc GPUs)
    let vram_path = format!("{card_path}/device/mem_info_vram_total");
    if let Ok(bytes_str) = std::fs::read_to_string(&vram_path) {
        if let Ok(bytes) = bytes_str.trim().parse::<u64>() {
            if bytes > 0 {
                return bytes / 1_048_576;
            }
        }
    }

    // iGPU: VRAM is stolen from system RAM, check stolen memory
    let stolen_path = format!("{card_path}/device/i915_vbt_data");
    // ponytail: iGPU stolen memory isn't easily exposed via sysfs. Use memory as VRAM.
    0
}

#[cfg(target_os = "linux")]
fn read_intel_name(card_path: &str) -> String {
    // Try product_name
    let prod_path = format!("{card_path}/device/product_name");
    if let Ok(name) = std::fs::read_to_string(&prod_path) {
        let trimmed = name.trim().to_string();
        if !trimmed.is_empty() {
            return trimmed;
        }
    }

    // Parse from drm card path + device id
    let dev_path = format!("{card_path}/device/device");
    if let Ok(dev_id) = std::fs::read_to_string(&dev_path) {
        return intel_name_from_device_id(dev_id.trim());
    }

    format!("Intel GPU")
}

fn intel_name_from_device_id(dev_id: &str) -> String {
    // Common Intel GPU device IDs
    match dev_id {
        // Arc A-Series (Discrete)
        "0x5690" | "0x5691" | "0x5692" => "Intel Arc A770".to_string(),
        "0x56A0" | "0x56A1" => "Intel Arc A750".to_string(),
        "0x56B0" | "0x56B1" => "Intel Arc A580".to_string(),
        "0x56C0" | "0x56C1" => "Intel Arc A380".to_string(),
        // Arc B-Series (Battlemage)
        "0xE202" | "0xE20B" => "Intel Arc B580".to_string(),
        "0xE20C" | "0xE20D" => "Intel Arc B570".to_string(),
        // Meteor Lake iGPU
        "0x7D55" | "0x7DD5" => "Intel Meteor Lake iGPU".to_string(),
        // Raptor Lake iGPU
        "0xA780" | "0xA788" => "Intel UHD Graphics 770".to_string(),
        "0xA720" | "0xA721" => "Intel UHD Graphics 730".to_string(),
        // Alder Lake iGPU
        "0x4680" | "0x4690" => "Intel UHD Graphics 770".to_string(),
        "0x46A0" | "0x46A2" => "Intel UHD Graphics 730".to_string(),
        // Iris Xe
        "0x9A49" | "0x9A59" | "0x9A60" | "0x9A68" | "0x9A70" | "0x9A78" |
        "0x9A40" | "0x9A48" => "Intel Iris Xe".to_string(),
        // Tiger Lake
        "0x9C49" | "0x9C59" | "0x9C60" | "0x9C68" => "Intel Iris Xe".to_string(),
        _ => format!("Intel GPU ({dev_id})"),
    }
}

fn detect_intel_xe_smi() -> Vec<GpuInfo> {
    let output = std::process::Command::new("xe-smi")
        .args(["-d", "1"]) // device info
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
        // xe-smi output lines like: "GPU Device ID: 0x5690"
        // and "GPU Name: Intel Arc A770"
        if lower.contains("gpu name") || lower.contains("device name") {
            if let Some(name) = line.split(':').nth(1) {
                let name = name.trim().to_string();
                if !name.is_empty() && !name.to_lowercase().contains("n/a") {
                    gpus.push(GpuInfo {
                        name: name.clone(),
                        vram_mb: 0, // will try to read from VRAM line
                        bandwidth_gbps: estimate_intel_bandwidth(&name),
                        vendor: GpuVendor::Intel,
                    });
                }
            }
        }
        // VRAM line: "VRAM: 8192 MB"
        if lower.contains("vram") || lower.contains("memory") {
            if let Some(last_gpu) = gpus.last_mut() {
                for part in line.split_whitespace() {
                    if let Ok(v) = part.trim_end_matches(',').parse::<u64>() {
                        if v > 100 && v < 1_000_000 {
                            last_gpu.vram_mb = v;
                        }
                    }
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
        let entries = if let Some(arr) = val.as_array() { arr.clone() } else { vec![val] };
        for entry in entries {
            let name = entry.get("Name").and_then(|v| v.as_str()).unwrap_or("Intel GPU").to_string();
            let vram_bytes = entry.get("AdapterRAM").and_then(|v| v.as_u64()).unwrap_or(0);
            let vram_mb = vram_bytes / 1_048_576;

            // Filter: only Intel discrete GPUs (Arc series) or notable iGPUs
            // Skip generic "Intel UHD" or low VRAM iGPUs unless they have significant VRAM
            let is_discrete = name.to_lowercase().contains("arc") || name.to_lowercase().contains("battlemage");
            let has_vram = vram_mb > 0;

            if is_discrete || has_vram {
                gpus.push(GpuInfo {
                    name,
                    vram_mb: if vram_mb == 0 { estimate_intel_vram(&gpus.last().map(|g: &GpuInfo| g.name.as_str()).unwrap_or("")) } else { vram_mb },
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
    // Older Intel Macs have Intel iGPUs — check system_profiler
    let output = std::process::Command::new("system_profiler")
        .args(["SPDisplaysDataType"])
        .output()
        .ok();

    let stdout = match output {
        Some(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => return vec![],
    };

    let mut gpus = vec![];

    // Look for Intel vendor or Intel GPU names
    let mut found_intel = false;
    let mut name = String::new();
    let mut vram_mb = 0u64;

    for line in stdout.lines() {
        let lower = line.to_lowercase();
        if lower.contains("intel") && (lower.contains("vendor") || lower.contains("chipset") || lower.contains("iriris") || lower.contains("uhd") || lower.contains("hd graphics")) {
            found_intel = true;
        }
        if line.contains("Chipset Model") {
            if let Some(n) = line.split(':').nth(1) {
                let n = n.trim().to_string();
                if n.to_lowercase().contains("intel") {
                    found_intel = true;
                    name = n;
                }
            }
        }
        if line.contains("VRAM") {
            if let Some(v) = line.split(':').nth(1) {
                let v = v.trim();
                // Parse "XXX MB" or "XXX GB"
                if let Some(mb) = parse_macos_vram(v) {
                    vram_mb = mb;
                }
            }
        }
    }

    if found_intel {
        gpus.push(GpuInfo {
            name: if name.is_empty() { "Intel iGPU".to_string() } else { name.clone() },
            vram_mb,
            bandwidth_gbps: estimate_intel_bandwidth(&name),
            vendor: GpuVendor::Intel,
        });
    }

    gpus
}

#[cfg(target_os = "macos")]
fn parse_macos_vram(s: &str) -> Option<u64> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() >= 2 {
        let num = parts[0].parse::<u64>().ok()?;
        if parts[1].to_lowercase().starts_with('g') {
            return Some(num * 1024);
        }
        return Some(num);
    }
    None
}

fn estimate_intel_vram(name: &str) -> u64 {
    let lower = name.to_lowercase();
    if lower.contains("arc a770") { 8192 }
    else if lower.contains("arc a750") { 8192 }
    else if lower.contains("arc a580") { 8192 }
    else if lower.contains("arc a380") { 6144 }
    else if lower.contains("arc b580") { 12288 }
    else if lower.contains("arc b570") { 10240 }
    else { 512 } // iGPU: small shared memory
}

fn estimate_intel_bandwidth(name: &str) -> f64 {
    let lower = name.to_lowercase();
    // Arc A-Series
    if lower.contains("arc a770") { 512.0 }
    else if lower.contains("arc a750") { 512.0 }
    else if lower.contains("arc a580") { 512.0 }
    else if lower.contains("arc a380") { 288.0 }
    // Arc B-Series (Battlemage)
    else if lower.contains("arc b580") { 272.0 }
    else if lower.contains("arc b570") { 240.0 }
    // iGPUs: very low bandwidth for LLM inference
    else if lower.contains("iris xe") { 68.0 }
    else if lower.contains("uhd") { 50.0 }
    else { 50.0 }
}
