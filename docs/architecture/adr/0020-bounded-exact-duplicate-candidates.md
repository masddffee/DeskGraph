# ADR-020: Exact Duplicate Candidates Use Bounded Byte Equality

- Status: Accepted
- Date: 2026-07-16

## Context

M4 requires explainable duplicate and version relations, but DeskGraph does not yet have an audited content-hash dependency or a general relation-scoring model. Filename, size, extracted-text, or non-cryptographic hash equality would create false duplicate claims. Reading an unlimited file or trusting a caller-supplied digest would violate the resource and provenance boundaries.

## Decision

- The first file-relation slice supports only `exact_duplicate` candidates for two different present file identities in one explicitly authorized scope.
- Both explicit paths must be absolute, canonical, non-symlink/reparse files inside the current canonical scope and present in the completed manifest. Platform identity, size, modified time, and a read-only open-handle identity must match the manifest before comparison and again after comparison.
- Files must be non-empty, equal in size, and at most 64 MiB. Rust compares the complete byte streams in fixed 64 KiB chunks with a five-second cooperative deadline. Size mismatch, any differing byte, short/extra read, timeout, stale metadata, identity change, or read error produces no relation observation.
- Hard links or two paths resolving to the same stable node are not duplicate candidates. Candidate endpoints are ordered by stable node ID so reversed input reuses the same relation identity.
- Migration 0009 stores an immutable stable relation identity plus append-only immutable observations. Each observation records both current location snapshots, compared byte count, 10,000-basis-point confidence, observation time, `system_rule`, fixed byte-equality provider/version, and no model.
- The database revalidates both current manifest snapshots in the same transaction that appends the observation. The Rust service owns byte comparison; callers cannot select another provider, comparison kind, confidence, or model.
- Every result remains `suggested`. The slice performs no merge, deletion, deduplication action, file membership, cross-root learning, fuzzy similarity, version inference, or filesystem mutation.
- Explicit duplicate/verify responses may contain the two current authorized paths. Structured logs contain only relation/scope/node IDs, compared bytes, and fixed state; no path, filename, or content is logged.
- No registry package, hashing library, model, API, network client, Python, Docker, or platform runtime is added.

## Consequences

- Exact equality has a clear deterministic explanation without presenting metadata or extracted-text similarity as content identity.
- The 64 MiB and cooperative timeout bounds make this a safe first slice, not complete duplicate discovery or a release-scale scanner.
- A later explicit verification appends a new observation; if either source no longer matches the manifest or byte equality, no current result is returned and the prior immutable history remains.
- Background discovery, larger-file hashing, fuzzy similarity, version relations, relation feedback, and current-data indexing need separate audited designs and evaluation.

## Rejected alternatives

- Treat equal names, sizes, timestamps, or extracted text as exact duplicates.
- Store or trust a caller-supplied digest without reading both current authorized files.
- Add a hashing dependency before source, API, license, maintenance, platform, and RustSec audit.
- Compare unlimited files or spawn an unbounded worker.
- Treat hard-link aliases as two duplicate files.
- Automatically merge, delete, move, rename, or otherwise organize duplicate candidates.
