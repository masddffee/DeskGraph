# ADR-033: Authorization Is an Explicit Coverage Set Minus Exclusions

- Status: Accepted
- Date: 2026-07-19

## Context

DeskGraph exists to organize a meaningful portion of a computer. Requiring a separate onboarding decision for every useful folder creates avoidable setup friction and makes missing results likely. The opposite model—silently reading the whole home directory or disk and asking the user to opt out afterward—would collect sensitive metadata before the user understands the boundary and cannot be implemented consistently through normal store-safe platform permissions.

Platform and product evidence establishes three distinct trust models:

- Apple App Sandbox does not give a third-party app unrestricted Home access. User-selected folders may be retained with security-scoped bookmarks, while Full Disk Access is a separate user-controlled system permission with materially broader reach. Apple also requires data minimization and Mac App Store sandboxing. See [App Sandbox](https://developer.apple.com/documentation/security/app-sandbox), [Accessing files from the macOS App Sandbox](https://developer.apple.com/documentation/security/accessing-files-from-the-macos-app-sandbox), [Files & Folders privacy](https://support.apple.com/en-lamr/guide/mac-help/mchld5a35146/mac), and [App Review Guidelines](https://developer.apple.com/app-store/review/guidelines/).
- Microsoft recommends File/Folder Picker plus retained access for ordinary packaged-app file access. `broadFileSystemAccess` is restricted, user-revocable, off by default, and requires Store justification that pickers are insufficient. See [File access permissions](https://learn.microsoft.com/en-us/windows/apps/develop/files/file-access-permissions) and [App capability declarations](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/app-capability-declarations).
- OS-owned indexes can use broader defaults because the operating system is the trust boundary. Windows Search offers Classic and Enhanced coverage, while Spotlight offers Search Privacy exclusions. High-sensitivity Microsoft Recall instead requires an explicit feature opt-in and then offers pause, deletion, and filters. These patterns support one understandable broad consent followed by visible controls; they do not justify silent third-party full-disk access. See [Windows search indexing](https://support.microsoft.com/en-us/windows/experience/performance-optimization/search-indexing-in-windows), [Spotlight Search Privacy](https://support.apple.com/en-lamr/guide/mac-help/mchl1bb43b84/mac), and [Recall privacy controls](https://support.microsoft.com/en-us/windows/privacy/privacy-and-control-over-your-recall-experience).

Third-party products demonstrate the usability trade-off but are not equivalent security precedents. [Everything](https://www.voidtools.com/en-us/support/everything/indexes/) indexes every filename on selected Windows volumes and then applies exclusions; [Raycast File Search](https://manual.raycast.com/file-search) may start from Home with ignore rules. Neither has DeskGraph's combined content extraction, OCR, graph, MCP, and future organization-action surface. [DEVONthink](https://download.devontechnologies.com/download/devonthink/3.8.2/DEVONthink.help/Contents/Resources/pgs/inandout-import.html) explicitly warns against indexing an entire home directory or disk. DeskGraph therefore needs lower setup friction without adopting their broad-by-default trust assumptions.

The already pinned `tauri-plugin-dialog 2.7.1` exposes `blocking_pick_folders() -> Option<Vec<FilePath>>`; its macOS and Windows backends use native multiple-directory selection. DeskGraph now calls that plural API for a one-dialog coverage-set flow without adding a dependency. This API still returns paths rather than the original macOS `NSURL` or a Windows `StorageFolder` access token. Signed macOS security-scoped-bookmark behavior and a packaged Windows retained-access adapter therefore remain separate release evidence rather than being inferred from the picker UX.

## Decision

### Authorization model

- Replace the product wording and UX assumption “authorize one folder at a time” with an **explicit coverage set**. The effective readable set is:

  `effective coverage = union(active user-confirmed roots) - union(active hard exclusions)`

- First-run setup offers a single review of recommended main folders such as Desktop, Documents, Downloads, Pictures, and the platform screenshots location. Nothing becomes active until the user confirms the displayed set through one native multi-folder selection or its platform-equivalent short flow. Canonical duplicates and redundant nested selections are resolved visibly before commit.
- A user may instead explicitly select a broader root such as Home. This is an advanced, clearly labeled choice with a pre-scan coverage/exclusion review; it is not the default and never implies Full Disk Access.
- DeskGraph does not request macOS Full Disk Access or Windows `broadFileSystemAccess` in the default v0.1 flow. A future broad-access mode requires a separate ADR, distribution-channel review, purpose limitation, runtime evidence, revocation UI, and Store justification.
- Selecting a coverage set authorizes metadata discovery only. Content extraction, Screenshot OCR, embeddings, and any future model use retain their own explicit per-scope/provider controls. File actions retain Preview, policy validation, durable transaction, confirmation, and Undo requirements.

### Exclusion model

- Hard exclusions are true access-policy denials, not UI result hiding. An excluded descendant cannot be persisted as a live location, read for content/OCR, embedded, linked into current graph/retrieval state, watched, returned by MCP, or targeted by a file action.
- Built-in non-overridable denials continue to cover protected system roots, symbolic-link/junction traversal, Trash/Recycle Bin enumeration, unsafe temporary files, and other accepted scanner policy. User exclusions are additional visible rules inside confirmed coverage.
- A pre-scan review lists every confirmed root, built-in exclusion category, and user exclusion. The UI must distinguish `excluded from DeskGraph` from a future non-security `hidden from results` preference; v0.1 does not implement result-only hiding under the word “exclude”.
- Adding or expanding an exclusion after indexing applies fail-closed immediately. Until an atomic privacy purge completes, the affected scope is unavailable to Search, MCP, Watch, extraction/OCR, Project/Cleanup discovery, and file-action planning. The purge removes or invalidates all index-derived paths, content, OCR, FTS, embeddings, current graph facts, caches, and pending automatic jobs for that subtree without changing any source file.
- Privacy withdrawal takes precedence over the ordinary immutability promise for local derived evidence. Project candidates, relation observations, screenshot groups, cleanup previews, legacy rename previews, and other path/content/identity-bearing derived history affected by the exclusion must be removed in the same privacy transaction; retaining them as immutable history is not an acceptable substitute for purge. Ordinary application code still cannot update or delete those rows.
- User-authored transaction journals are safety records, not ordinary index data. A scope/exclusion change must not silently remove a nonterminal action record or make crash recovery ambiguous. It blocks on or moves that record to an explicit `needs_attention` policy state under the action ADRs before privacy purge can commit. Retention and user-initiated clearing of terminal path-bearing execution receipts require a separate privacy/transaction decision before executable actions ship.
- Removing an exclusion grants no content access by itself. The user must run or confirm a new metadata reconciliation, after which optional content/OCR/embedding controls still apply independently.

### Policy revision and privacy purge

- Each coverage root has a durable monotonically increasing policy revision. Scan, Watch, extraction/OCR, Search/MCP result publication, Project/Cleanup detail, and file-action planning bind the revision they started with and fail closed if it changes before publication or return.
- Adding an exclusion and removing its local derived data are one `BEGIN IMMEDIATE` transaction: insert the canonical strict-descendant exclusion, increment the revision, invalidate old work, delete affected derived rows in foreign-key order, write a path-free purge receipt, and commit. A crash yields either the old policy with the old data or the new policy with the complete logical purge—never a visible half-state.
- Existing immutable-table triggers may gain only a transaction-scoped privacy-purge exception owned by the Rust database layer. The authorization is created and consumed inside the same SQLite transaction and cannot survive rollback or process exit. No general delete API, source-file delete, permanent-delete command, or LLM-accessible path is introduced.
- Logical purge must remove affected location and content FTS entries rather than merely setting `active = 0`. It also removes OCR/image metadata, embeddings, current graph and relation data, scan/extraction/watch queues or histories containing the excluded path, and orphaned nodes not required by an allowed location. Same-scope hard-linked identities are conservatively withheld from content/search/relations when any known location is excluded.
- Best-effort SQLite page reclamation, secure-delete configuration, WAL checkpointing, and maintenance vacuum may reduce local remnants after commit, but DeskGraph must not claim forensic erasure from SSDs, snapshots, backups, or filesystem history.

### Revocation and presentation

- Users can review effective coverage, add or remove roots, add or remove exclusions, and revoke a root from one settings surface. Every change is local, explicit, durable, and auditable without logging paths.
- Revoking a root immediately removes its runtime capability and disables all reads. Derived-data purge follows the same fail-closed rule as a newly added exclusion; the underlying files are never deleted or moved.
- Ordinary dashboard, logs, telemetry, MCP configuration, and path-free histories expose only stable IDs/counts/states. Paths appear only in the user's explicit local coverage/exclusion management view.

### Local root-revocation checkpoint — 2026-07-21

- The four-language Desktop now implements an exact two-stage root-revocation flow. Preview is bounded, short-lived, one-time and path-free outside the explicit local coverage-management view. It reports the complete current purge impact, including separate Rename and Cleanup Preview counts, plus the exclusions that will be removed. Confirmation accepts only the opaque Preview ID.
- Confirmation recomputes the policy revision, purge impact and exclusion count inside the same `BEGIN IMMEDIATE` transaction. Any mismatch returns `scope_root_revocation_preview_stale` before mutation and requires a fresh Preview. A successful transaction advances the policy revision, changes the active platform grant to `revoked`, replaces the reusable capability bytes with the exact `X'00'` tombstone, purges the complete root-derived SQLite state, removes that root's exclusions and writes an immutable path-free receipt. Migration 0026 backfills prior revoked rows and prevents later writes from restoring reusable bytes to a revoked grant.
- Every completed scan for the withdrawn revision is changed to a failed terminal state. Reauthorizing the same root therefore cannot revive old scan readiness: the user must run a fresh metadata scan before Watch, extraction, Search, Project/Cleanup or MCP can become eligible again.
- Every public Scanner filesystem-read entry point obtains the active grant and a shared scope fence. Public root-revocation and hard-exclusion APIs either obtain their own fence or require a matching scope-and-revision token. A two-second bounded gate-and-data protocol blocks new cooperative readers before the writer drains existing readers. The private files have immutable SQLite-bound identities; Unix opens use no-follow flags and validate directory/file identity before changing permissions, so replacement or reparse/symlink hazards fail closed rather than creating a split lock. The files live only in DeskGraph-managed app data, never inside a user scope and never contain a source path.
- After commit, Desktop drops the live runtime capability and requests native Watch removal. If that bounded acknowledgement times out, the committed response remains successful, retires callback admission, clears queued path hints, requests bounded coordinator stop and reports synchronization, callback-retirement and joined-stop booleans separately. The UI requires restart and never claims the OS registration closed unless join succeeded; it does not pretend the durable revocation rolled back. Source files and their bytes remain unchanged.
- This lock is cooperative/advisory coordination among DeskGraph processes. It is not an authorization boundary against an arbitrary or hostile same-user process, and it is not ADR-027's production filesystem-action process fence. The current evidence is local development evidence only; signed/notarized macOS, macOS Intel or Universal, packaged Windows, clean-machine, cross-platform race/stress and remote-CI acceptance remain open. Exclusion removal also remains unimplemented.

## Consequences

- Users can cover the common parts of a computer in one understandable onboarding decision instead of repeating the same picker workflow folder by folder.
- The security invariant changes in granularity, not in consent: DeskGraph may access only paths inside a user-confirmed coverage set and outside every active exclusion.
- A silent whole-computer opt-out default remains rejected. Local-first storage does not compensate for overbroad collection, OS/store incompatibility, or user surprise.
- Existing single-folder authorization remains a valid one-root coverage set and can migrate without weakening access.
- The local multi-root, add-only hard-exclusion and root-revocation slices do not make this model release-complete. M1/M6/M8 still require exclusion removal, signed and packaged platform acceptance, clean-machine and cross-platform race/stress evidence, and the remaining Watch/MCP/action-policy and release matrices.

## Rejected alternatives

- Silently index the home directory or every mounted disk, then offer exclusions afterward.
- Require Full Disk Access or `broadFileSystemAccess` for first-run success.
- Keep “one folder at a time” as the only primary onboarding path.
- Treat `.gitignore`, hidden-result filters, filename patterns, or model-classified sensitivity as authorization boundaries.
- Read content first in order to decide whether it is sensitive enough to exclude.
- Mark excluded rows hidden while retaining searchable content, OCR, embeddings, graph edges, MCP visibility, or automatic jobs.
