# Implementation Plan Variants

## Version A — Viral Proof

### 目標

最快產生一個足以展示「Graphify your computer」價值的公開可執行版本。

### 平台

macOS Apple Silicon 優先。

### 包含

- Desktop、Downloads、Screenshots。
- Metadata Scan。
- Screenshot OCR。
- PDF / TXT / MD / Code 抽取。
- Hybrid Search。
- Project Clustering。
- Graph Relations。
- Dry-run Organization Suggestions。
- CLI + 基礎 Tauri UI。
- Demo Dataset。
- 不自動搬移。
- 不包含 MCP。
- 不包含 Windows。

### 交付門檻

- 一鍵安裝。
- 3 分鐘內出現第一個 Project Cluster。
- 30 秒 Demo 可清楚顯示 Before / After。
- 10,000 檔案不崩潰。
- 公開 Alpha Release。

### 用途

作為 Version B 第 1 個 Milestone，不建議停在此版。

---

## Version B — Production Open-source MVP（推薦）

### 目標

做完即可正式公開，具備足以建立信任、獲得 GitHub Stars 與真實使用者的完整產品。

### 平台

- macOS Apple Silicon + Intel。
- Windows x64。
- Linux 可建置但不保證 v0.1 正式支援。

### 包含

Version A 全部，加上：

- Office 檔案抽取。
- Watch Folder。
- Smart Inbox + Smart Cleanup Inbox（精確重複檔、證據充分的舊版本、可解釋的截圖群組）。
- Folder Profiles。
- Project Discovery。
- Explainable Relations。
- Preview / Rename / Move。
- Transaction + Undo。
- Crash Recovery。
- Read-only MCP。
- Auto Updater。
- Signed Release Pipeline。
- Benchmark。
- Privacy & Threat Model。
- GitHub Community Files。
- 完整 Launch Campaign。

### v0.1 Release Gates

- 0 個已知資料遺失 Bug。
- 不存在無法 Undo 的 Move / Rename；移至系統垃圾桶的動作在垃圾桶項目仍存在時必須可 Undo。
- 自動模式預設關閉。
- Smart Cleanup 永遠需要使用者確認；不得自動移至垃圾桶、永久刪除或清空垃圾桶。
- 每個 Cleanup 動作都通過 Preview、Policy Validation、durable journal、執行後驗證與 crash recovery。
- macOS / Windows Installer 由乾淨 VM 驗證。
- 10k、100k 檔案 Benchmark 報告。
- Search p95、Memory、Scan Throughput 有實測。
- README Demo 可在 30 秒理解。
- MCP 只能讀取使用者授權範圍。

---

## Version C — Computer Context Platform

### 目標

把產品從檔案工具升級成整個 AI 生態的本地 Context Layer。

### 新增

- Linux 正式支援。
- Browser Extension。
- Source URL Capture。
- App Activity Context。
- Temporal Graph。
- Email / Calendar 選配 Connector。
- Plugin SDK。
- Rule Pack Marketplace。
- Write-capable MCP with approval.
- Graph Export / Import。
- Optional encrypted sync。
- Multi-device identity resolution。
- Developer API。
- Fine-grained access scopes。

### 啟動條件

只有在 v0.1 至少達到以下任一條件才開始：

- 1,000 位真實安裝使用者。
- 300 位週活躍使用者。
- 3,000 GitHub Stars。
- 50 位使用者主動要求 MCP / Plugin。
- Search Success Rate > 65%。

---

## Version D — Maximum-Star Launch Overlay

Version D 不是另一套核心，而是套用在 Version A 或 B 上的傳播增強層。

### 必備

- 無註冊即可使用。
- 開源 License 清楚。
- 一鍵下載。
- Demo Dataset 一鍵體驗。
- 20–30 秒 GIF。
- 90 秒影片。
- Architecture Diagram。
- 8GB RAM Benchmark。
- Privacy-first README。
- Show HN 可直接試用。
- MCP Demo。
- Good First Issues。
- Public Roadmap。
- Weekly Release cadence（前四週）。

### 不可做

- 只有 Landing Page 沒有可下載產品。
- 使用假 Benchmark。
- 為 Stars 交換投票。
- 宣稱「100% 正確」。
- 把未完成功能寫成已完成。
