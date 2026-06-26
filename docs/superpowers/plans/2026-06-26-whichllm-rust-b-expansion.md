# whichllm Rust v1→B Expansion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Extend v1 minimal with new commands (plan, upgrade, hardware), new filters (--gpu-only, --cpu-only, --markdown, --vram-headroom, --ram-budget, --profile, --quant, --context-length, --min-speed, --details), and GFM table output.

**Architecture:** Same single-binary structure. Extend `cli.rs` with subcommands and new flags. Add `plan` and `upgrade` logic to engine. Add markdown output to output module. Add profile filtering.

**Tech Stack:** Same as v1 (clap, reqwest, tokio, serde, owo-colors, dirs, chrono)

## Global Constraints

- Rust edition 2024
- All new flags are additions — no breaking changes to existing CLI
- `--markdown` and `--json` are mutually exclusive output formats
- Context length format: integer or `Nk`/`NM` suffix (e.g. `4096`, `4k`, `64k`)
- VRAM headroom format: `NGB` or `N.MGB` (e.g. `1GB`, `1.5GB`)
- Profile data bundled as static JSON like benchmarks

---

### Task 1: Expand CLI args with new flags

**Files:**
- Modify: `src/cli.rs`

**Interfaces:**
- Produces: expanded `cli::Args` with subcommands and new flag fields
- Produces: `cli::Command` enum for subcommands

- [ ] **Step 1: Rewrite src/cli.rs with subcommands and all new flags**

```rust
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "whichllm", about = "Find the best local LLM for your hardware")]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Simulate a specific GPU (e.g. "RTX 4090")
    #[arg(long)]
    pub gpu: Option<String>,

    /// Number of results to show
    #[arg(long, default_value = "10")]
    pub top: usize,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Output as GitHub-Flavored Markdown table
    #[arg(long, short = 'm')]
    pub markdown: bool,

    /// Bypass cache, fetch fresh data
    #[arg(long)]
    pub refresh: bool,

    /// Speed filter: "usable" (>=10 t/s) or "fast" (>=30 t/s)
    #[arg(long)]
    pub speed: Option<String>,

    /// Exact minimum tok/s floor
    #[arg(long)]
    pub min_speed: Option<f64>,

    /// Only show models that fit entirely in GPU VRAM
    #[arg(long)]
    pub gpu_only: bool,

    /// Fit filter: full-gpu, gpu (same), any (default)
    #[arg(long)]
    pub fit: Option<String>,

    /// Force CPU-only mode
    #[arg(long)]
    pub cpu_only: bool,

    /// Reserve extra VRAM (e.g. "1GB", "1.5GB")
    #[arg(long)]
    pub vram_headroom: Option<String>,

    /// Limit system RAM usable for offload (e.g. "8GB", "available")
    #[arg(long)]
    pub ram_budget: Option<String>,

    /// Task profile: general, coding, vision, math
    #[arg(long)]
    pub profile: Option<String>,

    /// Filter by quantization (e.g. "Q4_K_M")
    #[arg(long)]
    pub quant: Option<String>,

    /// Override context length for KV cache (e.g. "4096", "4k", "64k")
    #[arg(long)]
    pub context_length: Option<String>,

    /// Show download metadata instead of runtime columns
    #[arg(long)]
    pub details: bool,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Command {
    /// Show hardware info only
    Hardware,
    /// What GPU do I need for a specific model?
    Plan {
        /// Model name (fuzzy match, e.g. "llama 3 70b")
        model: String,
        /// Quantization filter
        #[arg(long)]
        quant: Option<String>,
        /// Context length override
        #[arg(long)]
        context_length: Option<String>,
    },
    /// Compare upgrade GPU candidates
    Upgrade {
        /// GPU names to compare
        gpus: Vec<String>,
        /// Number of top results per GPU
        #[arg(long, default_value = "3")]
        top: usize,
    },
}

/// Parse a size string like "1GB", "1.5GB" into MB
pub fn parse_size_mb(s: &str) -> Option<u64> {
    let s = s.to_uppercase();
    if let Some(num_str) = s.strip_suffix("GB") {
        num_str.parse::<f64>().ok().map(|v| (v * 1024.0) as u64)
    } else if let Some(num_str) = s.strip_suffix("MB") {
        num_str.parse::<u64>().ok()
    } else {
        None
    }
}

/// Parse a context length like "4096", "4k", "64k"
pub fn parse_context_length(s: &str) -> Option<u64> {
    let s = s.to_lowercase();
    if let Some(num_str) = s.strip_suffix('k') {
        num_str.parse::<f64>().ok().map(|v| (v * 1024.0) as u64)
    } else if let Some(num_str) = s.strip_suffix('m') {
        num_str.parse::<f64>().ok().map(|v| (v * 1_048_576.0) as u64)
    } else {
        s.parse::<u64>().ok()
    }
}
```

- [ ] **Step 2: Update main.rs to handle new flags and subcommands**

