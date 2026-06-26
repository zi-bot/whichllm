# whichllm

Find the best local LLM that actually runs on your hardware.

Auto-detects your GPU/CPU/RAM and ranks the top models from HuggingFace that fit your system.

**This is a Rust port of [whichllm](https://github.com/Andyyyy64/whichllm) (Python).**

## Quick start

```bash
cargo run --release
```

## Install

```bash
cargo install --git https://github.com/zi-bot/whichllm
```
```bash
cargo install whichllm
```

## Usage

```bash
# Best models for this machine
whichllm

# Pretend you have a specific GPU
whichllm --gpu "RTX 4090"

# Override detected VRAM limits
whichllm --vram-headroom 1GB

# Only show models that fit fully in GPU VRAM
whichllm --gpu-only

# Simulate a multi-GPU workstation
whichllm --gpu "2x RTX 4090"

# Hide models that are technically runnable but too slow
whichllm --speed usable    # minimum 10 tok/s
whichllm --speed fast      # minimum 30 tok/s
whichllm --min-speed 20    # exact tok/s floor

# Pasteable GitHub / Slack / Discord output
whichllm --markdown

# JSON output for scripts
whichllm --json
whichllm --top 1 --json | jq '.models[0].model_id'

# Force CPU-only mode
whichllm --cpu-only
```

## Commands

### `whichllm plan <model>`

Find the GPU needed for a specific model.

```bash
whichllm plan "llama 3 70b"
whichllm plan "Qwen2.5-72B" --quant Q80
whichllm plan "mistral 7b" --context-length 32768
```

### `whichllm upgrade <gpu1> <gpu2> ...`

Compare your current machine against candidate GPUs.

```bash
whichllm upgrade "RTX 4090" "RTX 5090" "H100"
whichllm upgrade "Apple M4 Max"
```

### `whichllm hardware`

Show hardware info only.

```bash
whichllm hardware
```

## Common workflows

```bash
# Safe pick — only full-GPU fits, usable speed, with headroom
whichllm --gpu-only --speed usable --vram-headroom 1GB

# Find best coding model
whichllm --profile coding

# Filter by quantization
whichllm --quant Q4KM

# Longer context (increases KV cache estimate)
whichllm --context-length 64k

# Limit RAM available for offloading
whichllm --ram-budget 8GB

# More results
whichllm --top 20

# Force refresh (ignore cache)
whichllm --refresh
```

## Output example

```text
$ whichllm --gpu "RTX 4090"

#1  Qwen/Qwen3-32B       32.0B  Q5_K_M  GPU  score 90.8  91 t/s
#2  Qwen/Qwen3-30B-A3B   30.0B  Q5_K_M  GPU  score 90.4  97 t/s
#3  Qwen/Qwen2.5-32B     32.0B  Q5_K_M  GPU  score 81.8  91 t/s
```

Speed is colored by practical usability:
- **red** — slow (&lt;4 tok/s)
- **yellow** — marginal (4–10 tok/s)
- **green** — usable (10–30 tok/s)
- **bright green** — fast (≥30 tok/s)

Score markers:
- `~` — score inherited/interpolated from model family
- `?` — no benchmark data available
- `!sr` — uploader-reported only, not independently verified

## How it works

1. **Hardware detection** — NVIDIA (nvidia-smi), Apple Silicon (system_profiler), CPU cores, RAM
2. **Model fetching** — HuggingFace API: text-generation + GGUF models, cached 6h
3. **Benchmark merging** — curated static scores + live HuggingFace evalResults
4. **VRAM estimation** — weights from params × bits_per_weight + KV cache + overhead
5. **Speed estimation** — memory bandwidth-bound tok/s with fit-type penalty
6. **Scoring** — benchmark × evidence × size_bonus × quant_penalty × fit_factor + speed_adj + trust_adj
7. **Ranking** — sort by score, deduplicate, take top N

## Scoring

Each model gets a 0–100 score:

| Factor | Effect | Description |
|--------|--------|-------------|
| Benchmark quality | core | Merged live + static benchmarks, weighted by evidence confidence |
| Model size | up to 35 | log₂-scaled world-knowledge proxy |
| Quantization | × penalty | Lower-bit quants discounted (Q2=0.6, Q3=0.75, Q4=0.88, Q5=0.95, Q8=0.98, FP=1.0) |
| Evidence confidence | ×0.55–1.0 | direct=1.0, variant=0.88, base_model=0.78, interpolated=0.65, self_reported=0.55 |
| Runtime fit | ×0.50–1.0 | full-gpu=1.0, partial-offload=0.72, cpu-only=0.50 |
| Speed | -8 to +8 | Usability gate vs tok/s floor |
| Source trust | +3 | Official-org bonus |

## Hardware support

| Platform | Detection method |
|----------|-----------------|
| NVIDIA | nvidia-smi |
| Apple Silicon | system_profiler + sysctl (unified memory = VRAM) |
| AMD | Stub (planned) |
| CPU-only | Fallback when no GPU detected |

GPU simulation (`--gpu`) uses a built-in spec table:

| GPU | VRAM | Bandwidth |
|-----|------|-----------|
| RTX 5090 | 32 GB | 1792 GB/s |
| RTX 4090 | 24 GB | 1008 GB/s |
| RTX 3090 | 24 GB | 936 GB/s |
| A100 80GB | 80 GB | 2039 GB/s |
| H100 | 80 GB | 3352 GB/s |

## Dependencies

- [clap](https://crates.io/crates/clap) — CLI argument parsing
- [reqwest](https://crates.io/crates/reqwest) — HTTP client (rustls-tls)
- [tokio](https://crates.io/crates/tokio) — async runtime
- [serde](https://crates.io/crates/serde) + [serde_json](https://crates.io/crates/serde_json) — serialization
- [owo-colors](https://crates.io/crates/owo-colors) — terminal colors
- [dirs](https://crates.io/crates/dirs) — cross-platform cache dir
- [chrono](https://crates.io/crates/chrono) — timestamps

## Development

```bash
git clone https://github.com/Andyyyy64/whichllm.git
cd whichllm
cargo build
cargo run -- --gpu "RTX 4090"
cargo run --release
```

## License

MIT
