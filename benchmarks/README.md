# DeskGraph Benchmarks

Benchmark reports are evidence snapshots, not release claims. Each report must identify its exact corpus, host class, toolchain, harness/version, path-free command digest, and missing evidence; retain any full local command only in a private evidence log. Never compare results from different corpus versions as if they were the same workload.

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

## OCR provider evaluation contract

The workspace-only `deskgraph-ocr-evaluator` scores one provider-run JSON against one versioned Traditional Chinese/English corpus JSON. It reads no image or user-file path, writes no file, chooses no winner, and emits a path/text-free JSON report with overall and tag-sliced micro CER/whitespace-token error, separate attempt/completed latency, failure-code histograms, no-text false positives, observation validity, externally reported RSS, and explicit missing evidence.

```bash
cargo run -p deskgraph-ocr-evaluator --release --offline -- \
  --corpus benchmarks/ocr/corpus-v1.example.json \
  --run benchmarks/ocr/run-v1.example.json
```

Both inputs are capped at 64 MiB, use strict versioned serde contracts, bind each case to an image SHA-256, and cap case, text, observation, and edit-distance work. The run must cite the evaluator-computed SHA-256 of the exact corpus input; the report also includes both input digests, so changing ground truth, tags, order, or normalization creates distinguishable evidence. The two checked-in synthetic examples are executable contract fixtures and are validated by unit tests. They are not an accuracy corpus and do not prove provider quality.

Text comparison uses NFC plus collapsed Unicode whitespace. CER counts Unicode scalar values. The secondary token metric counts whitespace-delimited tokens and is deliberately not called WER because that segmentation is weak for unspaced Chinese; cross-language selection must use CER as the primary accuracy gate. A run supplies one canonical `recognized_text` per completed case, while observations are evaluated only as spatial/confidence evidence, so provider-specific observation ordering cannot change text accuracy. Reports preserve exact edit/reference counts and integer parts-per-million rates rather than relying on rounded floats.

The evaluator validates provider-produced output only. It cannot prove that a runner really measured RSS, honored cancellation, unloaded a model, packaged correctly, or produced spatially accurate boxes; those remain provider/platform E2E gates. Every RSS value carries its own source and an explicit whole-process or provider-sidecar scope; missing readings stay missing. Box validity means only bounded normalized geometry, not IoU or localization quality. Do not commit a real OCR corpus until every image has source, license, checksum, privacy review, language/difficulty tags, and a documented split.
