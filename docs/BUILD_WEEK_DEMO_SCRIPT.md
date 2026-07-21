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

## English voiceover script (Build Week submission cut)

Use this version for the English-language submission video. It is written for
the existing 95-second guided cut; read it at a calm, conversational pace.
Do not add claims beyond what is on screen.

**0:00–0:10**

DeskGraph: graphify your computer. It is a local-first context graph for the
folders a person explicitly chooses to share with it.

**0:10–0:30**

This is a real Desktop state using synthetic files. A user authorizes one
folder with the native macOS picker, then separately starts a metadata scan.
Here, DeskGraph finds seven files, five folders, and zero issues. Granting
access does not automatically read file content or upload anything.

**0:30–0:50**

Search runs locally in SQLite. A search for README returns one metadata result
in eight milliseconds. Text extraction remains an explicit, local action, and
each result explains which local field matched.

**0:50–1:05**

Those safety boundaries are product features: paths and content stay on this
computer by default; language models can only suggest; and Smart Cleanup is
preview-only. It cannot delete or move a file.

**1:05–1:25**

Our reproducible CLI fixture verifies Traditional Chinese and English search,
a project candidate, two cleanup suggestions, and a durable system-trash
preview that is deliberately not executable. The source files stay unchanged.
Codex, powered by GPT-5.6, helped us implement, review, and validate these
safety-bounded slices under human direction.

**1:25–1:35**

This is a guided demo built from real Desktop states and CLI evidence, not a
continuous recording or a finished v0.1 release. Features without proof stay
closed.

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

## 功能凍結後的最終錄影 Checklist

這是重錄或製作正式提交版本時的可重現流程。它只使用合成資料與本機檔案；不要
錄製私人 Desktop、通知、終端歷史、瀏覽器分頁或任何雲端帳號。不要在此流程中上傳
影片、提交 Devpost 或發布 Release。

### 1. 凍結與驗證

- [ ] 在要提交的 commit 上執行，先記下 `git rev-parse --short HEAD`，並確認
      `git status --short` 沒有意外修改。
- [ ] 在錄影前執行並保留原始終端輸出；只在最終剪輯顯示必要的通過摘要：

  ```bash
  cargo test --workspace --all-features -- \
    --skip macos_recommended_watcher_delivers_a_live_file_hint \
    --skip macos_native_runtime_reconciles_create_modify_rename_and_delete \
    --test-threads=1
  pnpm check
  ```

- [ ] 若任一指令失敗，停止錄影與提交宣稱；修正後從同一份清單重新開始。
- [ ] 不把兩個被略過的 macOS live-event 測試描述成通過，也不以錄影取代跨平台
      或 packaged-build 證據。

### 2. 建立隔離的展示資料

- [ ] 使用新、空的暫存目錄建立 CLI 展示資料；命令會拒絕覆寫既有目錄：

  ```bash
  cargo run -p deskgraph-cli -- fixture demo \
    --path /private/tmp/deskgraph-demo-final
  ```

- [ ] Desktop 畫面只授權一個事先檢查過、只含合成檔案的資料夾。不要選取真實
      Desktop、Downloads、Documents 或任何使用者資料夾。
- [ ] 明確保留 Desktop 資料庫與 CLI fixture 資料庫是兩個獨立證據的說明；不可剪接
      成同一個即時共享資料庫的印象。

### 3. 錄製環境

- [ ] 使用獨立 macOS 使用者帳號或乾淨的展示桌面；開啟專注模式並關閉通知預覽。
- [ ] 將錄製範圍鎖定為 DeskGraph 視窗（1280×720 或更高），不要錄整個螢幕、Dock、
      選單列、其他 App 或終端機以外的私人內容。
- [ ] 若必須顯示終端，只顯示新開啟的乾淨視窗與上述 fixture／測試指令；確認提示字元、
      路徑和輸出不含使用者名稱、其他專案或 token。
- [ ] 在開始前走一次完整路徑，確認空態、載入態與錯誤態不會暴露先前使用過的 scope。

### 4. 拍攝與剪輯

- [ ] 依上方 1:35 shot list 逐段錄；每段只拍可在目前 build 真實重現的畫面。
- [ ] 以「guided demo」標籤開場與收尾，保留目前的限制：本機、opt-in extraction、
      Preview-only Cleanup、沒有自動 Watch Mode 宣稱。
- [ ] 將旁白與畫面對齊，但不要用旁白補足尚未可操作的功能；保留「不是完成的正式
      v0.1」這個邊界。
- [ ] 匯出前以靜音檢視一次，再以完整旁白檢視一次；確認沒有私人資訊、過度聲稱、
      未授權檔名或可執行檔案操作。

### 5. 交付前記錄

- [ ] 用 `ffprobe` 記錄最終檔的時長、解析度、編碼與是否有音訊；保存 SHA-256。
- [ ] 將成片保留在忽略版本控制的 `artifacts/demo/`；只把 script、驗證紀錄與公開可用
      的衍生素材納入 Git。
- [ ] 人工確認 Devpost 的影片 URL、公開說明、隱私措辭與提交截止時間，再由專案擁有者
      執行上傳與提交。
