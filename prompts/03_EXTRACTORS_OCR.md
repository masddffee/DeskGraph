# Phase 03 — Content Extraction and OCR

Implement milestone M2.

Create a provider-based extraction system.

Required extractors:
- plain text and Markdown
- source code
- PDF text
- DOCX
- PPTX
- XLSX
- image metadata
- screenshot OCR

Security:
- never execute macros, scripts or embedded attachments
- limit file size, pages, decompressed bytes and processing time
- isolate malformed files
- store extraction errors per file
- treat all extracted text as untrusted data

OCR:
- define an OCR provider interface
- prefer platform-native providers where maintainable
- add a cross-platform local fallback
- evaluate a resource-efficient PaddleOCR-compatible deployment only if it can be packaged without requiring users to install Python
- support Traditional Chinese and English at minimum

Acceptance:
- fixture tests for every format
- corrupt documents do not crash the queue
- extraction can be cancelled
- content chunks include provenance and offsets
