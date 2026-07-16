# ADR-013 — Bounded PDF text uses structural provenance

- Status: Accepted
- Date: 2026-07-16
- Owners: DeskGraph maintainers

## Context

ADR-012 requires PDF text extraction to operate on a core-controlled source, ignore active content, enforce resource limits, preserve provenance, and publish atomically. PDF text produced through fonts, character maps, and content streams does not have an honest contiguous byte range in the original file. Reusing `0..0` or a compressed-object range would create false provenance and violate the extractor safety rules.

The parser must also bound both eager parsing and page text extraction. A source-byte cap alone does not stop compressed object, cross-reference, page-content, or `/ToUnicode` streams from expanding in memory. Candidate APIs must not receive a path, open attachments, execute actions, make network requests, launch processes, or add a native runtime requirement.

## Decision

- Add a tagged chunk provenance model. Byte-oriented providers store `byte_range { start, end }`; PDF chunks store `pdf_page { page_number, fragment_index }`. SQLite columns are nullable only according to the tag, and database checks reject mixed or incomplete provenance.
- Migrate existing content chunks forward in one SQLite transaction, preserving their exact byte ranges as `byte_range` provenance. No historical content is rewritten to a false structural location.
- Adopt exactly `lopdf 0.44.0` for the PDF provider with `default-features = false`. This excludes the default Rayon/date feature graph and keeps extraction single-provider, synchronous, and free of a native PDF runtime.
- Read the already-authorized open handle into the existing bounded source buffer. Parse only with `Document::load_mem_with_options`, strict mode, and a non-optional per-stream decompression limit.
- Reject both still-encrypted and automatically empty-password-decrypted PDFs with a fixed unsupported-encryption error. DeskGraph does not request, store, or infer PDF passwords in v0.1.
- Enforce an explicit page-count cap. Process one page at a time with `extract_text_with_limit`; apply cancellation and elapsed-time checks before and after each bounded page unit; enforce output/chunk limits while converting page text into chunks.
- Call only page enumeration and bounded text extraction APIs. Do not traverse `/OpenAction`, `/AA`, `/JavaScript`, `/Launch`, `/URI`, multimedia, file specifications, embedded-file name trees, or annotations. These objects remain inert parser data and are never surfaced as work instructions.
- Preserve the fixed `untrusted_extracted_text` trust class. A complete PDF result atomically replaces the prior active chunks; corrupt, encrypted, oversized, timed-out, or cancelled input publishes nothing.
- Keep an open risk for aggregate parser residency: `LoadOptions` bounds each eagerly decoded object/xref stream rather than exposing a whole-document aggregate-memory budget. The source cap, per-stream cap, page cap, sequential pages, stored-output cap, and 8 GB benchmark are all release gates; a measured breach requires replacing or isolating the parser without weakening limits.

## Verified dependency evidence

- Cargo and docs.rs identify `lopdf 0.44.0`, MIT, Rust 1.88+, with its official repository at `J-F-Liu/lopdf`. The release was published 2026-07-10 and the repository was active when reviewed on 2026-07-16.
- The selected no-default-feature runtime closure resolves 53 registry packages. Every package reports a license expression; the expressions are permissive or offer a permissive choice. Notices still remain part of the M9 SBOM/license review.
- `cargo audit --no-fetch` against that minimal lockfile and 1,160 cached RustSec advisories reported zero vulnerabilities or warnings. The upstream full-feature lockfile did contain `RUSTSEC-2026-0204` through `crossbeam-epoch 0.9.18`; that graph is rejected and absent when default features are disabled.
- The minimal crate test passed on macOS arm64 and `cargo check --target x86_64-pc-windows-msvc` passed. `lopdf` and the selected graph require no platform PDF library. macOS Intel and Linux remain remote CI/runtime gates.
- `pdf-extract 0.12.0` was rejected. Its public memory/path functions call unbounded `Document::load*` and document-output paths, depend on older `lopdf 0.42`, and expose no source, page-decompression, cancellation, or output budget at its API boundary.

## Consequences

PDF results can be explained by page without pretending that decoded Unicode maps back to source bytes. The schema can later add slide, sheet, cell, image-region, or OCR-region variants without overloading byte offsets.

The provider deliberately supports text-layer PDFs only. Scanned pages remain for the separately audited OCR provider. Some valid but non-conforming or encrypted PDFs will fail closed. Page text order and Unicode fidelity depend on the document's font maps and PDF structure, so fixtures and user-visible error states are required rather than silent guessing.

## Alternatives considered

- `pdf-extract 0.12.0` was rejected because its high-level API cannot enforce the mandatory limits.
- Unbounded `lopdf::Document::extract_text` was rejected; only the bounded counterpart is allowed.
- Enabling default features was rejected because it adds unnecessary parallel/date dependencies and the audited upstream lock currently contains a vulnerable `crossbeam-epoch` version.
- Storing fake byte offsets was rejected because it would make provenance untrustworthy.
- Passing a filesystem path or spawning an external PDF utility was rejected by ADR-012 and the extractor sandbox contract.

## Validation and revisit trigger

Before the PDF row can become verified, tests must cover: valid Traditional Chinese/English text, corrupt syntax, encrypted input, source/page/decompression/output limits, cancellation between pages, correct page provenance, atomic prior-version preservation, and inert JavaScript/action/attachment/external-reference objects. Full workspace format, lint, tests, Windows cross-check, dependency audit, and an 8 GB residency benchmark remain required evidence.
