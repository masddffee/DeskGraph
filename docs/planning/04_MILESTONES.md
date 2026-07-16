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
- Undo。
- Crash recovery。
- Audit log。

Done：
- Fault injection 測試。
- 中途斷電模擬。
- 檔名衝突。
- 跨 volume move。
- Undo 後 hash 一致。
- 永久刪除不存在於程式碼路徑。

## M6 — Watch Mode and Smart Inbox

- OS watcher。
- Stability check。
- Incremental indexing。
- Smart Inbox。
- Notification policy。
- Background resource controls。

Done：
- 大量事件 debounce。
- Rename / move event reconciliation。
- 暫存下載不被提早處理。

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
- Smart Inbox。
- Action Preview。
- History / Undo。
- Settings。
- Model manager。
- Privacy page。

Done：
- Keyboard navigation。
- Empty / loading / error states。
- UI 無需理解 GraphRAG 術語。

## M9 — Release Engineering

- macOS package。
- Windows package。
- Signing config。
- Updater。
- GitHub Release automation。
- SBOM。
- Checksums。
- Clean VM smoke tests。

Done：
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
