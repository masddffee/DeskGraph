# Watcher Safety Instructions

- Treat every filesystem event as an untrusted hint, never as manifest source of truth.
- Validate the current canonical authorized scope before persisting or reading an observed path.
- Deny symlink, junction, reparse-point, hidden, protected, and out-of-scope traversal consistently with the scanner.
- Coalesce event storms durably and require a bounded stability window before reconciliation.
- A stable hint may schedule only the existing atomic manifest reconciliation path; it must not directly mutate live graph rows.
- Never perform file moves, renames, deletes, writes, network access, shell execution, or content extraction in a watcher adapter.
- Do not log observed paths, filenames, snapshots, or file content. User-requested CLI input may be used only for the explicit operation.
- Persist enough state to resume after restart, and keep failures in fixed-code states.
- Do not add a native watcher dependency until its official API, maintenance, platform support, license, and security closure are recorded.
