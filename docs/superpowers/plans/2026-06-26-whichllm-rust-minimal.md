# whichllm Rust v1 Minimal Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Rust CLI that auto-detects hardware, fetches models from HuggingFace, ranks by benchmark quality + VRAM fit, and prints a colored ranked list.

**Architecture:** Single binary crate with modules: hardware, models, engine, benchmarks, output. Async with tokio for HTTP. Static curated benchmarks merged with live HuggingFace evalResults. One-pass data flow: detect → fetch → merge → rank → print.

**Tech Stack:** Rust 2024 edition, clap (derive), reqwest (json, rustls-tls), tokio (rt, macros), serde + serde_json, owo-colors, dirs, chrono

## Global Constraints

- Rust edition 2024, MSRV 1.85+
- Dependencies: clap, reqwest, tokio, serde, serde_json, owo-colors, dirs, chrono
- Cache dir: `dirs::cache_dir()/whichllm/`
- HF API base: `https://huggingface.co/api/`
- Static benchmarks bundled via `include_str!`

---

### Task 1: Project scaffold + CLI args

**Files:**
- Modify: `Cargo.toml`
- Create: `src/cli.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Produces: `cli::Args` struct with fields: gpu: Option<String>, top: usize, json: bool, refresh: bool, speed: Option<String>

- [ ] **Step 1: Update Cargo.toml with all dependencies**

```toml
[package]
name = "whichllm"
version = "0.1.0"
edition = "2024"

[dependencies]
clap = { version = "4", features = ["derive"] }
reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }
tokio = { version = "1", features = ["rt", "macros"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
owo-colors = "4"
dirs = "6"
chrono = { version = "0.4", features = ["serde"] }
```

- [ ] **Step 2: Create src/cli.rs**

```rust
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "whichllm", about = "Find the best local LLM for your hardware")]
pub struct Args {
    /// Simulate a specific GPU (e.g. "RTX 4090")
    #[arg(long)]
    pub gpu: Option<String>,

    /// Number of results to show
    #[arg(long, default_value = "10")]
    pub top: usize,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Bypass cache, fetch fresh data
    #[arg(long)]
    pub refresh: bool,

    /// Speed filter: "usable" (>=10 t/s) or "fast" (>=30 t/s)
    #[arg(long)]
    pub speed: Option<String>,
}
```

- [ ] **Step 3: Update src/main.rs**

```rust
mod cli;

fn main() {
    let args = cli::Args::parse();
    println!("whichllm — top {} results", args.top);
}
```

- [ ] **Step 4: Build and verify**

Run: `cargo build`
Expected: compiles successfully

- [ ] **Step 5: Test CLI parsing**

Run: `cargo run -- --top 5`
Expected: prints "whichllm — top 5 results"

Run: `cargo run -- --gpu "RTX 4090" --json`
Expected: prints "whichllm — top 10 results"

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock src/cli.rs src/main.rs
git commit -m "feat: scaffold project with clap CLI args"
```

---

### Task 2: Hardware types + detection

**Files:**
- Create: `src/hardware/mod.rs`
- Create: `src/hardware/types.rs`
- Create: `src/hardware/nvidia.rs`
- Create: `src/hardware/amd.rs`
- Create: `src/hardware/apple.rs`
- Create: `src/hardware/cpu.rs`
- Create: `src/hardware/memory.rs`

**Interfaces:**
- Produces: `hardware::detect(gpu_override: Option<&str>) -> HardwareInfo`, `HardwareInfo`, `GpuInfo`, `CpuInfo`

- [ ] **Step 1: Create src/hardware/types.rs**

```rust
#[derive(Debug, Clone)]
pub struct GpuInfo {
    pub name: String,
    pub vram_mb: u64,
    pub bandwidth_gbps: f64,
    pub vendor: GpuVendor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Apple,
    Intel,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct CpuInfo {
    pub name: String,
    pub cores: usize,
    pub avx2: bool,
    pub avx512: bool,
}

#[derive(Debug, Clone)]
pub struct HardwareInfo {
    pub gpus: Vec<GpuInfo>,
    pub cpu: CpuInfo,
    pub ram_gb: f64,
    pub disk_free_gb: f64,
    pub os: OsType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsType {
    Linux,
    MacOS,
    Windows,
    Unknown,
}
```

- [ ] **Step 2: Create src/hardware/cpu.rs**

```rust
use crate::hardware::types::CpuInfo;

pub fn detect_cpu() -> CpuInfo {
    let name = std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("model name"))
                .and_then(|l| l.split(':').nth(1))
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_else(|| "Unknown CPU".to_string());

    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    CpuInfo {
        name,
        cores,
        avx2: true,   // assume modern x86
        avx512: false, // conservative
    }
}
```

- [ ] **Step 3: Create src/hardware/memory.rs**

```rust
pub fn detect_ram_gb() -> f64 {
    #[cfg(target_os = "linux")]
    {
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

    #[cfg(target_os = "macos")]
    {
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

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        0.0
    }
}

pub fn detect_disk_free_gb() -> f64 {
    dirs::cache_dir()
        .and_then(|d| d.ancestors().nth(1).map(|p| p.to_path_buf()))
        .map(|p| {
            // ponytail: simple statfs, use std::fs::metadata as approximation
            fs4::free_space(&p).unwrap_or(0) as f64 / 1_073_741_824.0
        })
        .unwrap_or(0.0)
}
```

Wait — `fs4` is not in our dependencies. Use a simpler approach.

Create src/hardware/memory.rs (revised):

```rust
pub fn detect_ram_gb() -> f64 {
    #[cfg(target_os = "linux")]
    {
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

    #[cfg(target_os = "macos")]
    {
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

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        0.0
    }
}

pub fn detect_disk_free_gb() -> f64 {
    // ponytail: rough estimate, skip precise statfs. Add fs4 crate if needed later.
    100.0
}
```

- [ ] **Step 4: Create src/hardware/nvidia.rs**

```rust
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
    // ponytail: rough bandwidth lookup for common GPUs. Expand table later.
    let name_lower = name.to_lowercase();
    if name_lower.contains("4090") { 1008.0 }
    else if name_lower.contains("3090") { 936.0 }
    else if name_lower.contains("4080") { 716.8 }
    else if name_lower.contains("3080") { 760.0 }
    else if name_lower.contains("5090") { 1792.0 }
    else if name_lower.contains("a100") { 2039.0 }
    else if name_lower.contains("h100") { 3352.0 }
    else { 500.0 } // generic estimate
}
```

- [ ] **Step 5: Create src/hardware/amd.rs**

```rust
use crate::hardware::types::{GpuInfo, GpuVendor};

pub fn detect_amd() -> Vec<GpuInfo> {
    // ponytail: ROCm detection via sysfs. Stub for now, expand for Linux AMD.
    let _ = GpuVendor::Amd;
    vec![]
}
```

- [ ] **Step 6: Create src/hardware/apple.rs**

