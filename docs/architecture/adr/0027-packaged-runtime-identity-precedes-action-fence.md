# ADR-027: Packaged Runtime Identity Precedes the Action Process Fence

- Status: Accepted
- Date: 2026-07-19

## Context

ADR-025 uses an expiring SQLite lease to coordinate durable action work. A lease cannot fence a
live executor that is paused longer than the lease: recovery could observe expiry and start while
the original process still owns operating-system state. ADR-026 therefore requires a separate,
crash-released process fence before any production Execute, automatic recovery, or Undo entry.

The current repository does not yet provide the package identity needed to place that fence in a
trusted namespace. `apps/desktop/src-tauri/tauri.conf.json` has bundling disabled, declares no
macOS App Sandbox entitlements, and has no Windows release-package manifest. Tauri's generic
application-data directory API provides a path, but it does not prove container ownership,
package identity, access control, no-follow opening, or installer repair behavior. Adding an
ordinary lock file there would only make unsafe code look complete.

The v0.1 topology also does not need a writer daemon or helper. The Tauri Rust core is the sole
future action host. CLI and MCP remain unable to execute actions, and the LLM boundary remains
suggestion-only.

## Evidence

- Apple's current container-protection documentation says that system-created app-data containers
  for apps with the App Sandbox capability offer System Integrity Protection; it separately dates
  SIP-protected App Group containers to macOS 15 and later. That page alone does not select or
  prove DeskGraph's per-app-container OS floor. An app outside a protected container or App Group
  may still ask the user to authorize access, so the container is not an unconditional boundary
  against root, explicit user consent, or an already-authorized process. User-selected locations
  outside that container require an explicit selection flow and security-scoped access that can
  be restored with a bookmark.
- Apple `flock` is an advisory cooperative lock that remains held while a process is stopped and
  is released when the last referring descriptor closes after exit or crash. Descriptor lifetime
  and close-on-exec handling are part of the safety contract.
- A shared macOS App Group is useful only when multiple separately entitled executables require
  the same container. It expands the sharing boundary and is unnecessary for the accepted v0.1
  single-host topology.
- Windows mutex ownership is thread-bound. A stopped owner keeps the mutex; termination produces
  `WAIT_ABANDONED`, which means the protected state may be inconsistent and requires recovery
  before new work.
- Windows private Object Manager namespaces can use a boundary descriptor and explicit security
  descriptor. Package family identity is stable across package updates and already gates the
  native Windows OCR design in ADR-024.
- Tauri's single-instance plugin is a UI lifecycle convenience. Its public contract does not
  prove the private namespace, access control, crash handoff, inheritance, replacement, or
  action-journal ordering required here.

Primary references:

- [Apple: Protecting local app data using containers on macOS](https://developer.apple.com/documentation/xcode/protecting-local-app-data-using-containers)
- [Apple: Accessing files from macOS App Sandbox](https://developer.apple.com/documentation/security/accessing-files-from-the-macos-app-sandbox)
- [Apple `flock(2)`](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/flock.2.html)
- [Microsoft: Mutex objects](https://learn.microsoft.com/en-us/windows/win32/sync/mutex-objects)
- [Microsoft: Private namespaces](https://learn.microsoft.com/en-us/windows/win32/sync/object-namespaces)
- [Microsoft `CreatePrivateNamespaceW`](https://learn.microsoft.com/en-us/windows/win32/api/namespaceapi/nf-namespaceapi-createprivatenamespacew)
- [Microsoft `CreateMutexExW`](https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-createmutexexw)
- [Microsoft `WaitForSingleObject`](https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-waitforsingleobject)
- [Microsoft: Packaged desktop apps](https://learn.microsoft.com/en-us/windows/msix/desktop/desktop-to-uwp-behind-the-scenes)
- [Microsoft `GetCurrentPackageFamilyName`](https://learn.microsoft.com/en-us/windows/win32/api/appmodel/nf-appmodel-getcurrentpackagefamilyname)

## Decision

### Threat model and acquisition order

- The fence serializes cooperating, trusted DeskGraph action and recovery processes. It prevents
  lease expiry from admitting recovery while a stopped live executor still owns the fence.
- It does not protect an arbitrary authorized source leaf from editors, sync clients, malware, or
  other same-user writers; it does not defend against root or administrator control; and it never
  substitutes for an exact-source or action-bound platform primitive.
- On macOS it also does not claim protection after the user grants another process access to the
  app-data container. If the supported macOS release/package configuration cannot prove that a
  non-entitled same-user process is prevented from silently replacing the fence entry, the macOS
  fence is unavailable rather than downgraded to a cooperative path lock.
- Execute, automatic action recovery, and Undo must acquire the platform fence before opening the
  action database. A busy or unavailable fence fails before database, journal, or filesystem side
  effects. The guard remains held through the durable terminal observation and database close.
- An abandoned Windows mutex enters recovery-only handling. It cannot admit an unrelated new
  action until journal recovery reaches a safe recorded outcome.

### v0.1 process topology

- The Tauri Rust core is the only action host. v0.1 adds no writer daemon, localhost service,
  socket, named pipe, XPC service, privileged helper, or separate action helper.
- CLI, read-only MCP, Watch callbacks, Inbox rules, LLM providers, and frontend code cannot
  acquire the fence or execute an action.
- Windows fence acquisition, journal work, and release run on one dedicated native thread because
  mutex ownership is thread-bound. Handles and descriptors are non-inheritable and close-on-exec.

### macOS candidate contract

- No macOS production fence is accepted yet. The candidate requires a signed App Sandbox build,
  an OS-provided per-app container protected against unauthorized modification, a selected
  supported macOS floor, and signed clean-machine evidence that a non-entitled same-user probe
  cannot silently open, unlink, rename, or recreate the fence entry. A user-authorized exception,
  root, or an entitled group member is outside this protection and must be documented honestly.
- If any of those conditions cannot be proven, macOS keeps search, graph, suggestions, and Preview
  but cannot acquire a production action fence. An unsigned/unsandboxed fallback is forbidden.
- Explicit user authorization must use a native selection flow and persist only the minimum
  security-scoped bookmark needed to restore that scope. Manual path text is not treated as an
  App Sandbox capability grant.
- Only after the container protection gate passes may the fixed candidate fence file live under
  that validated container. It must be opened component-safely without following links as an
  owned regular file with private permissions, retain verified root/file identity, and use
  nonblocking exclusive `flock` with close-on-exec. Container identity checks do not by
  themselves repair a replacement attack; the OS protection and hostile-probe evidence are
  mandatory.
- No App Group is added for v0.1. A future helper or second app must justify its topology,
  entitlements, update skew, IPC authentication, and shared-container boundary in a new ADR.

### Windows contract

- Production action support requires a verifiable packaged identity. The package family identity
  used for the fence must be the same release identity used to validate native Windows OCR.
- The fence uses an Object Manager private namespace with a boundary descriptor and explicit
  protected DACL, then a named mutex created inside that namespace. A predictable bare
  `Global\\`/`Local\\` mutex or an AppData lock file is not accepted.
- The mutex handle is non-inheritable; process creation while held must not inherit or duplicate
  it. `WAIT_TIMEOUT` is a path-free busy result and `WAIT_ABANDONED` is recovery-only.
- The initial v0.1 Windows desktop package may run as packaged classic desktop medium-integrity
  code. This ADR does not claim an AppContainer security boundary.

### Linux boundary

- Linux remains an experimental build and does not gain a production action fence, Rename/Move
  executor, or system-trash executor from this ADR. Its missing action runtime cannot delay the
  required macOS and Windows release artifacts and must be disclosed as a limitation.

### Implementation gate

- No production fence code or dependency is added until the platform packaging identity and
  container/scope handoff exist and can be exercised on the target OS.
- Existing production action entry points continue to return
  `action_platform_rename_unsupported` before any side effect.
- The first implementation work is the packaged-runtime identity foundation. Only then may the
  platform fence adapters and real child-process acceptance matrices be added.

## Required acceptance evidence

Each supported production action platform must prove, from the signed/packaged application:

1. two-process contention and fail-before-database ordering;
2. a paused owner held beyond the SQLite lease while recovery remains excluded;
3. crash or forced termination release and recovery-only abandoned-state handling;
4. fork/exec, process-spawn, descriptor/handle inheritance, and duplicate-handle resistance;
5. namespace/file replacement, link/reparse, ownership, ACL, mode, and identity rejection; a
   platform that cannot prevent a second process from acquiring a replacement object is not
   supported for production actions;
6. installer creation, update continuity, repair, uninstall, and reinstall behavior;
7. journal terminal durability, database reopen, and no interference with SQLite WAL locking; and
8. path-free status, error, and structured-log output.

macOS must additionally prove bookmark restore/revocation, selected-scope access from the sandbox,
the supported-version SIP/container guarantee, and a non-entitled same-user replacement probe
without a user-authorized exception. Windows must additionally prove private-namespace
boundary/DACL behavior and stable package family identity on supported Windows x64 installs.

## Consequences

- D-019 is resolved as an identity-first architecture decision, but no production action fence is
  implemented or runtime-accepted. The Windows primitive contract is selected; the macOS `flock`
  design remains a gated candidate until the protected-container replacement proof and minimum OS
  decision pass. M5 remains in progress and all production Execute/recovery/Undo controls stay
  unavailable.
- A narrow packaged-runtime identity foundation moves ahead of M5 in the dependency graph. Final
  installers, updater, signing, SBOM, checksums, and publishing remain M9 release gates.
- Windows package identity work is shared with the existing native OCR prerequisite instead of
  building two unrelated identity mechanisms.
- macOS authorization must migrate from a development path-entry flow to a native sandbox-aware
  scope flow before release. Existing local data needs an explicit, fail-closed migration design.
- The design avoids a new helper/daemon IPC attack surface and avoids shipping unused abstraction
  code before the operating-system contract can be tested.

## Rejected alternatives

- A lock file beside SQLite or in a caller-supplied/generic AppData path.
- SQLite leases, transactions, or WAL locks as a process-liveness fence.
- Tauri's single-instance plugin as an action-safety boundary.
- A macOS App Group without a justified second executable.
- An unsandboxed macOS lock root selected by convention.
- A predictable bare Windows mutex name without a private namespace and explicit security.
- An inheritable descriptor/handle or helper that implicitly shares ownership.
- Enabling the test-only Unix adapter after the fence is present; ADR-026 remains independent.

## Revisit trigger

Revisit only if the product adopts a separately installed writer/helper, Apple changes the
container or bookmark contract, Windows packaging cannot supply a stable private-namespace
identity, or a supported OS introduces a stronger crash-released primitive that materially
reduces this design's attack surface. Any change requires a new threat model and the complete
child-process/runtime matrix above.
