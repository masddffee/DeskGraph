# Initial Architecture Decision Records

## ADR-001 — SQLite instead of Neo4j

Decision：
v0.1 使用 SQLite + adjacency tables + FTS5 + vector adapter。

Why：

- 單一檔案。
- 無伺服器。
- 易備份。
- 適合桌面安裝。
- 8GB 裝置。
- 可透過 recursive CTE 與 Rust graph library 完成需要的 traversal。

Revisit：
當圖譜規模、查詢或多人同步明確超過 SQLite 能力時。

## ADR-002 — Local LLM is optional

Decision：
核心搜尋、圖譜與整理不得依賴 LLM。

Why：

- 安裝包大小。
- 記憶體。
- 硬體差異。
- 模型品質不穩。
- 可解釋性。

LLM only：
低信心分類、名稱建議、短解釋、候選關係消歧。

## ADR-003 — No delete

Decision：
v0.1 不實作永久刪除。

Why：
降低不可逆風險與信任門檻。

## ADR-004 — MCP read-only

Decision：
v0.1 MCP 不提供 move、rename、delete。

Why：
桌面 UI 才具備 Preview、Scope 與 Human Approval。

## ADR-005 — Graph as infrastructure, not visualization

Decision：
主 UI 以 Search、Projects、Inbox 與 History 為核心。

Why：
使用者價值是找到與整理，不是看一張複雜關係圖。

## ADR-006 — Model download after install

Decision：
大型模型不包在基礎 Installer。

Why：
縮小安裝包，讓無模型模式可立即使用。

## ADR-007 — Provider interfaces

Decision：
OCR、Embedding、LLM、Vector Index 都使用 abstraction。

Why：
避免綁死仍快速變動的模型與套件。

## ADR-008 — Apache-2.0 project license

Decision：
DeskGraph 原創專案程式碼以 Apache License 2.0 發布。第三方依賴、模型、圖示、字型與資料集保留各自經稽核的授權與 notices。

Why：

- 符合 M0 對明確 permissive open-source license 的要求。
- 除了商用、修改與散布權，也提供明確專利授權與專利終止條款。
- 對未來貢獻者提供一致的預設授權契約。

Revisit：
只有在有文件化的法律或生態系需求，且完成貢獻者影響評估後重新審議。

Canonical detail：
`docs/architecture/adr/0008-project-license.md`。

## ADR-010 — Manifest store and file identity

Decision：
M1 使用 bundled SQLite，並將 graph node identity 與 path location 分離。Unix 使用 device/inode；Windows 使用官方 `windows-sys` 的 volume serial/file index；所有路徑仍受 canonical authorized scope 約束。

Why：

- Path-only identity 無法安全處理 Move 與 Hard Link。
- Rust 1.97 的 Windows metadata accessor 仍不穩定。
- Initial Scan、Watch Reconcile 與 Transaction Engine 需要共同的持久化 identity source of truth。

Canonical detail：
`docs/architecture/adr/0010-manifest-store-and-file-identity.md`。

## ADR-011 — Resumable scan jobs publish atomically

Decision：
Scan path queue、observations 與 issues 先持久化到 job-scoped staging；pause、crash 與 resume 都不修改 live manifest。只有完整掃描結束後，才以單一 SQLite transaction 發布並 reconcile stale locations。

Why：

- Progress 與 pause/resume 必須跨 process exit 保留。
- Partial scan 不能暫時成為 graph source of truth。
- Runner lease 可區分失聯工作與正常的 concurrent status/pause connection。
- 同一 queue entry 重播必須 idempotent。

Canonical detail：
`docs/architecture/adr/0011-resumable-scan-jobs.md`。

## ADR-012 — Bounded extraction provider contract

Decision：
Extractor provider 不取得任意 filesystem path；Rust core 完成 scope、identity、stability 與 limits 驗證後，只交付受控的 bounded `Read + Seek` source。所有輸出標記為 untrusted，完整成功才原子替換 active chunks。

Why：

- 惡意文件不得擴大 provider 的檔案系統能力。
- Size、time、decompression、structure、output 與 cancellation limits 必須跨格式一致。
- Corrupt、cancelled 或超限檔案不能發布 partial retrieval truth，也不能清除上一版成功內容。
- 核心 text/Markdown/code extraction 不應等待 OCR、Python、模型或大型 parser dependency。

Canonical detail：
`docs/architecture/adr/0012-bounded-extraction-provider-contract.md`。

## ADR-013 — Bounded PDF text with structural provenance

Status：Accepted。PDF 僅由受控、限額、無 path capability 的 provider 讀取；page/fragment provenance 不偽造 byte offsets。

Canonical detail：
`docs/architecture/adr/0013-bounded-pdf-text-and-structural-provenance.md`。

## ADR-014 — Allowlisted OOXML parts

Status：Proposed，尚未接受。ZIP/XML dependency closure、license、RustSec、平台與 adversarial fixture 證據完成前不得實作或加入 dependency。

Canonical detail：
`docs/architecture/adr/0014-allowlisted-ooxml-parts-proposal.md`。

## ADR-015 — SQLite FTS5 trigram lexical baseline

Status：Accepted。以 bundled SQLite 的 FTS5 trigram 作為無模型、可說明且有界的 metadata/content lexical baseline；不宣稱已具備 vector 或 hybrid semantic search。

Canonical detail：
`docs/architecture/adr/0015-sqlite-fts5-lexical-baseline.md`。

## ADR-016 — Durable watch hints and atomic reconciliation

Status：Accepted。Watcher event 僅是 untrusted hint；scope、stability 與 open-handle identity 驗證通過後，才由既有 resumable scanner 原子 reconcile manifest。

Canonical detail：
`docs/architecture/adr/0016-durable-watch-hints-and-reconciliation.md`。

## ADR-017 — Immutable rename preview and first journal event

Status：Accepted。同資料夾檔案 Rename 僅能建立 immutable Preview 與 atomic append-only sequence-1 journal event；尚無 executor、recovery 或 Undo。

Canonical detail：
`docs/architecture/adr/0017-immutable-rename-preview-and-journal.md`。

## ADR-018 — Derived folder profiles and project suggestions

Status：Accepted。Folder Profile 從目前 `present` manifest locations 有界、唯讀地即時計算；規則式 Project 結果只能是含 confidence/provenance/observed time/creator/provider 的 suggestion，不建立自動 membership。

Canonical detail：
`docs/architecture/adr/0018-derived-folder-profiles-and-project-suggestions.md`。

## ADR-019 — Append-only Project candidates and user feedback

Status：Accepted。Project root 以 `(scope_id, root_folder_node_id)` 取得穩定候選 identity；suggestion observations/signals 不可變，使用者 accept/reject 以 append-only event 更正。Rejected 的同一 root 後續仍為 rejected，且 Accept 不會自動建立 file membership。

Canonical detail：
`docs/architecture/adr/0019-append-only-project-candidates-and-user-feedback.md`。
