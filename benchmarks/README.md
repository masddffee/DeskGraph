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

The workspace-only `deskgraph-ocr-evaluator` scores one provider-run JSON against one versioned Traditional Chinese/English corpus JSON. The evaluator reads no image or user-file path, writes no file, chooses no winner, and emits a path/text-free JSON report with overall and tag-sliced micro CER/whitespace-token error, separate attempt/completed latency, failure-code histograms, no-text false positives, observation validity, externally reported RSS, and explicit missing evidence.

```bash
cargo run -p deskgraph-ocr-evaluator --release --offline -- \
  --corpus benchmarks/ocr/corpus-v1.example.json \
  --run benchmarks/ocr/run-v1.example.json
```

Both inputs are capped at 64 MiB, use strict versioned serde contracts, bind each case to an image SHA-256, and cap case, text, observation, and edit-distance work. The run must cite the evaluator-computed SHA-256 of the exact corpus input; the report also includes both input digests, so changing ground truth, tags, order, or normalization creates distinguishable evidence. The two checked-in synthetic examples are executable contract fixtures and are validated by unit tests. They are not an accuracy corpus and do not prove provider quality.

Text comparison uses NFC plus collapsed Unicode whitespace. CER counts Unicode scalar values. The secondary token metric counts whitespace-delimited tokens and is deliberately not called WER because that segmentation is weak for unspaced Chinese; cross-language selection must use CER as the primary accuracy gate. A run supplies one canonical `recognized_text` per completed case, while observations are evaluated only as spatial/confidence evidence, so later observation reordering cannot change text accuracy. `text_reconstruction` makes the runner rule explicit; version 1 joins provider-returned observations with newlines before the evaluator's whitespace normalization. Reports preserve exact edit/reference counts and integer parts-per-million rates rather than relying on rounded floats.

The evaluator validates provider-produced output only. It cannot prove that a runner really measured RSS, honored cancellation, unloaded a model, packaged correctly, or produced spatially accurate boxes; those remain provider/platform E2E gates. Every RSS value carries its own source and an explicit whole-process or provider-sidecar scope; missing readings stay missing. Box validity means only bounded normalized geometry, not IoU or localization quality. Do not commit a real OCR corpus until every image has source, license, checksum, privacy review, language/difficulty tags, and a documented split.

### macOS Apple Vision runner

`deskgraph-macos-vision-runner` is the first provider runner. It is evidence tooling, not an application route or fallback. It accepts only an explicit absolute image root plus a private asset manifest, derives no path from `case_id`, reads no environment fallback, and sends only hash-bound owned PNG/JPEG bytes through the same production OCR request/output validator used by extraction jobs.

The private asset manifest is strict JSON:

```json
{
  "api_version": "deskgraph.ocr-assets.v1",
  "corpus_id": "private-corpus-v1",
  "corpus_input_sha256": "<sha256 of exact corpus JSON bytes>",
  "assets": [
    {
      "case_id": "mixed-001",
      "relative_path": "mixed/001.png",
      "image_sha256": "<same image sha256 as the corpus case>",
      "format": "png"
    }
  ]
}
```

Keep the manifest, images, and generated run JSON outside the repository. Relative asset paths are component-validated and opened relative to a held, non-symlink root directory descriptor; path replacement cannot redirect reads outside that authorized directory. The runner rejects hidden/system, symlink/reparse, non-regular, changed, oversized, format-mismatched, dimension-bomb, or checksum-mismatched inputs before Vision receives bytes. The whole corpus is capped at 512 MiB and one hour; each case is capped at 32 MiB and at most 60 seconds. Output contains recognized OCR text, so it is sensitive local evidence: the runner holds and revalidates an output-directory descriptor, requires a new absolute output path, creates a private mode-`0600` temporary with `O_EXCL`, and atomically publishes through macOS `renameatx_np(RENAME_EXCL)`. It never deletes or overwrites any file. A failed write or publish may deliberately leave the private `.<run-id>.<pid>.ocr-partial` artifact for explicit user cleanup instead of risking a name-replacement deletion race. Terminal output includes neither OCR text nor paths.

```bash
cargo run -p deskgraph-ocr-evaluator \
  --bin deskgraph-macos-vision-runner \
  --release --offline -- \
  --corpus /absolute/private/corpus.json \
  --asset-manifest /absolute/private/assets.json \
  --images-root /absolute/private/images \
  --output /absolute/private/new-run.json \
  --run-id macos-vision-arm64-1 \
  --os-version "macOS 26.5.1" \
  --cpu-model "Apple M3" \
  --ram-bytes 8589934592 \
  --rust-toolchain "rustc 1.97.0 (2d8144b78 2026-07-07)" \
  --deskgraph-commit "<40 lowercase hex commit>" \
  --runtime-revision "macOS Vision 26.5.1"
```

Apple Vision can fail inside a restricted development sandbox even when the same binary succeeds outside it. Record that boundary honestly; an unsandboxed local pass is development evidence only, not packaged entitlement, clean-machine, Intel/Universal, cancellation, RSS, or release evidence.
