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
Core Runtime
   ├── Scope Policy
   ├── Scanner
   ├── Watcher
   ├── Extraction Queue
   ├── Graph Builder
   ├── Retrieval Engine
   ├── Action Planner
   ├── Transaction Executor
   └── Identity-based Read Services
          │
SQLite + FTS5 + Vector Adapter
          │
Optional Providers
   ├── OCR Provider
   ├── Embedding Provider
   └── Local LLM Provider

Independent read-only MCP stdio
   └── Identity-based Read Services + read-only SQLite
```

`Core Runtime` 是邏輯責任邊界，不代表 v0.1 已採用獨立 OS daemon。v0.1 預設依 [Tauri process model](https://v2.tauri.app/concept/process-model/) 由 core process 承載 Rust 核心，並可透過官方 [tray](https://v2.tauri.app/learn/system-tray/)／[autostart](https://v2.tauri.app/plugin/autostart/) 能力支援 UI 關閉後的背景工作；CLI 仍直接呼叫相同 domain crates。不得為了架構形式引入 localhost HTTP。

唯讀 MCP 是可由外部 Agent 啟動的獨立 stdio process，直接承載相同 identity-based read services 並以唯讀 SQLite connection 工作，不依賴 daemon IPC，也不取得 background writer lease。Tauri、CLI、Watcher 或未來 daemon 的任何持久寫入 runner 必須透過 durable lease 取得唯一 ownership；程序啟動本身不代表有 writer 權限。

若 Watch Mode 的實測需求證明 Tauri process 無法滿足 UI 關閉、唯一 writer、資源隔離或可靠重啟，才可依 D-014 評估 per-user daemon。拆分前必須同時定義 authenticated local IPC、Unix socket／Windows named-pipe ACL、peer identity、protocol version、app/daemon update skew、啟動競態、clean restart、installer 與 uninstaller acceptance；不能只新增一個常駐程序而把生命週期問題留給使用者。

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
  "score": {
    "kind": "evidence_score",
    "basis_points": 9100,
    "calibration_manifest": null
  },
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

`content_chunks`、內容 hash、provider/model manifest 與 schema version 是語意資料的可追溯真相。SQLite embedding rows 是可重算、可版本化的衍生 cache；版本相符的 embedding rows 可重建 exact／ANN index，而 content hash 加完整 model manifest 可在模型仍可用時重算 embedding rows。exact 或 ANN index 都只是可刪除重建的加速 artifact，不得成為第二份 source of truth。M3 先以有上限的 exact search 建立正確性與 8 GB baseline，再依真實 chunk/vector 數量、recall@k、結果一致性、p95、建索引時間、RSS 與更新吞吐選擇是否需要 ANN；model-version mismatch 必須原子失效，不使用任意檔案數門檻。

## 9. Local AI

### Intelligence Ladder

每個 domain 依序使用 deterministic facts／rules、專用 OCR／embedding／小型 ML provider、最後才是選配 Local LLM。OCR routing、retrieval fusion 與 Project scoring 留在各自 domain；共用層只負責 provider lifecycle、model manifest、job control 與資源 budget，避免形成可直接操作所有能力的中央「Intelligence Router」。

規則 evidence score、provider confidence、經校準的 probability 與 LLM 自報信心在語意上必須分離。現有 bounded candidate 的 `confidence_basis_points` 視為 legacy evidence score，不是已校準機率；在加入異質推論前，持久化與 API schema 必須帶 `score_kind`，機率還必須帶 calibration manifest。未經版本化 corpus 與 calibration 驗證的值不得宣稱為機率，也不得跨 provider 直接比較或作為自動檔案操作門檻。所有推論仍只是 schema-validated suggestion。

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
2. 經同一 corpus、package、RSS、cancellation 與主要釋出平台 gate 選出的 packaged fallback；Linux experimental 的限制另行記錄，不得拖延 macOS／Windows。
3. 其他選配品質 Provider，只有在同樣 acceptance 下證明淨收益才採用。

禁止將 Python 環境需求暴露給最終使用者。v0.1 不因候選模型已有上游 benchmark 就預設 Tesseract、PaddleOCR、ONNX Runtime 或 Sidecar；若使用 Sidecar，必須被 Installer 完整包裝並通過權限、生命週期、checksum、SBOM 與 clean-machine 驗證。

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
