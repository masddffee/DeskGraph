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

Status：Accepted。只批准 `zip =8.6.0` 的 no-default `deflate-flate2-zlib-rs` 讀取路徑與 `quick-xml =0.41.0` no-default plain streaming reader；只讀 allowlisted in-memory OOXML parts，不跟隨 relationships、不解密、不寫出 archive entries。Provider adversarial fixtures、完整 lock audit、遠端 runtime 與 8 GB 證據仍是完成功能的必要 gate。

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

## ADR-020 — Bounded exact duplicate candidates

Status：Accepted。第一個 file relation 僅比較同一明確授權 scope 中兩個不同 stable identities 的非空檔案；在 64 MiB 與協作式時間上限內完成 full byte equality、前後 open-handle identity/metadata 驗證，才 append immutable `exact_duplicate` observation。結果永遠是 suggestion，沒有 merge、delete 或檔案操作。

Canonical detail：
`docs/architecture/adr/0020-bounded-exact-duplicate-candidates.md`。

## ADR-021 — Append-only file relation feedback

Status：Accepted。Exact-duplicate accept/reject 必須先重新完成 live byte verification，再 append immutable user feedback；相同決策 idempotent、相反決策保留更正歷史，且不授權任何檔案操作。

Canonical detail：
`docs/architecture/adr/0021-append-only-file-relation-feedback.md`。

## ADR-022 — Explicit numeric file version candidates

Status：Accepted。第一個 `version` relation 只接受同 scope、同 normalized base/extension 且具有 allowlisted `vN` suffix 的目前檔案；較小數字為 older、較大數字為 newer，所有方向證據 append-only 且不讀內容。

Canonical detail：
`docs/architecture/adr/0022-explicit-numeric-file-version-candidates.md`。

## ADR-023 — Evidence-bound directional version feedback

Status：Accepted。Version accept/reject 必須先 live reverify 並綁定 immutable directional evidence；只有等價的 ordered nodes、base/extension、version numbers 與 provider evidence 才能沿用決策，任何新方向證據回到 `suggested`。

Canonical detail：
`docs/architecture/adr/0023-version-feedback-is-bound-to-directional-evidence.md`。

## ADR-024 — Native-first OCR provider stack

Status：Accepted。macOS 先使用 Apple Vision；Windows 只在 package identity 與實際 Traditional Chinese／English recognizer 驗證通過時使用 `Windows.Media.Ocr`。Packaged fallback 必須經 D-015 同 corpus bake-off、license、checksum、memory、cancellation、packaging 與跨平台證據後另行選擇，不能把候選 runtime 當作已採用能力。

Canonical detail：
`docs/architecture/adr/0024-native-first-ocr-provider-stack.md`。

## ADR-025 — Journaled Rename protocol and fail-closed execution gate

Status：Accepted。批准 immutable SHA-256/root/parent/source execution binding、closed append-only command/recovery state、immutable request receipt 與 lease 作為 M5 內部協定基礎；不批准 production executor 或任何 process-fence 實作。macOS/Linux pathname prototype 僅能在測試使用，因為無法原子綁定 exact source inode。一般 Unix adapter 的最終判定由 ADR-026 補充。

Canonical detail：
`docs/architecture/adr/0025-journaled-direct-rename-execution-and-undo.md`。

## ADR-026 — General Unix Rename and Move remain Preview-only

Status：Accepted。官方 Apple／Linux／POSIX API 與 deterministic counterexample 證明 `renameatx_np`／`renameat2` 即使使用 directory descriptor、no-follow、no-overwrite 與 final identity check，仍不能把 syscall 原子綁定已持有的 source inode。D-018 因此以「一般 macOS/Linux 使用者 scope 的 Rename/Move 在 v0.1 保持 Preview-only」解決，而非降低安全門檻。Process fence 只序列化合作的 DeskGraph processes，不能修補 source race；D-019 另行決定 packaged-private fence。Windows handle adapter 與 D-017 System Trash 保持獨立審查。

Canonical detail：
`docs/architecture/adr/0026-general-unix-file-actions-remain-preview-only.md`。

## ADR-027 — Packaged runtime identity precedes the action process fence

Status：Accepted。D-019 以平台封裝身分先行的架構解決：v0.1 不新增 writer daemon/helper；Windows 以 package family identity 建立 protected private namespace 與 named mutex。macOS `flock` 只保留為 gated candidate，必須先選定支援的 OS floor，並由 signed App Sandbox／SIP-protected per-app container 的官方合約與真機 hostile probe 證明非 entitled 同 UID process 不能靜默替換 fence entry；證明不了就維持 unavailable。Fence 必須在 action database 開啟前取得，paused live owner 不因 SQLite lease expiry 失去排他性，crash/abandoned state 只能進 recovery。現有 repository 尚無這些封裝條件，所以不加入通用 AppData lock 或假完成的 runtime abstraction，所有 production Execute／Recovery／Undo 仍 fail closed。

Canonical detail：
`docs/architecture/adr/0027-packaged-runtime-identity-precedes-action-fence.md`。

## ADR-028 — Explainable screenshot review groups are suggest-only

Status：Accepted。截圖審查群組使用獨立的 append-only 多成員候選與完整 immutable evidence snapshot，不擴張既有二元 duplicate/version relation。v1 要求 completed scan 與 active platform grant，只使用同 scope 的 current M2 image metadata、completed explicit screenshot-OCR provenance、相同尺寸與十分鐘時間窗；source selection、grouping、revalidation 與 immutable write 共用一個 immediate transaction，等價 evidence 重跑不增長。它不讀 OCR text、image bytes、filename、embedding 或 model output。群組只協助逐項審查，不證明 screenshot origin、相同內容、keeper、可丟棄或可回收空間；任何 grant／成員／metadata／OCR provenance 改變都 fail closed，inactive history 只保留 path-free `current_evidence: false`。此決策不批准 Trash、ActionPlan、Execute 或 Undo；D-017 與 M5 安全交易仍是必要前置。

Canonical detail：
`docs/architecture/adr/0028-explainable-screenshot-review-groups.md`。

## ADR-029 — Smart Cleanup Inbox is a derived suggest-only read model

Status：Accepted。Smart Cleanup Inbox v1 不新增第四套 mutable candidate schema，而是明確由 exact-duplicate、directional-version 與 screenshot-group 的 immutable evidence 衍生。使用者選定一個 completed-scan、active-grant scope 後才可手動 refresh；duplicate/version 必須 live reverify，screenshot group 必須解析到目前 evidence observation。只有 `suggested` 來源會進入 path-free Inbox；relation 的 accepted/rejected 都不是 cleanup consent。輸出固定 `cleanup_authorized: false` / `action_authorized: false`，不含路徑、檔名、OCR、keeper、selection、reclaimable bytes、ActionPlan 或任何 filesystem action。

Canonical detail：
`docs/architecture/adr/0029-smart-cleanup-inbox-derived-read-model.md`。
