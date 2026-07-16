# ADR-022: File Version Candidates Require Explicit Numeric Suffixes

- Status: Accepted
- Date: 2026-07-16

## Context

DeskGraph needs a deterministic `version_of` baseline before similarity or model signals exist. Modification time, directory proximity, equal extension, or vaguely similar names alone are not reliable enough to infer version direction. The existing relation parent table is restricted to exact duplicates, and exact-pair feedback intentionally persists across identical byte observations; carrying that feedback model directly to a directional version observation could preserve an old decision after filenames reverse direction.

## Decision

- Migration 0011 evolves the immutable relation parent to admit `exact_duplicate` and `version` while preserving every existing relation ID, exact-byte observation, and feedback event. Version observations are stored in a separate immutable table with their own fixed schema and provider.
- The first version rule evaluates only two explicit, current files in the same authorized scope. Both must pass canonical path, non-symlink/reparse, manifest identity, platform identity, metadata, and read-only open-handle validation before and after name analysis.
- Each UTF-8 filename must end its stem with exactly one explicit numeric suffix: `-vN`, `_vN`, ` vN`, or `.vN`, case-insensitive for `v`. `N` is 1–999999 with no leading zero. Both files must have the same NFC/lowercased non-empty base and extension and different version numbers. The smaller number is `older`; the larger is `newer`.
- Evidence records both current location snapshots, normalized base/extension, ordered version numbers, 9000 basis-point confidence, observation time, `system_rule`, provider `deskgraph.filename-version` version `1`, and `model_version: null`.
- The relation identity is the stable unordered scope/node pair; each observation stores the current older/newer direction. A later rename must pass live verification and append a new observation. Historical lists remain path-free and are marked `verification_required`.
- Version candidates remain `suggested` in this slice. Existing exact-duplicate feedback must not be applied to them. Evidence-bound directional feedback requires a separate accepted decision before `relation decide` can support version candidates.
- The slice performs no content read, similarity inference, membership assignment, merge, delete, rename, move, model call, or filesystem mutation and adds no registry dependency.

## Consequences

- Common explicit version filenames gain a conservative, explainable, local-only edge without chronology guesses or optional AI.
- The rule intentionally misses dates, words such as `final`, semantic revisions, and version numbers not expressed in the allowlisted suffixes.
- Rebuilding the relation parent is more migration work, but preserves one global relation ID space and avoids fragmented edge identity.
- Version correction, related/similarity signals, background discovery, evaluation corpora, and Project membership remain open.

## Rejected alternatives

- Infer version order from modification time, file size, directory order, or lexical filename order.
- Treat `final`, `copy`, dates, or arbitrary trailing numbers as version proof.
- Read or hash file contents in this metadata-only rule.
- Reuse exact-pair feedback for a directional observation without binding the decision to evidence.
- Create a second unrelated relation-ID namespace.
- Let a model or extracted content create the relation.