```rust
mod cli;
mod hardware;
mod models;
mod benchmarks;
mod engine;
mod output;

use clap::Parser;

#[tokio::main]
async fn main() {
    let args = cli::Args::parse();

    match args.command {
        Some(cli::Command::Hardware) => {
            cmd_hardware(&args);
            return;
        }
        Some(cli::Command::Plan { model, quant, context_length }) => {
            cmd_plan(&args, &model, quant.as_deref(), context_length.as_deref()).await;
            return;
        }
        Some(cli::Command::Upgrade { gpus, top }) => {
            cmd_upgrade(&args, &gpus, top).await;
            return;
        }
        None => {}
    }

    cmd_rank(&args).await;
}

fn cmd_hardware(args: &cli::Args) {
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
    println!("OS: {:?}", hw.os);
}

async fn cmd_plan(args: &cli::Args, model_name: &str, quant: Option<&str>, ctx_len: Option<&str>) {
    // Fetch models and find best match
    let client = reqwest::Client::new();
    let models = models::fetch_models(&client, args.refresh).await;
    let matches = engine::find_model(&models, model_name);
    if matches.is_empty() {
        eprintln!("No models matching '{model_name}' found");
        std::process::exit(1);
    }
    let ctx = ctx_len.and_then(cli::parse_context_length).unwrap_or(4096);
    output::print_plan(&matches, quant, ctx, args.json);
}

async fn cmd_upgrade(args: &cli::Args, gpus: &[String], top: usize) {
    let client = reqwest::Client::new();
    let mut models = models::fetch_models(&client, args.refresh).await;
    benchmarks::merge_benchmarks(&mut models);

    let mut results = vec![];
    for gpu_name in gpus {
        let hw = hardware::detect(Some(gpu_name));
        let ranked = engine::rank(&models, &hw, top, None, 4096, false, None);
        results.push((gpu_name.clone(), hw, ranked));
    }
    // Also include current machine
    let hw_current = hardware::detect(args.gpu.as_deref());
    let ranked_current = engine::rank(&models, &hw_current, top, None, 4096, false, None);
    results.insert(0, ("Current".to_string(), hw_current, ranked_current));

    output::print_upgrade(&results, args.json);
}

async fn cmd_rank(args: &cli::Args) {
    let mut hw = hardware::detect(args.gpu.as_deref());

    // CPU-only override
    if args.cpu_only {
        hw.gpus.clear();
    }

    // GPU-only / fit filter
    let gpu_only = args.gpu_only || args.fit.as_deref() == Some("full-gpu") || args.fit.as_deref() == Some("gpu");

    // VRAM headroom
    if let Some(ref headroom) = args.vram_headroom {
        if let Some(mb) = cli::parse_size_mb(headroom) {
            for gpu in &mut hw.gpus {
                gpu.vram_mb = gpu.vram_mb.saturating_sub(mb);
            }
        }
    }

    // RAM budget
    if let Some(ref budget) = args.ram_budget {
        if budget == "available" {
            // keep detected RAM
        } else if let Some(mb) = cli::parse_size_mb(budget) {
            hw.ram_gb = mb as f64 / 1024.0;
        }
    }

    // Context length
    let ctx_len = args.context_length.as_deref().and_then(cli::parse_context_length).unwrap_or(4096);

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
        eprintln!("Error: no models fetched. Check network or try --refresh.");
        std::process::exit(1);
    }
    eprintln!("Found {} models", models.len());

    // Merge benchmarks
    benchmarks::merge_benchmarks(&mut models);

    // Profile filter
    if let Some(ref profile) = args.profile {
        models.retain(|m| engine::matches_profile(m, profile));
    }

    // Quant filter
    let quant_filter = args.quant.as_deref();

    // Min speed
    let min_speed = args.min_speed.or_else(|| {
        args.speed.as_deref().map(|s| match s {
            "usable" => 10.0,
            "fast" => 30.0,
            _ => 0.0,
        })
    });

    // Rank
    let results = engine::rank(&models, &hw, args.top, min_speed, ctx_len, gpu_only, quant_filter);

    // Output
    if args.json {
        output::print_json(&results);
    } else if args.markdown {
        output::print_markdown(&results);
    } else {
        output::print_ranking(&results);
    }
}
```

- [ ] **Step 3: Build and verify**

Run: `cargo build`
Expected: compiles (may have unused warnings for new functions not yet called)

- [ ] **Step 4: Commit**

```bash
git add src/cli.rs src/main.rs
git commit -m "feat: expanded CLI with subcommands and new flags"
```

---

### Task 2: Update engine::rank to accept new filters

**Files:**
- Modify: `src/engine/scoring.rs`
- Modify: `src/engine/vram.rs`
- Modify: `src/engine/mod.rs`

