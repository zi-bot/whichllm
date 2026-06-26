# whichllm Rust — v1 Minimal Design

Find the best local LLM that runs on your hardware. Auto-detect GPU/CPU/RAM, fetch models from HuggingFace, rank by benchmark quality + fit, print results.

## Architecture

Single binary crate, modules:

```
src/
├── main.rs           # CLI entry, clap args, tokio spawn
├── cli.rs            # Clap derive struct (Args)
├── hardware/
│   ├── mod.rs        # detect() → HardwareInfo
│   ├── nvidia.rs     # NVML binding
│   ├── amd.rs        # ROCm/sysfs
│   ├── apple.rs      # Metal/ioreg
│   ├── cpu.rs        # CPU info, cores, AVX
│   ├── memory.rs     # RAM, disk
│   └── types.rs      # GPUInfo, HardwareInfo
├── models/
│   ├── mod.rs        # fetch_models() orchestration
│   ├── hf_api.rs     # HuggingFace HTTP queries
│   ├── gguf.rs       # Parse siblings for GGUF variants + sizes
│   ├── cache.rs      # JSON file cache with TTL (dirs crate)
│   └── types.rs      # ModelInfo, GGUFVariant
├── engine/
│   ├── mod.rs        # rank() top-level
│   ├── vram.rs       # VRAM estimation (weights + KV + overhead)
│   ├── speed.rs      # tok/s from bandwidth
│   ├── scoring.rs    # Score formula (benchmark, size, quant, fit, speed)
│   └── types.rs      # RankResult, CompatibilityInfo
├── benchmarks/
│   ├── mod.rs        # merge_benchmarks()
│   ├── static.rs     # include_str! curated JSON
│   └── types.rs      # BenchmarkEntry, Evidence
└── output/
    └── mod.rs        # print_ranking() with owo-colors
```

Data flow: `main` → `hardware::detect()` → `models::fetch_models()` → `benchmarks::merge_benchmarks()` → `engine::rank()` → `output::print_ranking()`

Dependencies: clap (derive), reqwest (json, rustls-tls), tokio (rt, macros), serde + serde_json, owo-colors, dirs, chrono

## Hardware Detection

- `detect()` tries each GPU backend: NVIDIA (NVML FFI or nvidia-smi subprocess), AMD (ROCm /sys/class/drm), Apple (ioreg/Metal)
- Fallback: CPU-only with system RAM
- `--gpu "RTX 4090"` creates synthetic GPU via name→VRAM+bandwidth lookup table (static JSON)
- Multi-GPU: sum VRAM, min bandwidth
- Output: `HardwareInfo { gpus: Vec<GpuInfo>, cpu, ram_gb, disk_free_gb, os }`

## Model Fetching

- Two HF API calls: text-generation sorted by downloads, gguf filter sorted by downloads
- Parse model.siblings for *.gguf filenames — extract quant tag (Q4_K_M etc) and file size
- Cache as JSON in dirs::cache_dir()/whichllm/ with 6h TTL
- --refresh bypasses cache
- Retry with backoff on 429

## Benchmark Merging

- Static curated JSON (include_str!) with ~200 entries: {model_id, score, source, confidence, date}
- HuggingFace evalResults from model cards: parse when present, merge with static
- Merge: static baseline, live overrides when confidence higher and data newer
- Evidence levels: direct (1.0), variant (0.88), base_model (0.78), interpolated (0.65), self_reported (0.55)

## Scoring & Ranking

Score 0-100 = benchmark_score × size_bonus × quant_penalty × fit_factor × evidence_confidence + speed_adj + trust_adj

- Size bonus: log2(params_gb) × scale, cap 35
- Quant penalty: Q2=0.6, Q3=0.75, Q4=0.88, Q5=0.95, Q8=0.98, FP=1.0
- Fit factor: full-gpu=1.0, partial-offload=0.72, cpu-only=0.50
- Speed: estimated tok/s from GPU bandwidth × quant efficiency ÷ active params
- Speed color: <4 red, 4-10 yellow, 10-30 green, 30+ bright green
- Sort by score desc, take --top N (default 10)

## VRAM Estimation

- vram = weights_bytes + kv_cache + activations + overhead
- Weights: file size from GGUF sibling (accurate), or params × bytes_per_param(quant) (formula)
- KV cache: 2 × layers × 2 × kv_heads × head_dim × seq_len × dtype_bytes, GQA-aware
- Overhead: ~500MB fixed
- Compatibility: full-gpu if vram ≤ gpu_vram, partial-offload if vram ≤ gpu_vram + sys_ram, else cpu-only

## CLI & Output

```
whichllm                   # auto-detect, rank, print
whichllm --gpu "RTX 4090"  # simulate
whichllm --top 20          # more results
whichllm --json            # JSON output
whichllm --refresh         # bypass cache
whichllm --speed usable    # min 10 tok/s
```

Print format:
```
#1  Qwen/Qwen3-32B  32.0B  Q4_K_M  score 83.0  31 t/s
```

Speed colored by threshold. Score markers: ~ inferred, ? no data.
