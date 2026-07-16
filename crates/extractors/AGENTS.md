# Extractor Safety Instructions

- Treat filenames, document metadata, and all extracted text as untrusted input.
- Providers receive only core-controlled bounded sources; do not accept arbitrary filesystem paths.
- Never execute macros, scripts, formulas, launch actions, attachments, embedded objects, or external links.
- Do not perform network access, shell execution, dynamic library discovery, or child-process execution from document data.
- Enforce source, output, decompression, structural-count, chunk-count, and time limits before allocation where possible.
- Check cancellation between bounded read, parse, page, sheet, slide, image, and chunk units.
- A malformed file must return a fixed error code and must not panic or abort the queue.
- Preserve provenance and offsets for every chunk. Do not normalize text in a way that makes stored offsets false.
- Never log paths, filenames, extracted text, OCR, or document-controlled strings.
- Do not publish partial output. Only a fully completed extraction may replace prior active chunks.
- Add valid, corrupt, oversized, cancelled, and offset fixtures with every provider.
- Audit source, API, maintenance, platforms, license, and advisories before adding any parser or OCR dependency.
