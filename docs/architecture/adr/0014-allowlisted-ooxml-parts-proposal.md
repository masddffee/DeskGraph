# ADR-014 — OOXML reads allowlisted parts through a bounded archive adapter

- Status: Proposed
- Date: 2026-07-16
- Owners: DeskGraph maintainers

## Context

DOCX, PPTX, and XLSX are ZIP containers whose XML, relationships, macros, embedded objects, and external references are untrusted document data. ADR-012 forbids a generic archive extractor and requires providers to receive a core-controlled source rather than an arbitrary path. Structural Office text also cannot be represented honestly by byte offsets in the compressed source.

A high-level Office library can hide archive traversal, relationship resolution, decompression, formula handling, or external-resource behavior behind a convenient API. DeskGraph instead needs one small, auditable layer that exposes enough ZIP and streaming XML primitives for the core to enforce its own entry, decompression, structure, output, time, and cancellation limits.

Official published documentation identifies `zip 8.6.0` and `quick-xml 0.41.0` as current stable candidates. This proposal does **not** approve or add either dependency. The exact minimal dependency closures, resolved licenses, RustSec results, selected feature behavior, and cross-platform build evidence could not be generated after local Cargo registry access was denied by the exhausted tool quota.

## Proposed decision

- Use one shared, path-free OOXML adapter over a bounded in-memory `Read + Seek` source. Never unpack document entries to the filesystem.
- Consider exactly `zip 8.6.0` with default features disabled and only the smallest verified read feature required for stored and DEFLATE entries. Reject prerelease `9.x` and every compression or crypto feature not required by the accepted fixture corpus.
- Consider exactly `quick-xml 0.41.0` with default features disabled. Use its streaming pull reader; do not construct a whole-document DOM.
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

No implementation may depend on this Proposed ADR until the dependency gate below passes. Planning and test-fixture design may proceed without changing a manifest or lockfile.

## Alternatives considered

- High-level DOCX/PPTX/XLSX crates are deferred because separate format stacks increase supply-chain surface and may hide relationship, archive, or allocation behavior that DeskGraph must control.
- A generic recursive ZIP extractor is rejected by ADR-012 because it exposes unrelated attachments and increases traversal and decompression risk.
- Extracting entries to temporary files is rejected because it broadens the filesystem boundary and creates cleanup and disclosure risk.
- Parsing the complete XML tree is rejected because document-controlled structure could cause unnecessary peak residency.
- Following OOXML relationships is rejected for the first slice because targets may be external, embedded, cyclic, or outside the minimal text-part allowlist.

## Acceptance gate and revisit trigger

Before changing this ADR to Accepted or editing any dependency manifest:

1. Generate isolated exact lockfiles for the proposed feature sets and record every resolved package and license.
2. Inspect the published source for archive construction, entry lookup, encryption, overlap, path normalization, compressed/uncompressed sizes, actual bounded reads, XML events, entity decoding, encoding, and error behavior.
3. Run `cargo audit --no-fetch` against the isolated closures and then the complete DeskGraph lockfile.
4. Compile the minimal adapters on macOS arm64 and Windows x64; keep macOS Intel and Linux as remote CI/runtime gates.
5. Pass valid Traditional Chinese/English DOCX, PPTX, and XLSX fixtures plus corrupt ZIP/XML, traversal, duplicate/overlap, encryption, unsupported compression, decompression ratio/size, DTD/entity, deep XML, macro-enabled, external-link, embedded-object, formula, cancellation, and structural-provenance fixtures.
6. Measure the worst selected corpus on documented 8 GB hardware before claiming M2 complete.

Revisit the design if the minimal archive feature graph pulls in an unacceptable advisory/license, the APIs cannot enforce actual decompression limits without unbounded allocation, or real-world Office fidelity requires relationship traversal. No revisit may weaken the controlled-source, no-execution, atomic-publication, or privacy invariants.
