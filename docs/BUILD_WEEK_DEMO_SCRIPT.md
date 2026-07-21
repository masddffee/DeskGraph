# DeskGraph Build Week demo script

> **Target runtime: 2:45.** Read at a calm, conversational pace. This leaves
> 15 seconds below the three-minute submission cap for capture transitions.
> Record only synthetic files in a newly created demo folder. Do not show
> personal paths, file contents, access tokens, or private OCR text.

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

## Timestamped shot list and voiceover

| Time      | Screen action                                                                                                                          | Voiceover                                                                                                                                                                                                                                                                                                                                     |
| --------- | -------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 0:00–0:18 | Home screen: local-only status and safety contract.                                                                                    | “Computers hold context that could make AI useful, but giving an AI your entire filesystem is not acceptable. DeskGraph is a local-first context graph: it keeps its SQLite manifest and search on this machine, and starts only with folders a person explicitly chooses.”                                                                   |
| 0:18–0:36 | Use the native picker to select a harmless synthetic folder; show that scan is still separate.                                         | “The WebView does not submit an arbitrary path. Native selection grants one local scope. Selecting it neither reads file contents nor starts a scan, so consent and indexing are separate actions.”                                                                                                                                           |
| 0:36–0:55 | Run and finish an Initial Manifest Scan; show metadata search.                                                                         | “Now I run a metadata-only Initial Manifest Scan. It is resumable and atomically publishes a completed manifest. Hidden entries and symlinks are not followed, and the search results explain whether a match comes from a filename or approved extracted text.”                                                                              |
| 0:55–1:16 | Explicitly extract the synthetic Markdown file, then repeat a Traditional Chinese or English content search.                           | “Content is opt-in. I explicitly choose this scanned file; the Rust backend rechecks the live grant and file identity, runs a bounded durable job, and labels its text untrusted. SQLite FTS then finds this Traditional Chinese or English text locally.”                                                                                    |
| 1:16–1:39 | Terminal: run or show the successful `fixture demo` JSON fields for Project, duplicate/version, Inbox, Preview, and unchanged sources. | “For repeatability, this one-command fixture creates harmless bilingual files and drives the real Rust and SQLite cores. It verifies extraction, search, Project and relation suggestions, plus a Smart Cleanup Preview. The report proves source files stayed unchanged: Preview requires confirmation, but execution is unavailable.”       |
| 1:39–1:57 | Start MCP and show one `search_files` call over a completed, explicitly granted scope.                                                 | “For agents, the local MCP server exposes one read-only search tool. It has no arbitrary path parameter and no write tool. Content snippets are opt-in and visibly untrusted.”                                                                                                                                                                |
| 1:57–2:23 | Show `git log` with the cited commits and the deterministic test command/result.                                                       | “Codex with GPT-5.6 was my development collaborator: it explored the codebase, helped implement and review safety-bounded slices, and ran validation gates under human direction. The dated commits show that progression. Codex also helped reproduce a Unix wrong-inode rename race, so the product deliberately refuses unsafe execution.” |
| 2:23–2:45 | Return to the Preview-only state and the limitation list.                                                                              | “This is a pre-release development build, not a finished v0.1 release. Vector or hybrid search, executable rename, move, Trash, Undo, installers, and cross-platform runtime validation remain gated. DeskGraph’s promise today is useful local context with explicit scope and honest refusal, not unsafe automation.”                       |

## 繁體中文旁白稿（可直接照讀）

請先用自然語速試讀一次；畫面轉場較慢時可以停頓，不要加快到難以理解。這份稿件約
2 分 40 秒，對應上方的 2:45 畫面規劃。

**0:00–0:18**

電腦裡保存了許多能讓 AI 真正理解工作的脈絡，但把整個檔案系統直接交給 AI，並不安全。DeskGraph 是一個本機優先的電腦情境圖譜；SQLite 清單、搜尋與衍生資料都留在這台電腦，而且只從使用者明確選取的資料夾開始。

**0:18–0:36**

授權由作業系統的原生選取器完成，網頁介面不能自行送入任意路徑。選取一個資料夾，只建立可檢查的範圍，不會立即掃描、不會讀取檔案內容，也不會啟動 OCR 或模型。授權與建立索引是兩個分開的決定。

**0:36–0:55**

現在我明確啟動一次初始中繼資料掃描。它可以中斷後恢復，最後以原子方式發布完整清單；隱藏項目與符號連結不會被跟隨。完成後，離線搜尋會清楚說明結果是符合檔名，還是符合經過使用者允許後抽取的文字。

**0:55–1:16**

讀取內容仍然是選擇性的。我只對這個已掃描的合成 Markdown 檔案建立受限抽取工作。Rust 後端會重新檢查授權、檔案身分、大小與時間限制，並將結果標示為不受信任的抽取文字。接著，繁體中文與英文都能在本機 SQLite 全文搜尋中被找到。

**1:16–1:39**

為了讓評審可以重現，這個單一指令會建立完全無害的雙語範例，並驅動真正的 Rust 與 SQLite 核心。它驗證專案探索、精確重複檔、檔名版本關係，以及 Smart Cleanup Inbox。清理結果只是一份可解釋的 Preview；報告也證明所有來源檔案保持不變，執行權限仍然關閉。

**1:39–1:57**

對 AI Agent，DeskGraph 另外提供獨立啟動的唯讀 MCP。現在只有一個搜尋工具，沒有任意路徑參數，也沒有寫入、移動、重新命名或刪除工具。內容片段必須先由使用者選擇抽取，回傳時仍會標示為不受信任的本機資料。

**1:57–2:23**

Codex 與 GPT-5.6 是這次開發的協作工具：它們協助探索架構、拆解安全邊界、實作小型垂直切片，並執行格式、靜態分析、型別與測試驗證。這些日期化 commit 保留了進度。更重要的是，測試實際重現過 Unix 重新命名可能移動錯誤 inode 的競態，因此產品選擇拒絕不安全的執行，而不是為了展示而降低規則。

**2:23–2:45**

這仍然是預先發行的開發版本，不是已完成的公開 v0.1。向量與混合搜尋、可執行的重新命名、移動、系統垃圾桶、Undo、安裝程式，以及完整跨平台執行證據，仍然保持關閉。DeskGraph 今天證明的是：本機情境可以有用、範圍可以明確、建議可以解釋，而且在安全證據不足時，產品會誠實拒絕操作。

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
