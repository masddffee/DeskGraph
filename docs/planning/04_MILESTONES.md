# Milestones and Definition of Done

## M0 — Repository Foundation

- Monorepo 建立。
- Rust / TypeScript lint、format、test。
- ADR 模板。
- AGENTS.md。
- CI matrix。
- License、Security、Contributing。
- Architecture skeleton。

Done：
- 所有平台 CI 通過。
- 空白 App 可啟動。
- CLI 可執行 health check。

## M1 — Manifest Graph

- Scope selector。
- Scanner。
- File identity。
- Metadata DB。
- Node / Edge schema。
- Scan progress。
- Exclusions。
- Pause / resume。

Done：
- 10k fixture scan。
- 重新掃描冪等。
- 移動檔案後能維持 identity。
- 不進入系統與排除目錄。

## M2 — Content Intelligence

- Extractor interface。
- Text / Markdown / code。
- PDF。
- DOCX / PPTX / XLSX。
- Image metadata。
- OCR provider。
- Chunking。
- Error isolation。

Done：
- 壞檔案不會中止整批。
- 每個 Extractor 有 fixtures。
- 不執行巨集與附件。

## M3 — Hybrid Retrieval

- FTS5。
- Vector provider。
- Embedding cache。
- Query parser。
- RRF / score fusion。
- Search UI。
- Open file / reveal in folder。

Done：
- Search benchmark。
- 中文、英文混合查詢。
- 可說明命中理由。

## M4 — Project Graph

- Folder profile。
- Similarity graph。
- Entity candidate。
- Project cluster。
- User confirm / merge / split。
- Edge provenance。
- Exact duplicate、evidence-backed version 與 screenshot-group candidate provenance；screenshot group 永不單獨證明檔案可丟棄。
- Project page。

Done：
- Project 建議可被使用者修正。
- 修正會影響後續分數。
- 低信心不得自動歸屬。

## M5 — Safe Organization

- ActionPlan。
- Preview。
- Conflict policy。
- Move / rename transaction。
- Move-to-system-trash transaction；不提供 permanent delete 或 empty-trash 路徑。
- Undo。
- Crash recovery。
- Audit log。
- Packaged-private action process fence；Windows 必須先完成 ADR-027 的 package identity foundation；macOS `flock` candidate 另需通過 supported-version SIP container replacement proof，否則維持 unavailable。

Done：
- Fault injection 測試。
- 中途斷電模擬。
- 檔名衝突。
- 跨 volume move。
- Undo 後 hash 一致。
- Fence 在 action database 開啟前取得；paused live owner 不因 lease expiry 被 recovery 越過，crash／abandoned state 只進安全 recovery。
- macOS Trash 與 Windows Recycle Bin adapter 的 identity、collision、crash-recovery、external-empty 與 Undo runtime matrix 通過。
- Linux freedesktop Trash 僅需獨立 experimental artifact／evidence；未完成或失敗不得拖延 macOS／Windows，也不得宣稱 Linux cleanup 已驗證。
- 永久刪除不存在於程式碼路徑。

## M6 — Watch Mode and Smart Inbox

- OS watcher。
- Stability check。
- Incremental indexing。
- Smart Inbox。
- Smart Cleanup Inbox：從精確重複檔、有充分證據的舊版本與可解釋的截圖群組建立候選。
- Notification policy。
- Background resource controls。

Done：
- 大量事件 debounce。
- Rename / move event reconciliation。
- 暫存下載不被提早處理。
- Cleanup 建議逐項顯示 evidence、keeper、候選、預估數量／容量；群組或規則不得自動授權垃圾桶動作。
- v0.1 每次確認最多 100 個項目且 expected bytes 合計最多 100 GiB；每檔各自 immutable plan／journal／receipt／Undo，依序執行並在首個非 completed outcome 後停止，剩餘項目標記 `not_started`，不做 batch rollback。

## M7 — MCP

- stdio server。
- Read-only tools。
- Scope enforcement。
- Query logging。
- Prompt injection boundaries。
- Desktop setup instructions。

Done：
- MCP 無法讀取 scope 外路徑。
- Tool 回傳最少必要資訊。
- 無 write tools。

## M8 — Product UI

- Onboarding。
- First Scan。
- Dashboard。
- Search。
- Project page。
- Smart Inbox + Smart Cleanup Inbox review。
- Action Preview。
- Per-item／bounded-batch cleanup selection and confirmation。
- History / Undo。
- Settings。
- Model manager。
- Privacy page。

Done：
- Keyboard navigation。
- Empty / loading / error states。
- Cleanup evidence、確認、執行、復原、衝突與外部清空垃圾桶狀態皆可理解且可操作。
- UI 無需理解 GraphRAG 術語。

## M9 — Release Engineering

- 提前供 M2／M5 使用的 macOS App Sandbox scope/container identity 與 Windows package family identity foundation；這不代表 M9 完成。
- macOS package。
- Windows package。
- Signing config。
- Updater。
- GitHub Release automation。
- SBOM。
- Checksums。
- Clean VM smoke tests。

Done：
- macOS native folder selection／security-scoped bookmark／non-entitled replacement probe 與 Windows packaged identity 通過 clean-machine install／update／repair／uninstall matrix；macOS probe 失敗時不得啟用 action fence。
- Tag 可重現建置。
- Release assets 完整。
- Updater 簽章驗證。
- Rollback 文件。

## M10 — Launch

- README。
- Demo GIF。
- 90 秒影片。
- Website。
- Show HN 文案。
- Product Hunt assets。
- X / LinkedIn / Reddit posts。
- Issue templates。
- Public roadmap。
- v0.1.0 release。

Done：
- 使用者從 README 到成功索引不超過 5 個步驟。
- 至少一個平台公開發文完成。
- Launch-day support rotation 文件完成。