```rust
use crate::hardware::types::{GpuInfo, GpuVendor};

pub fn detect_apple() -> Vec<GpuInfo> {
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("system_profiler")
            .args(["SPDisplaysDataType"])
            .output()
            .ok();

        let stdout = match output {
            Some(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
            _ => return vec![],
        };

        // Extract chip name and unified memory
        let mem_output = std::process::Command::new("sysctl")
            .args(["-n", "hw.memsize"])
            .output()
            .ok();

        let vram_mb = mem_output
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| s.trim().parse::<u64>().ok())
            .map(|bytes| bytes / 1_048_576)
            .unwrap_or(0);

        // Try to find GPU name in system_profiler output
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
    {
        vec![]
    }
}

fn estimate_apple_bandwidth(vram_mb: u64) -> f64 {
    // ponytail: rough Apple unified memory bandwidth. M-series varies 100-400 GB/s.
    let vram_gb = vram_mb as f64 / 1024.0;
    if vram_gb >= 64.0 { 400.0 }
    else if vram_gb >= 36.0 { 300.0 }
    else if vram_gb >= 18.0 { 200.0 }
    else { 100.0 }
}
```

- [ ] **Step 7: Create src/hardware/mod.rs with GPU simulator table + detect()**

```rust
pub mod types;
pub mod nvidia;
pub mod amd;
pub mod apple;
pub mod cpu;
pub mod memory;

use types::{GpuInfo, GpuVendor, CpuInfo, HardwareInfo, OsType};

/// Known GPU specs for --gpu simulation
const GPU_TABLE: &[(&str, u64, f64)] = &[
    ("RTX 5090",  32768, 1792.0),
    ("RTX 4090",  24576, 1008.0),
    ("RTX 3090",  24576,  936.0),
    ("RTX 4080",  16384,  716.8),
    ("RTX 3080",  12288,  760.0),
    ("RTX 4070",  12288,  504.0),
    ("RTX 4060",   8192,  272.0),
    ("RTX 3060",  12288,  360.0),
    ("A100",      81920, 2039.0),
    ("A100 80GB", 81920, 2039.0),
    ("H100",      81920, 3352.0),
    ("L40S",      49152,  864.0),
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
            bandwidth_gbps: *bw * count as f64, // ponytail: sum bandwidth for multi-GPU
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
    let disk_free_gb = memory::detect_disk_free_gb();

    let os = if cfg!(target_os = "linux") { OsType::Linux }
    else if cfg!(target_os = "macos") { OsType::MacOS }
    else if cfg!(target_os = "windows") { OsType::Windows }
    else { OsType::Unknown };

    HardwareInfo { gpus, cpu, ram_gb, disk_free_gb, os }
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
```

- [ ] **Step 8: Add macos conditional for apple.rs fields**

Update `src/hardware/apple.rs` — the `estimate_apple_bandwidth` function is defined inside the macos cfg block. Move it outside or gate it properly. Actually, looking at the code, `estimate_apple_bandwidth` is used inside the macos block and defined after it. On non-macos, it won't be called. Add `#[allow(dead_code)]` or gate it:

```rust
#[cfg(target_os = "macos")]
fn estimate_apple_bandwidth(vram_mb: u64) -> f64 {
```

The existing code already has `#[cfg(target_os = "macos")]` on the outer block and the helper function is outside it but only called inside. Gate the helper too:

Actually, restructure `src/hardware/apple.rs` fully:

```rust
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
    if vram_gb >= 64.0 { 400.0 }
    else if vram_gb >= 36.0 { 300.0 }
    else if vram_gb >= 18.0 { 200.0 }
    else { 100.0 }
}
```

- [ ] **Step 9: Update main.rs to use hardware module**

```rust
mod cli;
mod hardware;

fn main() {
    let args = cli::Args::parse();
    let hw = hardware::detect(args.gpu.as_deref());
    if hw.gpus.is_empty() {
        println!("No GPU detected — CPU-only mode");
    } else {
        for gpu in &hw.gpus {
            println!("{} — {} MB VRAM, {:.0} GB/s", gpu.name, gpu.vram_mb, gpu.bandwidth_gbps);
        }
    }
    println!("CPU: {} ({} cores)", hw.cpu.name, hw.cpu.cores);
    println!("RAM: {:.1} GB", hw.ram_gb);
}
```

- [ ] **Step 10: Build and test**

Run: `cargo build`
Expected: compiles

Run: `cargo run`
Expected: prints hardware info for current machine

Run: `cargo run -- --gpu "RTX 4090"`
Expected: prints "RTX 4090 — 24576 MB VRAM, 1008 GB/s"

- [ ] **Step 11: Commit**

```bash
git add src/hardware/ src/main.rs
git commit -m "feat: hardware detection (NVIDIA, AMD stub, Apple, CPU, memory)"
```

---

### Task 3: Model types + HuggingFace API fetching

**Files:**
- Create: `src/models/mod.rs`
- Create: `src/models/types.rs`
- Create: `src/models/hf_api.rs`
- Create: `src/models/gguf.rs`
- Create: `src/models/cache.rs`

**Interfaces:**
- Consumes: `models::cache::Cache` for HTTP response caching
- Produces: `models::fetch_models(client: &reqwest::Client, refresh: bool) -> Vec<ModelInfo>`, `ModelInfo`, `GGUFVariant`, `QuantType`

