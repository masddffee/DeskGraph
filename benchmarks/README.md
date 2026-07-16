# DeskGraph Benchmarks

Benchmark reports are evidence snapshots, not release claims. Each report must state its corpus, host class, toolchain, command, and missing evidence. Never compare results from different corpus versions as if they were the same workload.

## Synthetic lexical search

The workspace-only `deskgraph-search-benchmark` binary creates a new synthetic SQLite database through the real migrations, inserts Traditional Chinese/English metadata and content through the FTS synchronization triggers, and queries through the production retrieval API. It never reads user files and refuses to overwrite an existing database path.

```bash
cargo run -p deskgraph-search-benchmark --release --offline -- \
  --database /private/tmp/deskgraph-search-benchmark-v1.sqlite3 \
  --documents 10000 \
  --iterations 50
```

The generated database is intentionally left in place for inspection. Choose a new path for every rerun. The tool caps the corpus at 100,000 documents and iterations at 1,000 per query case.

This measures the FTS/retrieval boundary only. It does not measure filesystem scanning, extraction, OCR, embeddings, UI rendering, peak RSS, energy/thermal behavior, concurrent writes, macOS Intel, Windows, or Linux. A release benchmark must add those environments and a representative real-world corpus without committing private user data.
