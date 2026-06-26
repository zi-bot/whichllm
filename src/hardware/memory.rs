#[cfg(target_os = "macos")]
pub fn detect_ram_gb() -> f64 {
    let output = std::process::Command::new("sysctl")
        .args(["-n", "hw.memsize"])
        .output()
        .ok();
    output
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
        .map(|bytes| bytes as f64 / 1_073_741_824.0)
        .unwrap_or(0.0)
}

#[cfg(target_os = "linux")]
pub fn detect_ram_gb() -> f64 {
    std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("MemTotal"))
                .and_then(|l| l.split(':').nth(1))
                .and_then(|l| l.trim().split_whitespace().next())
                .and_then(|v| v.parse::<u64>().ok())
                .map(|kb| kb as f64 / 1_048_576.0)
        })
        .unwrap_or(0.0)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn detect_ram_gb() -> f64 {
    0.0
}