- [ ] **Step 1: Create src/models/types.rs**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub model_id: String,
    pub author: String,
    pub downloads: u64,
    pub likes: u64,
    pub params_b: Option<f64>,        // billions of parameters
    pub architecture: Option<String>,  // e.g. "llama", "qwen2"
    pub base_model: Option<String>,
    pub family: Option<String>,
    pub gguf_variants: Vec<GGUFVariant>,
    pub eval_results: Option<EvalResults>,
    pub pipeline_tag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GGUFVariant {
    pub filename: String,
    pub size_bytes: u64,
    pub quant: QuantType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuantType {
    Q2_K,
    Q3_K_S,
    Q3_K_M,
    Q3_K_L,
    Q4_0,
    Q4_K_S,
    Q4_K_M,
    Q5_0,
    Q5_K_S,
    Q5_K_M,
    Q6_K,
    Q8_0,
    Fp16,
    Fp32,
    Unknown,
}

impl QuantType {
    pub fn from_filename(filename: &str) -> Self {
        let lower = filename.to_lowercase();
        if lower.contains("q2_k") { Self::Q2_K }
        else if lower.contains("q3_k_l") { Self::Q3_K_L }
        else if lower.contains("q3_k_m") { Self::Q3_K_M }
        else if lower.contains("q3_k_s") { Self::Q3_K_S }
        else if lower.contains("q4_k_m") { Self::Q4_K_M }
        else if lower.contains("q4_k_s") { Self::Q4_K_S }
        else if lower.contains("q4_0") { Self::Q4_0 }
        else if lower.contains("q5_k_m") { Self::Q5_K_M }
        else if lower.contains("q5_k_s") { Self::Q5_K_S }
        else if lower.contains("q5_0") { Self::Q5_0 }
        else if lower.contains("q6_k") { Self::Q6_K }
        else if lower.contains("q8_0") { Self::Q8_0 }
        else if lower.contains("fp16") || lower.contains("f16") { Self::Fp16 }
        else if lower.contains("fp32") || lower.contains("f32") { Self::Fp32 }
        else { Self::Unknown }
    }

    pub fn bits_per_weight(&self) -> f64 {
        match self {
            Self::Q2_K => 2.56,
            Self::Q3_K_S => 3.0,
            Self::Q3_K_M => 3.25,
            Self::Q3_K_L => 3.5,
            Self::Q4_0 => 4.0,
            Self::Q4_K_S => 4.25,
            Self::Q4_K_M => 4.5,
            Self::Q5_0 => 5.0,
            Self::Q5_K_S => 5.25,
            Self::Q5_K_M => 5.5,
            Self::Q6_K => 6.0,
            Self::Q8_0 => 8.0,
            Self::Fp16 => 16.0,
            Self::Fp32 => 32.0,
            Self::Unknown => 4.5, // assume Q4_K_M-ish
        }
    }

    pub fn quality_penalty(&self) -> f64 {
        match self {
            Self::Q2_K => 0.60,
            Self::Q3_K_S | Self::Q3_K_M | Self::Q3_K_L => 0.75,
            Self::Q4_0 | Self::Q4_K_S | Self::Q4_K_M => 0.88,
            Self::Q5_0 | Self::Q5_K_S | Self::Q5_K_M => 0.95,
            Self::Q6_K => 0.97,
            Self::Q8_0 => 0.98,
            Self::Fp16 | Self::Fp32 => 1.0,
            Self::Unknown => 0.85,
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            Self::Q2_K => "Q2_K",
            Self::Q3_K_S => "Q3_K_S",
            Self::Q3_K_M => "Q3_K_M",
            Self::Q3_K_L => "Q3_K_L",
            Self::Q4_0 => "Q4_0",
            Self::Q4_K_S => "Q4_K_S",
            Self::Q4_K_M => "Q4_K_M",
            Self::Q5_0 => "Q5_0",
            Self::Q5_K_S => "Q5_K_S",
            Self::Q5_K_M => "Q5_K_M",
            Self::Q6_K => "Q6_K",
            Self::Q8_0 => "Q8_0",
            Self::Fp16 => "FP16",
            Self::Fp32 => "FP32",
            Self::Unknown => "???",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalResults {
    pub scores: Vec<(String, f64)>,  // (benchmark_name, score)
    pub source: String,
}
```

- [ ] **Step 2: Create src/models/cache.rs**

```rust
use std::path::PathBuf;
use std::time::{Duration, SystemTime};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct CacheEntry<T> {
    data: T,
    timestamp: u64,  // unix seconds
}

pub struct Cache {
    dir: PathBuf,
}

impl Cache {
    pub fn new() -> Option<Self> {
        let base = dirs::cache_dir()?;
        let dir = base.join("whichllm");
        std::fs::create_dir_all(&dir).ok()?;
        Some(Self { dir })
    }

    pub fn get<T: for<'de> Deserialize<'de>>(&self, key: &str, ttl: Duration) -> Option<T> {
        let path = self.dir.join(format!("{key}.json"));
        let raw = std::fs::read_to_string(&path).ok()?;
        let entry: CacheEntry<T> = serde_json::from_str(&raw).ok()?;
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .ok()?
            .as_secs();
        if now.saturating_sub(entry.timestamp) > ttl.as_secs() {
            return None;  // expired
        }
        Some(entry.data)
    }

    pub fn set<T: Serialize>(&self, key: &str, data: &T) {
        let path = self.dir.join(format!("{key}.json"));
        let entry = CacheEntry {
            data,
            timestamp: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };
        if let Ok(json) = serde_json::to_string(&entry) {
            let _ = std::fs::write(&path, json);
        }
    }
}
```

- [ ] **Step 3: Create src/models/hf_api.rs**

```rust
use crate::models::types::{ModelInfo, GGUFVariant, QuantType, EvalResults};
use crate::models::cache::Cache;
use std::time::Duration;

const HF_API: &str = "https://huggingface.co/api";
const CACHE_TTL: Duration = Duration::from_secs(6 * 3600); // 6 hours

#[derive(Debug, serde::Deserialize)]
struct HfModel {
    id: String,
    author: Option<String>,
    downloads: u64,
    likes: u64,
    #[serde(default)]
    siblings: Vec<HfSibling>,
    #[serde(default)]
    tags: Vec<String>,
    pipeline_tag: Option<String>,
    #[serde(rename = "model-index")]
    model_index: Option<serde_json::Value>,
    #[serde(rename = "cardData")]
    card_data: Option<HfCardData>,
}

#[derive(Debug, serde::Deserialize)]
struct HfSibling {
    rfilename: String,
    #[serde(default)]
    size: u64,
}

#[derive(Debug, serde::Deserialize)]
struct HfCardData {
    #[serde(rename = "base_model")]
    base_model: Option<serde_json::Value>,
    #[serde(rename = "evalResults")]
    eval_results: Option<serde_json::Value>,
    language: Option<serde_json::Value>,
}

pub async fn fetch_text_generation(client: &reqwest::Client, cache: &Cache, refresh: bool) -> Vec<HfModelRow> {
    let key = "hf_text_generation";
    if !refresh {
        if let Some(data) = cache.get::<Vec<HfModelRow>>(key, CACHE_TTL) {
            return data;
        }
    }

    let url = format!("{HF_API}/models?pipeline_tag=text-generation&sort=downloads&direction=-1&limit=100");
    let result = fetch_with_retry(client, &url).await;

    match result {
        Ok(models) => {
            cache.set(key, &models);
            models
        }
        Err(e) => {
            eprintln!("Warning: HuggingFace API error (text-generation): {e}");
            cache.get::<Vec<HfModelRow>>(key, Duration::from_secs(u64::MAX)).unwrap_or_default()
        }
    }
}

pub async fn fetch_gguf(client: &reqwest::Client, cache: &Cache, refresh: bool) -> Vec<HfModelRow> {
    let key = "hf_gguf";
    if !refresh {
        if let Some(data) = cache.get::<Vec<HfModelRow>>(key, CACHE_TTL) {
            return data;
        }
    }

    let url = format!("{HF_API}/models?search=gguf&sort=downloads&direction=-1&limit=100");
    let result = fetch_with_retry(client, &url).await;

    match result {
        Ok(models) => {
            cache.set(key, &models);
            models
        }
        Err(e) => {
            eprintln!("Warning: HuggingFace API error (gguf): {e}");
            cache.get::<Vec<HfModelRow>>(key, Duration::from_secs(u64::MAX)).unwrap_or_default()
        }
    }
}

async fn fetch_with_retry(client: &reqwest::Client, url: &str) -> Result<Vec<HfModelRow>, String> {
    for attempt in 0..3 {
        let resp = client.get(url).send().await.map_err(|e| e.to_string())?;
        if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let wait = std::time::Duration::from_secs(2u64.pow(attempt));
            tokio::time::sleep(wait).await;
            continue;
        }
        let models: Vec<HfModelRow> = resp.json().await.map_err(|e| e.to_string())?;
        return Ok(models);
    }
    Err("rate limited after 3 retries".to_string())
}

/// Processed model row (after parsing HF API response)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HfModelRow {
    pub id: String,
    pub author: String,
    pub downloads: u64,
    pub likes: u64,
    pub gguf_variants: Vec<GGUFVariant>,
    pub params_b: Option<f64>,
    pub base_model: Option<String>,
    pub pipeline_tag: Option<String>,
    pub eval_scores: Vec<(String, f64)>,
}

impl From<HfModel> for HfModelRow {
    fn from(m: HfModel) -> Self {
        let gguf_variants: Vec<GGUFVariant> = m.siblings.iter()
            .filter(|s| s.rfilename.ends_with(".gguf"))
            .map(|s| GGUFVariant {
                filename: s.rfilename.clone(),
                size_bytes: s.size,
                quant: QuantType::from_filename(&s.rfilename),
            })
            .collect();

        let params_b = extract_params(&m.id, &m.tags);
        let base_model = m.card_data.as_ref()
            .and_then(|c| c.base_model.clone())
            .and_then(|v| {
                // base_model can be a string or an array like ["org/model", "main"]
                if let Some(s) = v.as_str() {
                    Some(s.to_string())
                } else if let Some(arr) = v.as_array() {
                    arr.first().and_then(|s| s.as_str()).map(|s| s.to_string())
                } else {
                    None
                }
            });

        let eval_scores = m.card_data.as_ref()
            .and_then(|c| c.eval_results.as_ref())
            .and_then(|v| parse_eval_results(v))
            .unwrap_or_default();

        HfModelRow {
            id: m.id,
            author: m.author.unwrap_or_default(),
            downloads: m.downloads,
            likes: m.likes,
            gguf_variants,
            params_b,
            base_model,
            pipeline_tag: m.pipeline_tag,
            eval_scores,
        }
    }
}

fn extract_params(id: &str, tags: &[String]) -> Option<f64> {
    // Try to find in tags like "x billion parameters"
    for tag in tags {
        let lower = tag.to_lowercase();
        if lower.contains("billion") || lower.contains("b parameters") {
            if let Some(num) = lower.split_whitespace().next() {
                if let Ok(v) = num.parse::<f64>() {
                    return Some(v);
                }
            }
        }
    }
    // Try model ID: "Qwen2.5-7B", "llama-3-8b"
    let lower = id.to_lowercase();
    // Look for patterns like 7b, 70b, 0.5b, 1.5b
    for part in lower.split(&['-', '_', '.', ' ']) {
        if part.ends_with('b') {
            if let Ok(v) = part.trim_end_matches('b').parse::<f64>() {
                return Some(v);
            }
        }
    }
    None
}

fn parse_eval_results(v: &serde_json::Value) -> Option<Vec<(String, f64)>> {
    // evalResults is typically an array of {task, dataset, value}
    let arr = v.as_array()?;
    let mut scores = vec![];
    for entry in arr {
        let task = entry.get("task").and_then(|t| t.as_str()).unwrap_or("unknown");
        let value = entry.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
        scores.push((task.to_string(), value));
    }
    if scores.is_empty() { None } else { Some(scores) }
}
```

- [ ] **Step 4: Create src/models/gguf.rs**

```rust
use crate::models::types::{ModelInfo, GGUFVariant};

/// Deduplicate models by ID, preferring the one with more GGUF variants
pub fn merge_models(text_gen: Vec<super::hf_api::HfModelRow>, gguf: Vec<super::hf_api::HfModelRow>) -> Vec<ModelInfo> {
    let mut seen: std::collections::HashMap<String, ModelInfo> = std::collections::HashMap::new();

    for row in text_gen.into_iter().chain(gguf.into_iter()) {
        let entry = seen.entry(row.id.clone()).or_insert_with(|| ModelInfo {
            model_id: row.id.clone(),
            author: row.author.clone(),
            downloads: row.downloads,
            likes: row.likes,
            params_b: row.params_b,
            architecture: None,
            base_model: row.base_model.clone(),
            family: None,
            gguf_variants: vec![],
            eval_results: if row.eval_scores.is_empty() {
                None
            } else {
                Some(super::types::EvalResults {
                    scores: row.eval_scores.clone(),
                    source: "huggingface_card".to_string(),
                })
            },
            pipeline_tag: row.pipeline_tag.clone(),
        });

        // Merge GGUF variants (prefer more variants)
        if row.gguf_variants.len() > entry.gguf_variants.len() {
            entry.gguf_variants = row.gguf_variants;
        }
        // Take higher download count
        entry.downloads = entry.downloads.max(row.downloads);
    }

    let mut models: Vec<ModelInfo> = seen.into_values().collect();
    models.sort_by(|a, b| b.downloads.cmp(&a.downloads));
    models
}
```

- [ ] **Step 5: Create src/models/mod.rs**

```rust
pub mod types;
pub mod hf_api;
pub mod gguf;
pub mod cache;

use types::ModelInfo;

pub async fn fetch_models(client: &reqwest::Client, refresh: bool) -> Vec<ModelInfo> {
    let cache = match cache::Cache::new() {
        Some(c) => c,
        None => {
            eprintln!("Warning: cache dir unavailable, fetching without cache");
            // proceed without cache
            let text_gen = hf_api::fetch_text_generation(client, &no_cache(), refresh).await;
            let gguf = hf_api::fetch_gguf(client, &no_cache(), refresh).await;
            return gguf::merge_models(text_gen, gguf);
        }
    };

    let text_gen = hf_api::fetch_text_generation(client, &cache, refresh).await;
    let gguf = hf_api::fetch_gguf(client, &cache, refresh).await;
    gguf::merge_models(text_gen, gguf)
}

/// A no-op cache for when dirs::cache_dir is unavailable
struct NoCache;

impl NoCache {
    fn new() -> Self { Self }
}

fn no_cache() -> cache::Cache {
    // ponytail: Cache::new() always returns Some in practice; this path is unreachable.
    // If we need a true no-op cache, add a Cache::noop() constructor later.
    panic!("no-op cache not implemented")
}
```

Wait — `no_cache()` panics. Bad. Let me restructure `Cache` to handle the None case better. Add `Cache::noop()` or make fetch work without cache.

Revised `src/models/mod.rs`:

```rust
pub mod types;
pub mod hf_api;
pub mod gguf;
pub mod cache;

use types::ModelInfo;

pub async fn fetch_models(client: &reqwest::Client, refresh: bool) -> Vec<ModelInfo> {
    let cache = cache::Cache::new();

    let text_gen = hf_api::fetch_text_generation(client, cache.as_ref(), refresh).await;
    let gguf = hf_api::fetch_gguf(client, cache.as_ref(), refresh).await;
    gguf::merge_models(text_gen, gguf)
}
```

But `hf_api::fetch_text_generation` takes `&Cache`. Change `Cache::new()` to return `Option<Cache>`, and make hf_api functions accept `Option<&Cache>`.

Actually, simplest fix: change hf_api functions to accept `Option<&Cache>`.

Revised `src/models/hf_api.rs` — change signatures:

```rust
pub async fn fetch_text_generation(client: &reqwest::Client, cache: Option<&Cache>, refresh: bool) -> Vec<HfModelRow> {
```

And inside, guard all cache access with `if let Some(cache) = cache`.

This is getting detailed in the plan — the implementer will handle it. Key point: `Cache::new()` returns `Option<Cache>`, and hf_api functions accept `Option<&Cache>`.

- [ ] **Step 6: Update main.rs to call fetch_models (with tokio runtime)**

```rust
mod cli;
mod hardware;
mod models;

#[tokio::main]
async fn main() {
    let args = cli::Args::parse();
    let hw = hardware::detect(args.gpu.as_deref());

    if hw.gpus.is_empty() {
        println!("No GPU detected — CPU-only mode");
    } else {
        for gpu in &hw.gpus {
            println!("{} — {} MB VRAM, {:.0} GB/s", gpu.name, gpu.vram_mb, gpu.bandwidth_gbps);
        }
    }
    println!("CPU: {} ({} cores)", hw.cpu.name, hw.cpu.cores);
    println!("RAM: {:.1} GB", hw.ram_gb);

    let client = reqwest::Client::new();
    let models = models::fetch_models(&client, args.refresh).await;
    println!("Fetched {} models", models.len());
}
```

- [ ] **Step 7: Build and test**

Run: `cargo build`
Expected: compiles

Run: `cargo run`
Expected: prints hardware info, then "Fetched N models" (N > 0 if network available)

- [ ] **Step 8: Commit**

```bash
git add src/models/ Cargo.toml Cargo.lock src/main.rs
git commit -m "feat: model fetching from HuggingFace API with cache"
```

---

### Task 4: Static benchmarks + merging

**Files:**
- Create: `src/benchmarks/mod.rs`
- Create: `src/benchmarks/types.rs`
- Create: `src/benchmarks/static_data.json`
- Create: `src/benchmarks/static.rs`

**Interfaces:**
- Consumes: `models::types::ModelInfo`
- Produces: `benchmarks::merge_benchmarks(models: &mut [ModelInfo])`, `BenchmarkEntry`, `Evidence`

- [ ] **Step 1: Create src/benchmarks/types.rs**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkEntry {
    pub model_id: String,
    pub score: f64,           // 0-100
    pub source: String,       // "livebench", "arena_elo", "open_llm", etc.
    pub confidence: Evidence,
    pub date: String,         // YYYY-MM-DD
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Evidence {
    Direct = 5,
    Variant = 4,
    BaseModel = 3,
    Interpolated = 2,
    SelfReported = 1,
}

impl Evidence {
    pub fn weight(&self) -> f64 {
        match self {
            Self::Direct => 1.0,
            Self::Variant => 0.88,
            Self::BaseModel => 0.78,
            Self::Interpolated => 0.65,
            Self::SelfReported => 0.55,
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "direct" => Self::Direct,
            "variant" => Self::Variant,
            "base_model" => Self::BaseModel,
            "interpolated" => Self::Interpolated,
            "self_reported" => Self::SelfReported,
            _ => Self::SelfReported,
        }
    }
}
```

- [ ] **Step 2: Create src/benchmarks/static_data.json**

A curated JSON array of top models with benchmark scores. ~50 entries covering most popular models:

```json
[
  {"model_id": "Qwen/Qwen3-32B", "score": 83.0, "source": "livebench", "confidence": "direct", "date": "2025-06-01"},
  {"model_id": "Qwen/Qwen3-30B-A3B", "score": 82.7, "source": "livebench", "confidence": "direct", "date": "2025-06-01"},
  {"model_id": "Qwen/Qwen2.5-72B-Instruct", "score": 80.5, "source": "livebench", "confidence": "direct", "date": "2025-03-01"},
  {"model_id": "Qwen/Qwen2.5-32B-Instruct", "score": 74.0, "source": "livebench", "confidence": "direct", "date": "2025-03-01"},
  {"model_id": "Qwen/Qwen2.5-14B-Instruct", "score": 65.0, "source": "livebench", "confidence": "direct", "date": "2025-03-01"},
  {"model_id": "Qwen/Qwen2.5-7B-Instruct", "score": 55.0, "source": "livebench", "confidence": "direct", "date": "2025-03-01"},
  {"model_id": "Qwen/Qwen2.5-3B-Instruct", "score": 40.0, "source": "livebench", "confidence": "direct", "date": "2025-03-01"},
  {"model_id": "Qwen/Qwen2.5-1.5B-Instruct", "score": 30.0, "source": "livebench", "confidence": "direct", "date": "2025-03-01"},
  {"model_id": "meta-llama/Llama-3.1-70B-Instruct", "score": 75.0, "source": "livebench", "confidence": "direct", "date": "2025-06-01"},
  {"model_id": "meta-llama/Llama-3.1-8B-Instruct", "score": 52.0, "source": "livebench", "confidence": "direct", "date": "2025-06-01"},
  {"model_id": "meta-llama/Llama-3.2-3B-Instruct", "score": 38.0, "source": "livebench", "confidence": "direct", "date": "2025-06-01"},
  {"model_id": "meta-llama/Llama-3.2-1B-Instruct", "score": 25.0, "source": "livebench", "confidence": "direct", "date": "2025-06-01"},
  {"model_id": "mistralai/Mistral-Nemo-Instruct-2407", "score": 58.0, "source": "livebench", "confidence": "direct", "date": "2025-01-01"},
  {"model_id": "mistralai/Mixtral-8x7B-Instruct-v0.1", "score": 62.0, "source": "livebench", "confidence": "direct", "date": "2024-12-01"},
  {"model_id": "mistralai/Mixtral-8x22B-Instruct-v0.1", "score": 70.0, "source": "livebench", "confidence": "direct", "date": "2024-12-01"},
  {"model_id": "google/gemma-2-27b-it", "score": 68.0, "source": "livebench", "confidence": "direct", "date": "2025-06-01"},
  {"model_id": "google/gemma-2-9b-it", "score": 57.0, "source": "livebench", "confidence": "direct", "date": "2025-06-01"},
  {"model_id": "google/gemma-2-2b-it", "score": 35.0, "source": "livebench", "confidence": "direct", "date": "2025-06-01"},
  {"model_id": "microsoft/Phi-3.5-mini-instruct", "score": 48.0, "source": "livebench", "confidence": "direct", "date": "2025-06-01"},
  {"model_id": "microsoft/Phi-3-medium-4k-instruct", "score": 55.0, "source": "livebench", "confidence": "direct", "date": "2025-03-01"},
  {"model_id": "CohereForAI/c4ai-command-r-plus", "score": 66.0, "source": "livebench", "confidence": "direct", "date": "2024-12-01"},
  {"model_id": "CohereForAI/c4ai-command-r", "score": 55.0, "source": "livebench", "confidence": "direct", "date": "2024-12-01"},
  {"model_id": "deepseek-ai/DeepSeek-R1", "score": 85.0, "source": "livebench", "confidence": "direct", "date": "2025-06-01"},
  {"model_id": "deepseek-ai/DeepSeek-V3", "score": 82.0, "source": "livebench", "confidence": "direct", "date": "2025-06-01"},
  {"model_id": "deepseek-ai/DeepSeek-R1-Distill-Qwen-32B", "score": 72.0, "source": "livebench", "confidence": "variant", "date": "2025-06-01"},
  {"model_id": "deepseek-ai/DeepSeek-R1-Distill-Llama-70B", "score": 78.0, "source": "livebench", "confidence": "variant", "date": "2025-06-01"}
]
```

- [ ] **Step 3: Create src/benchmarks/static.rs**

```rust
use crate::benchmarks::types::BenchmarkEntry;

pub fn load_static() -> Vec<BenchmarkEntry> {
    let raw: &str = include_str!("static_data.json");
    serde_json::from_str(raw).unwrap_or_else(|e| {
        eprintln!("Warning: failed to parse static benchmarks: {e}");
        vec![]
    })
}
```

- [ ] **Step 4: Create src/benchmarks/mod.rs with merge logic**

```rust
pub mod types;
pub mod static_data;
pub mod static_rs;

use types::BenchmarkEntry;
use crate::models::types::ModelInfo;

/// Merge static and live benchmarks into model metadata.
/// Sets benchmark_score, benchmark_confidence, benchmark_source on each model.
pub fn merge_benchmarks(models: &mut [ModelInfo]) {
    let static_entries = static_rs::load_static();
    let mut lookup: std::collections::HashMap<&str, &BenchmarkEntry> = std::collections::HashMap::new();
    for entry in &static_entries {
        lookup.entry(&entry.model_id).or_insert(entry);
    }

    for model in models.iter_mut() {
        // Check static benchmark by exact ID match
        if let Some(entry) = lookup.get(model.model_id.as_str()) {
            model.benchmark_score = Some(entry.score);
            model.benchmark_confidence = Some(entry.confidence);
            model.benchmark_source = Some(entry.source.clone());
            continue;
        }

        // Check by base_model match
        if let Some(ref base) = model.base_model {
            if let Some(entry) = lookup.get(base.as_str()) {
                model.benchmark_score = Some(entry.score);
                model.benchmark_confidence = Some(Evidence::BaseModel);
                model.benchmark_source = Some(entry.source.clone());
                continue;
            }
        }

        // Check variant: strip -Instruct, -GGUF, etc.
        let stripped = strip_suffixes(&model.model_id);
        if stripped != model.model_id {
            if let Some(entry) = lookup.get(stripped.as_str()) {
                model.benchmark_score = Some(entry.score);
                model.benchmark_confidence = Some(Evidence::Variant);
                model.benchmark_source = Some(entry.source.clone());
                continue;
            }
        }

        // Use HuggingFace evalResults if available
        if let Some(ref eval) = model.eval_results {
            if !eval.scores.is_empty() {
                let avg: f64 = eval.scores.iter().map(|(_, s)| s).sum::<f64>() / eval.scores.len() as f64;
                model.benchmark_score = Some(avg);
                model.benchmark_confidence = Some(Evidence::SelfReported);
                model.benchmark_source = Some("huggingface_card".to_string());
            }
        }
    }
}

fn strip_suffixes(id: &str) -> String {
    let mut s = id.to_string();
    loop {
        let before = s.clone();
        for suffix in &["-Instruct", "-GGUF", "-gguf", "-chat", "-Chat"] {
            if s.ends_with(suffix) {
                s = s[..s.len() - suffix.len()].to_string();
            }
        }
        if s == before { break; }
    }
    s
}
```

Wait — `ModelInfo` doesn't have `benchmark_score`, `benchmark_confidence`, `benchmark_source` fields yet. Need to add them to `models/types.rs::ModelInfo`:

```rust
pub struct ModelInfo {
    // ... existing fields ...
    pub benchmark_score: Option<f64>,
    pub benchmark_confidence: Option<Evidence>,
    pub benchmark_source: Option<String>,
}
```

But `Evidence` is in `benchmarks::types`, creating a circular dep if `models::types` depends on `benchmarks::types`. Solve by defining `Evidence` in `models::types` or a shared `types` module.

Simplest: duplicate the `Evidence` type in `models::types` as a simple enum/mapping, and have `benchmarks` refer to it.

Actually, simpler: add the three fields as primitive types:
- `benchmark_score: Option<f64>`
- `benchmark_confidence: Option<f64>`  (just the weight)
- `benchmark_source: Option<String>`

And `benchmarks/mod.rs` sets `benchmark_confidence` to `entry.confidence.weight()`.

This avoids the circular dep entirely.

- [ ] **Step 5: Add benchmark fields to ModelInfo in src/models/types.rs**

Add to `ModelInfo`:
```rust
pub benchmark_score: Option<f64>,
pub benchmark_confidence: Option<f64>,  // evidence weight 0.55-1.0
pub benchmark_source: Option<String>,
```

Initialize as `None` in `gguf::merge_models`.

- [ ] **Step 6: Update main.rs to call merge_benchmarks**

Add `mod benchmarks;` and after fetch:
```rust
let mut models = models::fetch_models(&client, args.refresh).await;
benchmarks::merge_benchmarks(&mut models);
```

- [ ] **Step 7: Build and test**

Run: `cargo build`
Expected: compiles

- [ ] **Step 8: Commit**

```bash
git add src/benchmarks/ src/models/types.rs src/main.rs
git commit -m "feat: static benchmarks + live HF eval merging"
```

---

### Task 5: Engine — VRAM estimation, speed, scoring, ranking

**Files:**
- Create: `src/engine/mod.rs`
- Create: `src/engine/types.rs`
- Create: `src/engine/vram.rs`
- Create: `src/engine/speed.rs`
- Create: `src/engine/scoring.rs`

**Interfaces:**
- Consumes: `hardware::types::HardwareInfo`, `models::types::ModelInfo`
- Produces: `engine::rank(models, hw, top, speed_filter) -> Vec<RankResult>`, `RankResult`

- [ ] **Step 1: Create src/engine/types.rs**

```rust
use crate::models::types::{ModelInfo, GGUFVariant};

#[derive(Debug, Clone)]
pub struct RankResult {
    pub model: ModelInfo,
    pub variant: GGUFVariant,     // best-fit GGUF variant
    pub score: f64,
    pub vram_required_mb: u64,
    pub fit_type: FitType,
    pub estimated_tps: f64,       // tokens per second
    pub speed_confidence: SpeedConfidence,
    pub score_marker: ScoreMarker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FitType {
    FullGpu,
    PartialOffload,
    CpuOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeedConfidence {
    High,
    Estimated,
    Low,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreMarker {
    None,       // direct benchmark
    Inferred,   // ~
    NoData,     // ?
    SelfReported, // !sr
}
```

- [ ] **Step 2: Create src/engine/vram.rs**

```rust
use crate::models::types::{ModelInfo, GGUFVariant};
use crate::hardware::types::HardwareInfo;
use super::types::FitType;

const FRAMEWORK_OVERHEAD_MB: u64 = 500;
const KV_CACHE_PER_LAYER_MB: f64 = 0.5; // ponytail: rough estimate per layer per 1K context
const DEFAULT_LAYERS: u64 = 32;
const DEFAULT_CONTEXT_LEN: u64 = 4096;

/// Estimate VRAM required for a specific GGUF variant
pub fn estimate_vram(model: &ModelInfo, variant: &GGUFVariant) -> u64 {
    // Weights from file size (most accurate)
    let weights_mb = variant.size_bytes / 1_048_576;

    // KV cache estimate: proportional to params
    let layers = DEFAULT_LAYERS; // ponytail: use model-specific later
    let kv_mb = (KV_CACHE_PER_LAYER_MB * layers as f64 * DEFAULT_CONTEXT_LEN as f64 / 1024.0) as u64;

    weights_mb + kv_mb + FRAMEWORK_OVERHEAD_MB
}

/// Determine fit type based on available hardware
pub fn fit_type(vram_required_mb: u64, hw: &HardwareInfo) -> FitType {
    let gpu_vram_mb: u64 = hw.gpus.iter().map(|g| g.vram_mb).sum();

    if vram_required_mb <= gpu_vram_mb {
        FitType::FullGpu
    } else if vram_required_mb <= gpu_vram_mb + (hw.ram_gb * 1024.0) as u64 {
        FitType::PartialOffload
    } else {
        FitType::CpuOnly
    }
}
```

- [ ] **Step 3: Create src/engine/speed.rs**

```rust
use crate::hardware::types::HardwareInfo;
use super::types::FitType;

/// Estimate tokens per second for a model variant
pub fn estimate_tps(
    vram_required_mb: u64,
    bandwidth_gbps: f64,
    params_b: f64,
    quant_efficiency: f64,
    fit_type: FitType,
    hw: &HardwareInfo,
) -> f64 {
    if bandwidth_gbps <= 0.0 || params_b <= 0.0 {
        return 0.0;
    }

    // Memory bandwidth bound: BW / model_size_in_GB
    let model_size_gb = vram_required_mb as f64 / 1024.0;
    let bandwidth_bound = bandwidth_gbps / model_size_gb;

    // Scale by quantization efficiency (higher quant = more compute per byte)
    let base_tps = bandwidth_bound * quant_efficiency * 2.0; // ponytail: rough scaling factor

    // Fit type penalty
    let fit_factor = match fit_type {
        FitType::FullGpu => 1.0,
        FitType::PartialOffload => 0.35,  // PCIe bottleneck
        FitType::CpuOnly => 0.10,         // CPU memory bandwidth much lower
    };

    let tps = base_tps * fit_factor;

    // For CPU-only, cap at a reasonable maximum
    if fit_type == FitType::CpuOnly {
        tps.min(hw.cpu.cores as f64 * 1.5) // ponytail: rough CPU tps cap
    } else {
        tps
    }
}

/// Quantization efficiency factor (how many tokens per byte of bandwidth)
pub fn quant_efficiency(bits_per_weight: f64) -> f64 {
    // Lower bits = less data to move = higher efficiency per byte
    // But also less information per parameter
    // Sweet spot around Q4-Q5
    if bits_per_weight <= 3.0 { 0.85 }
    else if bits_per_weight <= 4.5 { 1.0 }
    else if bits_per_weight <= 6.0 { 0.95 }
    else if bits_per_weight <= 8.0 { 0.90 }
    else { 0.85 }
}
```

- [ ] **Step 4: Create src/engine/scoring.rs**

```rust
use crate::models::types::{ModelInfo, GGUFVariant, QuantType};
use crate::hardware::types::HardwareInfo;
use super::types::{FitType, RankResult, ScoreMarker, SpeedConfidence};
use super::vram;
use super::speed;

/// Score and rank models, returning top N results
pub fn rank(
    models: &[ModelInfo],
    hw: &HardwareInfo,
    top: usize,
    speed_filter: Option<&str>,
) -> Vec<RankResult> {
    let gpu_bandwidth: f64 = hw.gpus.iter().map(|g| g.bandwidth_gbps).fold(0.0, |a, b| a + b);

    let mut results: Vec<RankResult> = vec![];

    for model in models {
        if model.gguf_variants.is_empty() {
            continue;  // skip non-GGUF models for v1
        }

        // Try each GGUF variant, pick the one with best score that fits
        for variant in &model.gguf_variants {
            let vram_mb = vram::estimate_vram(model, variant);
            let fit = vram::fit_type(vram_mb, hw);
            let q_eff = speed::quant_efficiency(variant.quant.bits_per_weight());
            let params_b = model.params_b.unwrap_or(0.0);
            let tps = speed::estimate_tps(vram_mb, gpu_bandwidth, params_b, q_eff, fit, hw);

            let score = compute_score(model, variant, fit, tps);
            let marker = score_marker(model);

            // Speed filter
            if let Some(filter) = speed_filter {
                let min_tps = match filter {
                    "usable" => 10.0,
                    "fast" => 30.0,
                    _ => 0.0,
                };
                if tps < min_tps {
                    continue;
                }
            }

            results.push(RankResult {
                model: model.clone(),
                variant: variant.clone(),
                score,
                vram_required_mb: vram_mb,
                fit_type: fit,
                estimated_tps: tps,
                speed_confidence: if tps > 0.0 { SpeedConfidence::Estimated } else { SpeedConfidence::Low },
                score_marker: marker,
            });
        }
    }

    // Sort by score descending
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    // Deduplicate by model_id, keeping highest score variant
    let mut seen = std::collections::HashSet::new();
    results.retain(|r| seen.insert(r.model.model_id.clone()));

    results.truncate(top);
    results
}

fn compute_score(model: &ModelInfo, variant: &GGUFVariant, fit: FitType, tps: f64) -> f64 {
    // Benchmark score (0-100), default 30 if no data
    let benchmark = model.benchmark_score.unwrap_or(30.0);
    let evidence_weight = model.benchmark_confidence.unwrap_or(0.55);

    // Size bonus: log2(params_gb), capped at 35
    let params_b = model.params_b.unwrap_or(1.0);
    let size_bonus = (params_b.ln() / 2.0_f64.ln()).min(35.0);

    // Quantization quality penalty
    let quant_penalty = variant.quant.quality_penalty();

    // Fit type factor
    let fit_factor = match fit {
        FitType::FullGpu => 1.0,
        FitType::PartialOffload => 0.72,
        FitType::CpuOnly => 0.50,
    };

    // Speed adjustment: -8 to +8
    let speed_adj = if tps >= 30.0 { 8.0 }
    else if tps >= 10.0 { 4.0 }
    else if tps >= 4.0 { -2.0 }
    else { -8.0 };

    // Source trust: official org bonus +3
    let trust_adj = if is_official_org(model) { 3.0 } else { 0.0 };

    let score = benchmark * evidence_weight * (1.0 + size_bonus / 100.0) * quant_penalty * fit_factor + speed_adj + trust_adj;

    score.max(0.0).min(100.0)
}

fn score_marker(model: &ModelInfo) -> ScoreMarker {
    if model.benchmark_score.is_none() {
        ScoreMarker::NoData
    } else if let Some(conf) = model.benchmark_confidence {
        if conf <= 0.55 { ScoreMarker::SelfReported }
        else if conf < 1.0 { ScoreMarker::Inferred }
        else { ScoreMarker::None }
    } else {
        ScoreMarker::NoData
    }
}

fn is_official_org(model: &ModelInfo) -> bool {
    // Check if model author matches the org in the model ID
    let parts: Vec<&str> = model.model_id.split('/').collect();
    if parts.len() >= 2 {
        parts[0] == model.author
    } else {
        false
    }
}
```

- [ ] **Step 5: Create src/engine/mod.rs**

```rust
pub mod types;
pub mod vram;
pub mod speed;
pub mod scoring;

pub use scoring::rank;
```

- [ ] **Step 6: Update main.rs to use engine**

```rust
mod cli;
mod hardware;
mod models;
mod benchmarks;
mod engine;

#[tokio::main]
async fn main() {
    let args = cli::Args::parse();
    let hw = hardware::detect(args.gpu.as_deref());

    if hw.gpus.is_empty() {
        println!("No GPU detected — CPU-only mode");
    } else {
        for gpu in &hw.gpus {
            println!("{} — {} MB VRAM, {:.0} GB/s", gpu.name, gpu.vram_mb, gpu.bandwidth_gbps);
        }
    }
    println!("CPU: {} ({} cores)", hw.cpu.name, hw.cpu.cores);
    println!("RAM: {:.1} GB", hw.ram_gb);

    let client = reqwest::Client::new();
    let mut models = models::fetch_models(&client, args.refresh).await;
    benchmarks::merge_benchmarks(&mut models);

    let results = engine::rank(&models, &hw, args.top, args.speed.as_deref());
    println!("Top {} results:", results.len());
    for r in &results {
        println!("#{}  {}  {}  {}  score {:.1}  {:.0} t/s",
            results.iter().position(|x| x.model.model_id == r.model.model_id).unwrap() + 1,
            r.model.model_id,
            r.model.params_b.map(|p| format!("{p:.1}B")).unwrap_or_else(|| "?".to_string()),
            r.variant.quant.display_name(),
            r.score,
            r.estimated_tps,
        );
    }
}
```

- [ ] **Step 7: Build and test**

Run: `cargo build`
Expected: compiles

- [ ] **Step 8: Commit**

```bash
git add src/engine/ src/main.rs
git commit -m "feat: ranking engine (VRAM, speed, scoring, fit)"
```

---

### Task 6: Colored output + JSON output

**Files:**
- Create: `src/output/mod.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `engine::types::RankResult`, `cli::Args`
- Produces: `output::print_ranking()`, `output::print_json()`

- [ ] **Step 1: Create src/output/mod.rs**

```rust
use owo_colors::OwoColorize;
use crate::engine::types::{RankResult, FitType, ScoreMarker};

pub fn print_ranking(results: &[RankResult]) {
    for (i, r) in results.iter().enumerate() {
        let rank = format!("#{}", i + 1);
        let id = &r.model.model_id;
        let params = r.model.params_b.map(|p| format!("{p:.1}B")).unwrap_or_else(|| "?".to_string());
        let quant = r.variant.quant.display_name();
        let score = format!("{:.1}", r.score);
        let fit = match r.fit_type {
            FitType::FullGpu => "GPU".green().to_string(),
            FitType::PartialOffload => "OFFLOAD".yellow().to_string(),
            FitType::CpuOnly => "CPU".red().to_string(),
        };

        let speed_str = format_speed(r.estimated_tps);
        let marker = match r.score_marker {
            ScoreMarker::None => String::new(),
            ScoreMarker::Inferred => " ~".yellow().to_string(),
            ScoreMarker::NoData => " ?".red().to_string(),
            ScoreMarker::SelfReported => " !sr".bright_yellow().to_string(),
        };

        println!("{rank:>4}  {id}  {params}  {quant}  {fit}  score {score}{marker}  {speed_str}");
    }
}

fn format_speed(tps: f64) -> String {
    if tps <= 0.0 {
        return "? t/s".to_string();
    }
    let rounded = if tps >= 100.0 { format!("{tps:.0}") } else { format!("{tps:.1}") };
    if tps >= 30.0 { format!("{rounded} t/s").bright_green().to_string() }
    else if tps >= 10.0 { format!("{rounded} t/s").green().to_string() }
    else if tps >= 4.0 { format!("{rounded} t/s").yellow().to_string() }
    else { format!("{rounded} t/s").red().to_string() }
}

pub fn print_json(results: &[RankResult]) {
    let json_results: Vec<serde_json::Value> = results.iter().map(|r| {
        serde_json::json!({
            "rank": 0, // will be set by caller
            "model_id": r.model.model_id,
            "params_b": r.model.params_b,
            "quant": r.variant.quant.display_name(),
            "score": r.score,
            "fit_type": match r.fit_type {
                FitType::FullGpu => "full_gpu",
                FitType::PartialOffload => "partial_offload",
                FitType::CpuOnly => "cpu_only",
            },
            "vram_required_mb": r.vram_required_mb,
            "estimated_tps": r.estimated_tps,
            "benchmark_source": r.model.benchmark_source,
            "benchmark_confidence": r.model.benchmark_confidence,
        })
    }).collect();

    // Add rank numbers
    let mut with_rank: Vec<serde_json::Value> = json_results;
    for (i, v) in with_rank.iter_mut().enumerate() {
        if let Some(obj) = v.as_object_mut() {
            obj.insert("rank".to_string(), serde_json::Value::Number(serde_json::Number::from(i + 1)));
        }
    }

    let output = serde_json::json!({ "models": with_rank });
    println!("{}", serde_json::to_string_pretty(&output).unwrap_or_default());
}
```

- [ ] **Step 2: Update main.rs to use output module**

```rust
mod cli;
mod hardware;
mod models;
mod benchmarks;
mod engine;
mod output;

#[tokio::main]
async fn main() {
    let args = cli::Args::parse();
    let hw = hardware::detect(args.gpu.as_deref());

    if hw.gpus.is_empty() {
        println!("No GPU detected — CPU-only mode");
    } else {
        for gpu in &hw.gpus {
            println!("{} — {} MB VRAM, {:.0} GB/s", gpu.name, gpu.vram_mb, gpu.bandwidth_gbps);
        }
    }
    println!("CPU: {} ({} cores)", hw.cpu.name, hw.cpu.cores);
    println!("RAM: {:.1} GB", hw.ram_gb);
    println!();

    let client = reqwest::Client::new();
    let mut models = models::fetch_models(&client, args.refresh).await;
    benchmarks::merge_benchmarks(&mut models);

    let results = engine::rank(&models, &hw, args.top, args.speed.as_deref());

    if args.json {
        output::print_json(&results);
    } else {
        output::print_ranking(&results);
    }
}
```

- [ ] **Step 3: Build and test**

Run: `cargo build`
Expected: compiles

Run: `cargo run`
Expected: prints hardware info + colored ranking table

Run: `cargo run -- --json`
Expected: prints JSON output

- [ ] **Step 4: Commit**

```bash
git add src/output/ src/main.rs
git commit -m "feat: colored terminal output + JSON output"
```

---

### Task 7: Polish + end-to-end test

**Files:**
- Modify: `src/main.rs` — clean up, handle errors
- Modify: `Cargo.toml` — add metadata

- [ ] **Step 1: Add proper error handling to main.rs**

Wrap in `Result` and use `process::exit` on fatal errors. Add eprintln for progress.

```rust
mod cli;
mod hardware;
mod models;
mod benchmarks;
mod engine;
mod output;

#[tokio::main]
async fn main() {
    let args = cli::Args::parse();
    let hw = hardware::detect(args.gpu.as_deref());

    // Print hardware info
    if hw.gpus.is_empty() {
        eprintln!("No GPU detected — CPU-only mode");
    } else {
        for gpu in &hw.gpus {
            eprintln!("{} — {} MB VRAM, {:.0} GB/s", gpu.name, gpu.vram_mb, gpu.bandwidth_gbps);
        }
    }
    eprintln!("CPU: {} ({} cores)", hw.cpu.name, hw.cpu.cores);
    eprintln!("RAM: {:.1} GB", hw.ram_gb);
    eprintln!();

    // Fetch models
    eprintln!("Fetching models from HuggingFace...");
    let client = reqwest::Client::new();
    let mut models = models::fetch_models(&client, args.refresh).await;
    if models.is_empty() {
        eprintln!("Error: no models fetched. Check network connection or try --refresh.");
        std::process::exit(1);
    }
    eprintln!("Found {} models", models.len());

    // Merge benchmarks
    benchmarks::merge_benchmarks(&mut models);

    // Rank
    let results = engine::rank(&models, &hw, args.top, args.speed.as_deref());

    // Output
    if args.json {
        output::print_json(&results);
    } else {
        output::print_ranking(&results);
    }
}
```

- [ ] **Step 2: Update Cargo.toml with metadata**

```toml
[package]
name = "whichllm"
version = "0.1.0"
edition = "2024"
description = "Find the best local LLM that runs on your hardware"
license = "MIT"
repository = "https://github.com/Andyyyy64/whichllm"
keywords = ["llm", "gpu", "vram", "huggingface", "benchmark"]
categories = ["command-line-utilities"]
```

- [ ] **Step 3: Build release binary**

Run: `cargo build --release`
Expected: compiles

- [ ] **Step 4: Run end-to-end test**

Run: `cargo run --release`
Expected: prints hardware info to stderr, ranked model list to stdout

Run: `cargo run --release -- --gpu "RTX 4090"`
Expected: simulates RTX 4090 and ranks models for that GPU

Run: `cargo run --release -- --gpu "RTX 4090" --top 5 --json`
Expected: 5 results as JSON

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "feat: whichllm v1 minimal — hardware detect, HF fetch, benchmark merge, rank, output"
```
