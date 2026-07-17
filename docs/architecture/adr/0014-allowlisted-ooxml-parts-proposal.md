# ADR-014 — OOXML reads allowlisted parts through a bounded archive adapter

- Status: Accepted
- Date: 2026-07-17
- Owners: DeskGraph maintainers

## Context

DOCX, PPTX, and XLSX are ZIP containers whose XML, relationships, macros, embedded objects, and external references are untrusted document data. ADR-012 forbids a generic archive extractor and requires providers to receive a core-controlled source rather than an arbitrary path. Structural Office text also cannot be represented honestly by byte offsets in the compressed source.

A high-level Office library can hide archive traversal, relationship resolution, decompression, formula handling, or external-resource behavior behind a convenient API. DeskGraph instead needs one small, auditable layer that exposes enough ZIP and streaming XML primitives for the core to enforce its own entry, decompression, structure, output, time, and cancellation limits.

Official package metadata and published source identify `zip 8.6.0` and `quick-xml 0.41.0` as the current stable releases. An isolated exact lock, feature closure, source/API inspection, license inventory, RustSec scan, macOS arm64 test, and Windows x64 cross-check now pass. This ADR therefore approves only the exact feature set below; it does not approve a generic archive extractor or any higher-level Office library.

## Decision

- Use one shared, path-free OOXML adapter over a bounded in-memory `Read + Seek` source. Never unpack document entries to the filesystem.
- Use exactly `zip =8.6.0` with `default-features = false` and only `deflate-flate2-zlib-rs`. Stored entries require no feature. Reject prerelease `9.x`, archive writing/extraction-to-disk, encryption, and every other compression feature.
- Use exactly `quick-xml =0.41.0` with `default-features = false`. Use the streaming plain `Reader`, not `NsReader` or a whole-document DOM; DeskGraph compares only allowlisted OOXML local element names and does not need namespace resolution.
- Reject an archive before text parsing when it is encrypted, overlaps entries, exceeds the entry-count or claimed-total-size limits, contains an unsafe or duplicate selected name, uses an unsupported compression method, or exceeds the selected-entry, actual-decompressed, compression-ratio, time, or cancellation budget.
- Validate normalized enclosed entry names even though no entry is written to disk. Select only exact allowlisted parts; never recursively discover or follow relationship targets.
- DOCX initially reads only `word/document.xml`. PPTX reads numerically ordered `ppt/slides/slideN.xml`. XLSX reads numerically ordered `xl/worksheets/sheetN.xml` plus bounded `xl/sharedStrings.xml` when referenced. Additional parts require an explicit adapter change and fixtures.
- Do not read or execute VBA projects, macros, scripts, formulas, OLE objects, embedded packages, attachments, custom XML instructions, external links, hyperlinks, data connections, or relationship targets. Formulas remain inert and are never evaluated.
- Reject DTD declarations, processing instructions, unsupported encodings, excessive depth, excessive attributes, excessive events, oversized text nodes, and unrecognized general entities. Permit only XML predefined references and numeric character references after exact parser-API verification.
- Add explicit structural provenance variants for DOCX paragraphs, PPTX slides/fragments, and XLSX sheets/cells before publishing Office chunks. Never fabricate compressed-source byte offsets.
- Stage per-file output and atomically replace prior active chunks only after complete success. Preserve `untrusted_extracted_text`, fixed error codes, privacy-safe status, and cancellation checks between bounded archive entries and document units.

## Consequences

One constrained ZIP/XML boundary can serve the three v0.1 Office formats without giving a parser filesystem, network, process, or relationship-following capability. Resource policies and adversarial fixtures remain owned by DeskGraph rather than a high-level document library.

The first provider will intentionally omit some user-visible Office content such as headers, notes, charts, comments, and formula results unless each part is explicitly added later. Some valid documents using unusual XML encodings or compression methods will fail closed. XLSX shared strings require bounded random lookup or an explicitly budgeted in-memory table, which must be measured on 8 GB hardware.

The dependency-selection gate is complete, so implementation may depend on this Accepted ADR. The provider is still incomplete until every adversarial fixture and resource gate below passes; acceptance of this architecture is not evidence that DOCX, PPTX, or XLSX extraction works.

## Alternatives considered

- High-level DOCX/PPTX/XLSX crates are deferred because separate format stacks increase supply-chain surface and may hide relationship, archive, or allocation behavior that DeskGraph must control.
- A generic recursive ZIP extractor is rejected by ADR-012 because it exposes unrelated attachments and increases traversal and decompression risk.
- Extracting entries to temporary files is rejected because it broadens the filesystem boundary and creates cleanup and disclosure risk.
- Parsing the complete XML tree is rejected because document-controlled structure could cause unnecessary peak residency.
- Following OOXML relationships is rejected for the first slice because targets may be external, embedded, cyclic, or outside the minimal text-part allowlist.

## Verification evidence, provider gate, and revisit trigger

Completed before accepting the dependency decision or editing the workspace manifest:

1. An isolated lock resolves 14 registry packages for the two exact dependencies and selected features. Every package has a permissive license expression.
2. Published source confirms archive entry count, encryption flag, overlap detection, enclosed names, compression method, compressed/uncompressed sizes, bounded `Read`, and explicit XML `Event` variants for DTD, processing instructions, text, and general references.
3. `cargo audit --no-fetch --json` with 1,160 cached advisories reports zero vulnerabilities and zero warnings for the isolated lock. `quick-xml 0.41.0` contains the fix for `RUSTSEC-2026-0195`; the selected plain `Reader` also avoids the affected namespace-resolver path.
4. The isolated lock builds and tests on macOS arm64 and checks for `x86_64-pc-windows-msvc`. macOS Intel, Windows runtime, and Linux remain remote gates.

Required before marking the Office provider verified:

1. Pass valid Traditional Chinese/English DOCX, PPTX, and XLSX fixtures plus corrupt ZIP/XML, traversal, duplicate/overlap, encryption, unsupported compression, decompression ratio/size, DTD/entity, deep XML, macro-enabled, external-link, embedded-object, formula, cancellation, and structural-provenance fixtures.
2. Run the complete DeskGraph lockfile audit and all repository gates after integration.
3. Measure the worst selected corpus on documented 8 GB hardware before claiming M2 complete.

Revisit the design if the minimal archive feature graph pulls in an unacceptable advisory/license, the APIs cannot enforce actual decompression limits without unbounded allocation, or real-world Office fidelity requires relationship traversal. No revisit may weaken the controlled-source, no-execution, atomic-publication, or privacy invariants.