**Interfaces:**
- New signature: `rank(models, hw, top, min_speed, ctx_len, gpu_only, quant_filter) -> Vec<RankResult>`
- New: `find_model(models, query) -> Vec<ModelInfo>` for plan command
- New: `matches_profile(model, profile) -> bool` for profile filter

- [ ] **Step 1: Update engine::rank signature and logic**

Change `rank` to accept:
- `min_speed: Option<f64>` (replaces `speed_filter: Option<&str>`)
- `ctx_len: u64` (context length override)
- `gpu_only: bool`
- `quant_filter: Option<&str>`

Update `scoring.rs::rank()`:
- Use `ctx_len` in VRAM estimation
- Filter by `gpu_only` (skip `CpuOnly` and `PartialOffload` results)
- Filter by `quant_filter` (match `variant.quant.display_name()`)
- Use `min_speed` as exact floor

- [ ] **Step 2: Update vram.rs to accept context length**

Change `estimate_vram` to accept `ctx_len: u64` parameter for KV cache scaling.

- [ ] **Step 3: Add find_model and matches_profile to engine**

`find_model(models, query)`: fuzzy match model ID by normalizing query (lowercase, strip spaces, substring match).

`matches_profile(model, profile)`: check model ID prefixes against curated profile lists. Add `src/engine/profiles.rs` with static data:

```rust
const CODING_PREFIXES: &[&str] = &[
    "deepseek-ai/DeepSeek-Coder",
    "Qwen/Qwen2.5-Coder",
    "microsoft/Phi-3",
    "mistralai/Codestral",
];

const VISION_PREFIXES: &[&str] = &[
    "llava",
    "Qwen/Qwen2-VL",
    "google/paligemma",
    "cogvlm",
];

const MATH_PREFIXES: &[&str] = &[
    "deepseek-ai/DeepSeek-Math",
    "meta-llama/Llama-3",
    "Mathstral",
];
```

- [ ] **Step 4: Build and test**

Run: `cargo build`
Expected: compiles

- [ ] **Step 5: Commit**

```bash
git add src/engine/
git commit -m "feat: engine rank with gpu_only, quant filter, context length, profiles"
```

---

### Task 3: Output — markdown + plan + upgrade displays

**Files:**
- Modify: `src/output/mod.rs`

**Interfaces:**
- New: `print_markdown(results)`
- New: `print_plan(matches, quant, ctx_len, json)`
- New: `print_upgrade(results, json)`

- [ ] **Step 1: Add print_markdown**

GFM table format:
```markdown
| # | Model | Params | Quant | Fit | Score | Speed |
|---|-------|--------|-------|-----|-------|-------|
| 1 | Qwen/Qwen3-32B | 32.0B | Q5_K_M | GPU | 90.8 | 91 t/s |
```

- [ ] **Step 2: Add print_plan**

For plan command output:
```
Model: Qwen/Qwen2.5-72B-Instruct (72.0B params)

Quant  VRAM Required  Fits
Q4_K_M  38544 MB      RTX 4090+RAM, A100 40GB, A100 80GB, H100
Q5_K_M  47520 MB      A100 80GB, H100
Q8_0    76032 MB      H100
```

Uses GPU_TABLE from hardware module to check which GPUs can run it.

- [ ] **Step 3: Add print_upgrade**

Side-by-side comparison:
```
         Current (Apple M1)    RTX 4090              RTX 5090
VRAM     8192 MB               24576 MB              32768 MB
Top #1   Qwen2.5-7B (55.0)    Qwen3-32B (90.8)     Qwen3-32B (90.8)
Speed    ~8 t/s                ~91 t/s               ~121 t/s
```

- [ ] **Step 4: Build and test**

Run: `cargo build`
Expected: compiles

- [ ] **Step 5: Commit**

```bash
git add src/output/
git commit -m "feat: markdown, plan, and upgrade output formats"
```

---

### Task 4: End-to-end testing + polish

**Files:**
- Modify: `src/main.rs` — final wiring
- Modify: `Cargo.toml` — bump version to 0.2.0

- [ ] **Step 1: Test all new commands**

```bash
cargo run -- hardware
cargo run -- plan "llama 3 70b"
cargo run -- upgrade "RTX 4090" "RTX 5090"
cargo run -- --gpu "RTX 4090" --gpu-only --top 5
cargo run -- --gpu "RTX 4090" --markdown --top 5
cargo run -- --gpu "RTX 4090" --vram-headroom 1GB --top 5
cargo run -- --gpu "RTX 4090" --quant Q4_K_M --top 5
cargo run -- --gpu "RTX 4090" --context-length 32k --top 5
cargo run -- --gpu "RTX 4090" --profile coding --top 5
cargo run -- --cpu-only --top 5
cargo run -- --min-speed 20 --gpu "RTX 4090" --top 5
```

- [ ] **Step 2: Build release**

Run: `cargo build --release`

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat: whichllm v0.2.0 — plan, upgrade, hardware, gpu-only, markdown, profiles, filters"
```
