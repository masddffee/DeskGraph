# Phase 10 — Read-only MCP Server

Implement milestone M7.

Expose only user-authorized context.

Tools:
- search_files
- get_file_context
- get_project_context
- list_related_files
- explain_relation
- list_recent_files
- preview_organization_plan

Requirements:
- read-only
- stdio first
- explicit scope enforcement
- minimal returned fields
- no arbitrary path parameter that bypasses identity resolution
- audit tool calls locally
- redact content when only metadata is required
- clear setup docs for Codex, ChatGPT and other MCP clients

Add prompt-injection tests where indexed files attempt to redefine tool behavior.

Acceptance:
- scope escape tests fail safely
- no write operations exist
- demo query works end-to-end
