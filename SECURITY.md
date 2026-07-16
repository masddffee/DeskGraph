# Security Policy

DeskGraph handles private local files, so data-loss, scope-escape, unintended network transfer, unsafe update, and MCP authorization reports are treated as security-sensitive.

## Supported versions

No public production version is supported yet. Security fixes apply to the current default branch until the first release support table is published.

## Reporting a vulnerability

When a public GitHub repository exists, use its private GitHub Security Advisory flow. Do not open a public issue for an unpatched vulnerability and do not include private user files, paths, OCR, embeddings, credentials, or graph data in a report.

Include only the minimum reproducible information:

- affected commit or version;
- operating system and architecture;
- the violated security boundary;
- synthetic reproduction steps;
- expected and observed behavior;
- whether data loss, scope escape, code execution, update compromise, or MCP exposure is possible.

Until the private advisory channel exists, retain the report locally and ask the repository owner to establish the private channel. No placeholder email address is published.

## Security invariants

- No permanent deletion operation.
- No direct LLM-to-filesystem execution path.
- Explicit scope and canonical-path validation.
- Durable and undoable move/rename transactions.
- Extracted text is untrusted and never executed.
- No default upload of file data or derived context.
