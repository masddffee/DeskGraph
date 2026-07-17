# ADR-024: Native-first OCR providers with a packaged in-process fallback

- Status: Accepted
- Date: 2026-07-17

## Context

DeskGraph must extract Traditional Chinese and English screenshot text while remaining local-first and useful without Python, Docker, an API key, a downloaded LLM, or a separately installed OCR service. ADR-012 requires OCR to use a separate provider boundary that receives only core-controlled bounded image data. OCR output is untrusted, must retain honest spatial provenance, and may replace prior active OCR text only after complete success.

The provider choice must also cover macOS arm64 and Intel, Windows x64, and an experimental Linux path without making Linux delay macOS or Windows. A platform-native API may not have the requested language installed, and an opaque OS service cannot be described as unloadable merely because DeskGraph drops its request objects.

## Decision

- Use a native-first provider stack: Apple Vision on macOS and `Windows.Media.Ocr` on Windows. Every native provider performs a runtime capability check for `zh-Hant`/Traditional Chinese and English before accepting work. Missing language capability is a fixed provider-unavailable error, not a silent language downgrade.
- Use a separate `OcrProvider` interface. Providers receive core-validated bounded encoded bytes or a core-owned bounded raster plus dimensions, pixel format, orientation, deadline, and cancellation token. They never receive a path, URL, environment-controlled model directory, network client, process capability, or arbitrary file handle.
- The macOS adapter uses Vision's `VNRecognizeTextRequest` and `VNImageRequestHandler.initWithData:options:` through exact `objc2-vision 0.3.2`. Only the minimal Apple-framework features are enabled, and the dependency is target-specific. The provider checks supported languages at runtime and requests `zh-Hant` before `en-US`, because the verified mixed-language fixture requires that priority on the current runtime.
- The Windows adapter will use `Windows.Media.Ocr.OcrEngine` and `SoftwareBitmap` through an exact, feature-minimized `windows` binding only after its isolated dependency, API, Language Feature on Demand, memory, cancellation, and Windows x64 runtime gates pass. This ADR does not preapprove that dependency.
- The cross-platform fallback is an in-process, bytes-only Tesseract adapter with packaged `eng` and `chi_tra` trained data. It may use neither the Tesseract CLI nor ambient data/dynamic-library discovery. Tesseract, Leptonica, binding, trained-data versions, hashes, multi-language initialization, packaging, cancellation, memory, notices, and all target builds remain separate acceptance gates; no fallback dependency is accepted by this ADR.
- PaddleOCR with ONNX Runtime is deferred to a later optional quality provider. It may be reconsidered only with DeskGraph's own screenshot corpus, macOS packaging evidence, telemetry-disabled runtime evidence, checksums, and unload/memory measurements. Pure-Rust `ocrs` is rejected for v0.1 because its current official model support is Latin-only and does not meet the Traditional Chinese requirement.
- OCR inputs are independently bounded below the generic image-metadata ceiling. Source bytes, dimensions, total pixels, output bytes, observation/chunk counts, and active processing time are checked before atomic publication. Cancellation and deadline requests must reach native work where the platform API permits; no partial native result is published after cancellation or timeout.
- Each stored OCR chunk records provider/version, source snapshot, one-based observation number, fragment index, normalized bounding box, confidence, and `untrusted_extracted_text`. An observation without valid bounded spatial/confidence provenance is rejected instead of receiving fabricated byte offsets.
- Native APIs may cache OS models. DeskGraph may claim only that its own provider objects and buffers are dropped after a job. Release evidence must measure RSS before, during, and after OCR and must not equate object drop with unloading an OS-owned model.

## Consequences

- macOS and Windows can use their maintained local platform OCR without shipping a model in the primary path, while the provider contract still admits a deterministic packaged fallback.
- The core remains functional when OCR is disabled or unavailable. OCR cannot become a hidden requirement for manifest scan, metadata/FTS search, Folder Profiles, or safe organization.
- The first implementation slice may ship only macOS Vision code evidence; it cannot close M2 or the cross-platform fallback requirement. Windows, Tesseract, representative corpus quality, Intel, clean-machine, 8 GB, installer, SBOM, and checksum evidence remain explicit gates.
- Apple Vision processing can fail in a restricted runner even when the runtime language probe succeeds. The 2026-07-17 local spike returned a nil platform error inside the restricted sandbox but recognized `DeskGraph OCR` and `桌面圖譜 安全整理` from the same controlled PNG through both Swift and Rust outside that sandbox. Runtime tests must expose, not hide, this environment boundary.

## Rejected alternatives

- Require Python, Docker, Ollama, a local LLM, or an external OCR service.
- Pass screenshot paths or URLs directly to native frameworks.
- Spawn Tesseract or another OCR CLI.
- Adopt one cross-platform model/runtime before native capability and resource measurements.
- Store OCR text with fake byte/page offsets or publish partial observations.
- Treat declared platform language support, object destruction, or an upstream benchmark as DeskGraph release evidence.

## Verification and revisit trigger

The dependency-selection spike compiled the published `objc2-vision 0.3.2` source with Rust 1.97 and only `VNObservation`, `VNRecognizeTextRequest`, `VNRequest`, `VNRequestHandler`, `VNTypes`, `block2`, `objc2-core-foundation`, and required Foundation features. A progress handler that reads an atomic cancellation flag and calls `VNRequest.cancel`, plus normalized bounding-box access, compiles; the mixed-language runtime probe returned both text observations with valid boxes. Runtime cancellation remains an implementation test, not accepted evidence. The complete nine-package isolated lock had no cached RustSec finding, and only `objc2-vision` is absent from DeskGraph's existing lock. The published crate archive SHA-256 was `bfc194758a2d5d7540b1ad283bfb9ca318ec608991892326e95b428230b2689b`.

Before any OCR provider is called complete, pass valid mixed zh-Hant/English, no-text, corrupt, extension/signature mismatch, oversized source/pixels/output, invalid provenance, cancellation, deadline, source-change, atomic replacement, migration preservation, FTS, and privacy-redaction fixtures. Revisit this decision if either native API cannot consume bounded bytes/raster, cannot expose the required languages, cannot meet the release memory budget, or cannot provide a safe cancellation boundary.
