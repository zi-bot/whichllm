# whichllm Rust — v1→B Expansion Design

Extends v1 minimal with additional CLI commands, filters, and output formats.

## New Commands

### `whichllm plan <model>`
Reverse lookup: what GPU do I need for a specific model?
- Parse model name (fuzzy match against fetched models)
- Show min VRAM needed per quant variant
- List GPUs from the simulation table that can run it
- Output: colored table or JSON

### `whichllm upgrade <gpu1> [<gpu2> ...]`
Compare your current machine against candidate GPUs.
- For each GPU, rank top model and show score/speed
- Side-by-side comparison table
- `--top N` applies

### `whichllm hardware`
Show hardware info only, no model fetching.

## New CLI Flags

- `--gpu-only` / `--fit full-gpu` — only show models that fit entirely in GPU VRAM (hide partial-offload and CPU-only)
- `--fit <type>` — explicit fit filter: `full-gpu`, `gpu` (same), `any` (default)
- `--cpu-only` — force CPU-only mode (ignore GPUs)
- `--markdown` / `-m` — output as GitHub-Flavored Markdown table
- `--vram-headroom <size>` — reserve extra VRAM (e.g. `1GB`, `1.5GB`). Subtract from available VRAM before fit check
- `--ram-budget <size|available>` — limit system RAM usable for offload
- `--profile <type>` — task profile filter: `general`, `coding`, `vision`, `math`
- `--quant <q>` — filter by quantization (e.g. `Q4_K_M`)
- `--context-length <len>` — override context length for KV cache estimation (e.g. `8k`, `32768`)
- `--min-speed <tps>` — exact tok/s floor (replaces `--speed usable/fast` which was coarse)
- `--details` — showDownloads metadata instead of runtime columns

## Markdown Output

GFM table format for `--markdown`:
```
| # | Model | Params | Quant | Fit | Score | Speed |
|---|-------|--------|-------|-----|-------|-------|
| 1 | Qwen/Qwen3-32B | 32.0B | Q5_K_M | GPU | 90.8 | 91 t/s |
```

## Profile Filtering

Static model tags for profiles:
- `coding`: models known strong at code (DeepSeek-Coder, Qwen-Coder, Phi)
- `vision`: multimodal models (LLaVA, Qwen-VL)
- `math`: math-specialized (DeepSeek-Math, Mathstral)
- `general`: all models (default)

Implemented as a curated list of model ID prefixes/tags in a static JSON.

## Context Length Override

KV cache scales linearly with context length. `--context-length 64k` increases KV cache estimate, potentially pushing models from full-gpu to partial-offload.

Default context: 4096 tokens.
