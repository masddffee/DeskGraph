# Scanner safety instructions

- Scanning is metadata-only until the M2 extractor boundary.
- Start only from a persisted, explicitly authorized canonical directory.
- Validate every discovered canonical path remains beneath the authorized root.
- Inspect `symlink_metadata` before canonicalization. Never follow symlinks, junctions, reparse points, aliases, or shortcuts during traversal.
- Skip hidden entries and protected system roots by default; record bounded issue codes without logging paths.
- Path strings are sensitive. They may exist in the local manifest and explicit scope UI, but never in logs, telemetry, network requests, or generic error messages.
- Stable filesystem identity and location are separate concepts. Never replace identity with a path when platform metadata is available.
- Do not read file contents, execute files, or invoke external programs.
