# Product Definition

## 1. 產品定位

### 類別

Local-first Computer Context Graph / Semantic File Intelligence Layer

### 一句話

Turn your computer into a private, searchable context graph.

### 核心價值

- 找到不知道名稱與位置的檔案。
- 看懂散落檔案其實屬於同一個專案。
- 讓新下載與截圖自動獲得正確情境。
- 安全地建議歸檔與重新命名。
- 找出可解釋的重複檔、舊版本與截圖群組，經確認後安全移至系統垃圾桶並可復原。
- 讓 AI Agent 在使用者授權下取得可靠的本地 Context。

### 授權與覆蓋原則

DeskGraph 的目標是低摩擦地涵蓋電腦中有用的工作情境，但不以「先讀完整顆磁碟、之後再排除」作為預設。v0.1 採用使用者一次檢視並明確確認的 Coverage Set：可在同一個原生流程選取 Desktop、Documents、Downloads、Pictures、Screenshots 等主要範圍，Home 則是有清楚風險說明的進階選項。有效覆蓋範圍為：

`effective coverage = union(active user-confirmed roots) - union(active hard exclusions)`

Hard Exclusion 是真正的本機存取與索引拒絕，不是隱藏搜尋結果。新增排除或撤銷範圍後，DeskGraph 必須原子清除受影響的路徑、內容、OCR、FTS、Embedding、Graph 與衍生候選資料，且永遠不修改或刪除來源檔。Metadata 授權也不等同於 Content、OCR、Embedding 或檔案動作授權。

## 2. 第一目標使用者

### Primary

- 開發者、設計師、研究者、學生與創作者。
- Desktop、Downloads 與 Screenshots 長期混亂。
- 同時處理多個專案。
- 願意使用本地 AI 或開源工具。
- 重視隱私與可控性。

### Secondary

- 顧問、律師、財務與知識工作者。
- 需要跨 PDF、Office、圖片與截圖尋找內容。
- 不希望上傳私人文件到雲端。

## 3. 核心 Job to Be Done

### JTBD-1：找回檔案

當我只記得內容、事件或當時在做的事情時，我希望可以用自然語言找到相關檔案，而不必記得檔名或路徑。

### JTBD-2：理解專案

當多個檔案散落在不同資料夾時，我希望系統能辨識它們其實屬於同一個專案，並顯示判斷依據。

### JTBD-3：安全整理

當新檔案出現時，我希望系統提出正確的命名與歸檔建議，而且任何動作都能預覽及復原。

### JTBD-4：安全清理

當重複檔、舊版本或大量截圖佔用空間時，我希望系統顯示可驗證的判斷依據，讓我逐項確認後移至作業系統垃圾桶，並在垃圾桶項目仍存在時可靠復原。

### JTBD-5：提供 Agent Context

當我使用 Codex、ChatGPT、Claude 或其他 AI Agent 時，我希望它能透過標準介面搜尋我授權的本地資料，而不是無限制讀取整顆磁碟。

## 4. v0.1 必須具備

1. 初始範圍選擇與權限說明。
2. Metadata Manifest Scan。
3. 文字檔、Markdown、程式碼、PDF、DOCX、PPTX、XLSX 基礎抽取。
4. 圖片與截圖 OCR。
5. FTS + Vector Hybrid Search。
6. File、Folder、Project、Topic、Entity、App、Action 節點。
7. 有 provenance 的 edges。
8. Project Discovery。
9. Smart Inbox + Smart Cleanup Inbox：找出精確重複檔、有充分版本證據的舊版本與可解釋的截圖群組；只提出建議，經使用者逐項或有限批次確認後才可移至系統垃圾桶，並提供 durable journal、crash recovery 與 Undo。
10. 搬移與重新命名 Preview。
11. Transaction + Undo。
12. Watch Folder。
13. macOS、Windows Installer。
14. Read-only MCP。
15. Benchmark 與隱私報告。

## 5. v0.1 不做

- 永久刪除檔案或清空系統垃圾桶。
- 預設全磁碟掃描。
- Email、瀏覽器歷史或行事曆深度整合。
- 完整 Finder / Explorer 替代品。
- 團隊協作與雲端同步。
- 7B 以上模型常駐。
- 3D 力導向 Graph 作為主 UI。
- 自動執行 MCP 寫入工具。
- 手機 App。
- 付費牆。

## 6. North Star Metric

**Weekly Successful Context Retrievals**

定義：使用者透過語意搜尋、專案頁或 MCP 找到並開啟正確檔案的次數。

輔助指標：

- First Value Time。
- Search Success Rate。
- Suggestion Acceptance Rate。
- Cleanup Suggestion Acceptance Rate。
- Undo Rate。
- False Move Rate。
- False Cleanup Suggestion Rate。
- Trash Undo Success Rate；使用者或系統已在 DeskGraph 外清空垃圾桶者需獨立標記，不可偽裝為成功。
- Crash-free Sessions。
- Indexed Files per Watt / per Minute。
- GitHub Release Downloads。
- Stars / Visitors Conversion。
