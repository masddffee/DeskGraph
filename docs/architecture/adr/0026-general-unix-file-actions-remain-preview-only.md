# ADR-026: General Unix Rename and Move Remain Preview-Only

- Status: Accepted
- Date: 2026-07-19

## Context

ADR-025 left D-018 open because its macOS and Linux prototypes select the source with a
held parent-directory descriptor plus a leaf pathname. DeskGraph also holds and revalidates
the expected source file, but another process can replace that leaf after the last userspace
identity check and before the rename syscall begins. A successful no-overwrite rename can
therefore move a different file.

This is not only a hostile-process scenario. Any editor, sync client, download finalizer, or
other local process with write access to the authorized parent can change its namespace in
the same interval. Post-action identity verification detects the wrong mutation only after it
has happened and cannot make that mutation safe.

## Evidence

- Apple `renameatx_np(int fromfd, const char *from, int tofd, const char *to, ...)`
  accepts directory descriptors plus source and destination path strings. `RENAME_EXCL`
  protects an occupied destination and `RENAME_NOFOLLOW_ANY` rejects symlink traversal; neither
  conditions the operation on a previously opened source file. The current XNU implementation
  resolves `from` with `nameiat` before calling the filesystem rename operation.
- Linux `renameat2(int olddirfd, const char *oldpath, int newdirfd, const char *newpath, ...)`
  has the same source-name boundary. `RENAME_NOREPLACE` protects the destination but does not
  accept an exact source file descriptor.
- Linux `linkat(..., AT_EMPTY_PATH)` and macOS `fclonefileat` can create a new name or clone from
  an exact source descriptor, but neither removes the original name. A later pathname unlink or
  rename reintroduces the same race and is not an atomic Rename or Move.
- `flock` and open-file-description locks can serialize cooperating DeskGraph processes and
  are released when the last referencing descriptor closes. They are advisory, protect a lock
  object rather than the authorized source name, and do not constrain non-cooperating writers.
  A replaceable lock pathname can also be rebound to a second inode.
- `NSFileCoordinator` coordinates registered presenters around URLs. It is not an exact-FD
  rename primitive or a mandatory security boundary for arbitrary POSIX actors.
- A deterministic macOS/Linux test now performs a successful final identity check, replaces
  the ordinary source leaf, then invokes the native no-overwrite pathname syscall. The
  replacement file reaches the destination while the expected inode remains elsewhere. This
  counterexample is development evidence for rejection, not an accepted adapter test.

Primary references:

- [Apple XNU rename manual](https://raw.githubusercontent.com/apple-oss-distributions/xnu/main/bsd/man/man2/rename.2)
- [Apple XNU rename implementation](https://raw.githubusercontent.com/apple-oss-distributions/xnu/main/bsd/vfs/vfs_syscalls.c)
- [Apple XNU public rename declaration](https://raw.githubusercontent.com/apple-oss-distributions/xnu/main/bsd/sys/stdio.h)
- [Apple `NSFileCoordinator`](https://developer.apple.com/documentation/foundation/nsfilecoordinator)
- [Linux `rename(2)`](https://man7.org/linux/man-pages/man2/rename.2.html)
- [Linux `linkat(2)`](https://man7.org/linux/man-pages/man2/link.2.html)
- [Linux advisory and OFD locks](https://man7.org/linux/man-pages/man2/fcntl_locking.2.html)
- [POSIX `rename`/`renameat`](https://pubs.opengroup.org/onlinepubs/9799919799/functions/rename.html)

## Decision

### v0.1 product boundary

- General macOS and Linux Rename and Move for user-authorized folders remain Preview-only.
  CLI and Desktop may create, inspect, and list immutable plans, but expose no Execute,
  automatic recovery, or Undo control for those operations.
- Every production Unix Rename/Move entry returns
  `action_platform_rename_unsupported` before opening the database, acquiring a fence,
  journaling a command, or changing the filesystem. The deterministic Unix adapter remains
  `cfg(test)` only and must not be enabled by a feature flag, environment variable, or build
  profile.
- This resolves D-018 by rejecting a general Unix production adapter under the accepted safety
  invariants. It does not claim that the platform race was fixed.

### Process-fence boundary

- A crash-released OS fence solves only cooperation among DeskGraph executor and recovery
  processes. It cannot prove that the authorized source leaf still names the held inode.
- Any future action executor must separately prove an exact-source or action-bound platform
  primitive and a process fence. The fence must live in a package-private or otherwise
  replacement-resistant namespace, be opened without following links, retain stable identity,
  survive `SIGSTOP` without expiry, release on crash, and pass fork/exec/descriptor-leak plus
  unlink/rename/recreate child-process tests.
- No process-fence implementation is accepted by this ADR. D-019 owns that packaged runtime
  decision. SQLite leases remain bounded operational coordination and never become liveness
  fences.

### Independent platform work

- Windows may pursue a separately reviewed handle-bound
  `SetFileInformationByHandle(FileRenameInfo)` adapter because its source is an already-open
  handle. It still requires D-019, no-overwrite behavior, reparse-safe parent/source handles,
  durability/recovery evidence, and real Windows runtime tests.
- System Trash remains governed by D-017. This ADR does not approve pathname-based Trash or let
  Trash inherit the rejected Rename prototype. Its platform adapter must independently prove
  the exact plan-bound source, opaque receipt, recovery, and restore behavior.

## Reconsideration criteria

A future ADR may supersede this decision only when at least one of the following is true:

1. the target OS exposes a supported primitive that atomically conditions namespace mutation
   on the exact already-open source object and rejects destination overwrite; or
2. the operation is restricted to a genuinely managed namespace in which non-DeskGraph writers
   cannot change the source parent during the complete mutation, with that restriction enforced
   and verified by the OS rather than by an advisory lock file.

The new design must pass the source-leaf counterexample, real child-process pause/kill tests,
filesystem/volume capability checks, permission/durability failure injection, and the complete
ADR-025 recovery matrix before any product control appears.

## Consequences

- macOS and Linux users keep useful, explainable Rename/Move previews without a hidden unsafe
  execution path.
- v0.1 does not promise general Unix Rename/Move execution. Release copy and demos must not imply
  otherwise.
- The internal journal, immutable bindings, history states, and recovery reducer remain useful
  foundations for platform actions that later prove both required boundaries.
- M5 remains in progress for Windows handle execution, D-019, D-017 System Trash, Move planning,
  user-facing recovery/Undo, and their platform fault matrices.

## Rejected alternatives

- Accept the final check-to-syscall window as too small to matter.
- Treat destination no-overwrite, no-follow, hashing, or post-verification as exact-source proof.
- Use `linkat(AT_EMPTY_PATH)` or `fclonefileat` followed by a pathname removal.
- Treat `flock`, OFD locks, SQLite leases, file coordination, Watch, or FSEvents as a mandatory
  lock on arbitrary authorized folders.
- Move the wrong file and attempt an automatic rollback after detecting it.
- Advertise a test-only syscall prototype as an experimental production adapter.
