# System Architecture

## 1. 架構原則

- Rust 負責所有檔案、索引、圖譜與交易核心。
- Tauri 2 + React/TypeScript 僅負責桌面 UI。
- 所有模型與 OCR 都透過 Provider Interface，可替換且可關閉。
- 單機資料預設儲存在單一應用資料目錄。
- SQLite 是 source of truth；Graph 不依賴獨立伺服器。
- 每個外部模型、Binary 與權重都必須有版本、License、SHA-256。
- 系統必須支援 Crash Recovery 與冪等重試。

## 2. Repository Structure

```text
.
├── AGENTS.md
├── Cargo.toml
├── package.json
├── pnpm-workspace.yaml
├── apps/
│   ├── desktop/
│   │   ├── src/
│   │   └── src-tauri/
│   └── cli/
├── crates/
│   ├── domain/
│   ├── config/
│   ├── database/
│   ├── scanner/
│   ├── watcher/
│   ├── identity/
│   ├── extractors/
│   ├── ocr/
│   ├── embeddings/
│   ├── graph/
│   ├── retrieval/
│   ├── project-discovery/
│   ├── classifier/
│   ├── planner/
│   ├── executor/
│   ├── transactions/
│   ├── inference/
│   ├── mcp-server/
│   ├── security/
│   └── telemetry/
├── packages/
│   ├── ui/
│   ├── shared-types/
│   └── rule-schema/
├── models/
│   ├── manifests/
│   └── licenses/
├── migrations/
├── fixtures/
├── benchmarks/
├── docs/
│   ├── architecture/
│   ├── security/
│   ├── product/
│   └── planning/
└── .github/
    ├── workflows/
    ├── ISSUE_TEMPLATE/
    └── release.yml
```

## 3. Runtime Components

```text
Desktop UI
   │
   ├── IPC Commands
   │
Core Daemon
   ├── Scope Policy
   ├── Scanner
   ├── Watcher
   ├── Extraction Queue
   ├── Graph Builder
   ├── Retrieval Engine
   ├── Action Planner
   ├── Transaction Executor
   └── MCP Server
          │
SQLite + FTS5 + Vector Adapter
          │
Optional Providers
   ├── OCR Provider
   ├── Embedding Provider
   └── Local LLM Provider
```

## 4. Ingestion Pipeline

```text
Observed Path
  → Scope Validation
  → Symlink / Permission / Placeholder Check
  → File Stability Check
  → Identity Resolution
  → Metadata Extraction
  → MIME Detection
  → Content Extraction
  → OCR when needed
  → Chunking
  → Embedding
  → Entity / Topic Candidate Extraction
  → Edge Scoring
  → Project Assignment
  → Search Index Update
  → Action Suggestions
```

### Stability Check

新下載檔案不得在仍被寫入時處理。至少確認：

- 檔案大小在兩次檢查間不變。
- 修改時間穩定。
- 不屬於 `.part`、`.crdownload`、`.download` 等暫存格式。
- 可正常開啟為唯讀。
- Cloud Placeholder 已實際下載，或被標記為 unavailable。

## 5. Graph Model

### Node Types

- File
- FileVersion
- Folder
- Project
- Topic
- Person
- Organization
- Location
- Event
- Application
- Source
- Tag
- ActionPlan
- Transaction

### Edge Types

- located_in
- belongs_to
- mentions
- similar_to
- duplicate_of
- near_duplicate_of
- version_of
- created_by
- opened_with
- downloaded_from
- co_used_with
- active_during
- references
- supports
- proposed_move_to
- proposed_rename_to

### Edge Required Fields

```json
{
  "id": "edge_ulid",
  "source_node_id": "node_a",
  "relation": "belongs_to",
  "target_node_id": "node_b",
  "confidence": 0.91,
  "provenance": [
    {
      "kind": "ocr",
      "reference": "chunk_123",
      "weight": 0.30
    },
    {
      "kind": "embedding",
      "reference": "folder_centroid_9",
      "weight": 0.45
    }
  ],
  "observed_at": "RFC3339",
  "valid_from": "RFC3339",
  "valid_until": null,
  "created_by": "rule|model|user|system",
  "model_version": null
}
```

## 6. Database Tables

- schema_migrations
- files
- file_identities
- file_locations
- file_versions
- content_chunks
- extracted_entities
- nodes
- edges
- projects
- project_memberships
- folders
- folder_profiles
- embeddings
- rules
- scan_jobs
- job_items
- action_plans
- action_items
- transactions
- transaction_items
- user_feedback
- model_registry
- audit_events
- settings

## 7. File Identity

路徑不可作為唯一 ID。

優先組合：

- Volume identifier。
- Platform file identifier / inode。
- Canonical path。
- Size。
- Fast hash。
- Full content hash（必要時背景計算）。

處理：

- Case-sensitive 與 case-insensitive volume。
- Unicode NFC / NFD。
- Hard link。
- Symlink。
- Junction。
- Network volume。
- External drive。
- Cloud sync placeholder。

## 8. Retrieval

採 Hybrid Retrieval：

```text
FTS5 lexical score
+ vector similarity
+ graph proximity
+ recency
+ project affinity
+ user interaction prior
```

所有分數需要可診斷，不得只有最終黑箱分數。

## 9. Local AI

### Embedding

- Provider trait。
- ONNX / native runtime。
- 多語言小型模型。
- 模型可下載、可移除。
- 支援 int8。
- 批次處理與記憶體上限。
- Model Manifest 鎖定來源、版本、維度、License 與 checksum。

### OCR

Provider 順序：

1. 平台原生 OCR，可用時優先。
2. 跨平台輕量 OCR Provider。
3. 選配 PaddleOCR 高品質模型。

禁止將 Python 環境需求暴露給最終使用者；若使用 Sidecar，必須被 Installer 完整包裝。

### Local LLM

- 非必要依賴。
- llama.cpp sidecar 或 library adapter。
- 建議 1B–2B 級、4-bit GGUF。
- 只處理低信心案例與短摘要。
- 非思考模式優先。
- JSON Grammar 約束輸出。
- App 必須在未下載 LLM 時正常運作。

## 10. MCP

v0.1 預設唯讀。

Tools：

- search_files
- get_file_context
- get_project_context
- list_related_files
- explain_relation
- list_recent_files
- preview_organization_plan

Resources：

- project://{id}
- file-context://{id}
- recent://files
- graph://stats

寫入工具延後至 v0.2，且必須要求桌面 UI 顯示一次性核准 Token。

## 11. Memory Budgets

目標：

- Idle without models：< 350 MB。
- Metadata scan peak：< 1 GB。
- OCR / Embedding 單一 Provider 工作時：< 2.5 GB。
- Optional LLM peak：整體 < 5 GB。
- 8GB 裝置仍保留足夠 OS 空間。

策略：

- Provider 不同時常駐。
- bounded queues。
- batch size 自適應。
- 電池模式降速。
- thermal / load aware。
- 暫停與恢復。
- Low Memory Mode。
