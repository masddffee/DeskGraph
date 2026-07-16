# Phase 09 — Optional Local AI Providers

Implement optional local inference without making it required.

Embedding:
- local multilingual small model
- lazy load
- int8 or otherwise memory-efficient artifact
- model manager and checksum
- benchmark memory and throughput

LLM:
- llama.cpp adapter
- downloadable 1B–2B class GGUF candidate
- non-thinking / efficient mode
- grammar-constrained JSON
- timeout, cancellation and fallback
- unload model when idle
- never send filesystem handles or executor capabilities

Model manager:
- show size, license, source, checksum and last-used date
- allow removal
- keep app usable without model

Acceptance:
- 8GB reference test
- malformed model output cannot affect files
- provider failure degrades to deterministic pipeline
