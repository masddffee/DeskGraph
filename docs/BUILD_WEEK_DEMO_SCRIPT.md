# DeskGraph Build Week demo script

> **Current cut: 1:35.** This is a guided demo assembled from real DeskGraph
> Desktop states and a separately verified synthetic CLI fixture. It is not a
> continuous live-operation recording. Read the Traditional Chinese voiceover
> below at a calm pace and leave the final media under the three-minute cap.

## Before recording

Run the deterministic local checks and use the separately generated CLI proof:

```bash
cargo test --workspace --all-features -- \
  --skip macos_recommended_watcher_delivers_a_live_file_hint \
  --skip macos_native_runtime_reconciles_create_modify_rename_and_delete \
  --test-threads=1
pnpm check
cargo run -p deskgraph-cli -- fixture demo --path /absolute/new/path/deskgraph-demo
```

The two skipped tests are opt-in macOS live-filesystem-event tests whose
FSEvents callback is unavailable on this host. They are not a reason to claim
native Watch Mode, packaged support, or cross-platform validation; individual
runs also did not receive a callback on this host.

## Current 1:35 shot list

| Time      | Verified visual                                                                                                     |
| --------- | ------------------------------------------------------------------------------------------------------------------- |
| 0:00–0:10 | DeskGraph title card; explicitly labels the cut as a guided demo using synthetic data.                              |
| 0:10–0:30 | Real Desktop state after native authorization and Initial Manifest Scan: 7 files, 5 folders, 0 issues.              |
| 0:30–0:50 | Real Desktop metadata search for `README`: 1 local result in 8 ms with explicit content-extraction control visible. |
| 0:50–1:05 | Safety card: local search, opt-in extraction, and Preview-only Cleanup.                                             |
| 1:05–1:25 | Verified CLI fixture summary: bilingual retrieval, Project, two Cleanup candidates, non-executable Trash Preview.   |
| 1:25–1:35 | Evidence boundary: real Desktop states plus CLI verification; not represented as continuous live operation or v0.1. |

## 繁體中文旁白稿（可直接照讀）

請用自然語速試讀一次，必要時只縮短停頓，不要加快到難以理解。這份稿件對應目前
95 秒的無聲成片。

**0:00–0:10**

DeskGraph，把你的電腦脈絡化。一個本機優先、由使用者控制範圍的電腦情境圖譜。

**0:10–0:30**

這是真實桌面狀態。使用者用 macOS 原生選取器授權合成資料夾，再分開啟動中繼資料掃描。結果是七個檔案、五個資料夾、零個問題；授權不會自動讀取內容，也不會上傳。

**0:30–0:50**

搜尋完全在本機 SQLite 執行。以 README 查到一筆 metadata 結果，只花八毫秒。內容仍須按下「在本機抽取文字」才會讀取，搜尋結果也會說明匹配來源。

**0:50–1:05**

安全邊界就是產品功能：路徑與內容預設不離開電腦；LLM 只能提出建議；Cleanup 目前只有 Preview，不能執行或刪除。

**1:05–1:25**

可重現的 CLI fixture 驗證中英文搜尋、專案候選、兩筆 Smart Cleanup 建議，以及未授權、不可執行的系統垃圾桶預覽；所有來源檔案保持不變。Codex 與 GPT-5.6 協助實作、審查與驗證。

**1:25–1:35**

這是由真實桌面狀態與 CLI 證據組成的 guided demo，不是連續操作錄影，也不是完成的正式 v0.1；未通過的功能仍保持關閉。

## Capture guardrails

- Do not imply the CLI fixture database and Desktop app database are one live
  shared state; they are deliberately separate.
- Do not call Watch Mode incremental or automatic; the demonstrated product
  retains a bounded reconciliation fallback and lacks complete incremental
  content re-indexing evidence.
- Do not show or describe any file action as executable. Current Rename and
  Cleanup workflows are Preview-only.
- Do not add root revocation to the timed shot list unless the real Desktop
  flow has first been rehearsed and every search/extraction shot is already
  captured. Revocation purges only DeskGraph's local derived state and access
  grant, never a source file; it is local development evidence, not packaged
  or hostile-process proof.
- Show the actual test output from the final submission commit. Do not replace
  counts or claim broader platform evidence than that output supports.
