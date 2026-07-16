# GitHub Launch and Marketing Plan

## 1. Launch Goal

第一波不是營收，而是：

- 建立可信的開源品牌。
- 取得高品質 Issues 與 Contributors。
- 驗證最強 Use Case。
- 累積 GitHub Stars、Downloads 與 Weekly Active Users。
- 找到產品應該繼續往 File Organizer、Local Search 或 Agent Context 發展。

## 2. Positioning Variants

### A — 最易懂

Your files organize themselves — locally and reversibly.

### B — 差異最大

Graphify your computer.

### C — AI Agent Audience

A private context graph for your entire computer.

### D — Privacy Audience

Search and organize every file without uploading your life.

README Hero 建議使用 B + 一句解釋：

> Graphify your computer.
> A local-first context graph that connects, searches and safely organizes your files.

## 3. Launch Assets

必須在 Release 前完成：

- 25 秒無聲 GIF。
- 90 秒旁白 Demo。
- 5 分鐘技術深度影片。
- Before / After screenshot。
- Graph architecture diagram。
- 8GB benchmark screenshot。
- Privacy architecture screenshot。
- MCP demo。
- App icon。
- Social preview 1280×640。
- Product Hunt gallery。
- Landing page。
- Press kit。
- FAQ。

## 4. Demo Storyboard

1. 顯示混亂的 Downloads 與 Screenshots。
2. App 掃描並顯示「偵測到 4 個專案」。
3. 搜尋「上個月 Google Play 付款驗證錯誤」。
4. 找到檔案，即使檔名是 Screenshot 2026-...。
5. 顯示為什麼它與專案相關。
6. 預覽重新命名與歸檔。
7. 執行。
8. 一鍵 Undo。
9. AI Agent 透過 MCP 查詢同一專案。
10. 顯示 Local / No cloud / 8GB-ready。

## 5. Launch Sequence

### T-14 至 T-7（與開發平行）

- 建立公開 Roadmap。
- 建立 landing page。
- 每 2–3 天發布 build-in-public clip。
- 邀請 20–50 位技術使用者測試。
- 收集 installer 與 first-value 問題。
- 準備 FAQ 與 Known Issues。

### T-6 至 T-1

- Release candidate。
- Demo 錄製。
- README freeze。
- 建立 Launch posts drafts。
- 安排發布日可即時回覆 Issue。
- 不公開要求投票或交換 Stars。

### Launch Day

順序：

1. GitHub `v0.1.0` Release。
2. 官網與下載頁。
3. Show HN。
4. X 長文 Thread。
5. LinkedIn 創辦故事與技術架構。
6. 相關 Reddit 社群，先確認各社群規則。
7. Dev.to / Hashnode 技術文章。
8. Discord / Slack 開源社群。
9. YouTube Demo。
10. Product Hunt 可同日或隔日，確保能持續回覆。

### Launch Day Operations

- 15–30 分鐘檢查 Issues。
- 對每個可重現 Bug 加 label。
- Installer 阻塞 Bug 優先。
- README confusion 當天修正。
- 6–12 小時內發布 hotfix，如有必要。
- 不與批評者爭論；要求重現資訊並公開修復。

## 6. Channel Copy

### Show HN Title

Show HN: I built a local context graph that searches and organizes your computer

### Show HN Opening

I kept losing screenshots, downloads and project files because folders only express one relationship: where a file lives. I built `<PROJECT_NAME>` to create a private context graph across files, projects, topics and activity. It runs locally, works without an API key, explains every suggested move, and can undo every file operation.

The interesting part is not the graph visualization. The graph drives retrieval, project discovery and safe organization. There is also a read-only MCP server so coding and AI agents can query only the folders you authorize.

I would especially value feedback on false relationships, resource use on 8GB machines, and which integrations are actually useful.

### X Hook

I turned my messy computer into a local knowledge graph.

Not another AI file sorter:
- no cloud
- no API key
- every relationship is explainable
- every move is reversible
- AI agents can query it through MCP

Open source: `<REPO>`

### Reddit Approach

不要跨社群貼完全相同內容。每篇必須：

- 先描述該社群的具體痛點。
- 公開限制。
- 不要求 upvote。
- 邀請測試特定功能。
- 回覆所有技術問題。

## 7. Product Hunt

- Maker 自己發布。
- 準備 tagline、gallery、demo、maker comment。
- 不要求使用者直接 upvote。
- 鼓勵試用與留下真實評論。
- 發布時間以能全日回覆為優先。
- Product Hunt 是第二波，不應延遲 GitHub Release。

## 8. Content Flywheel — 30 Days

### Week 1

- Architecture deep dive。
- Why folders are not enough。
- 8GB local AI benchmark。
- How Undo transactions work。
- First community fixes。

### Week 2

- OCR multilingual demo。
- MCP setup demo。
- User workflow case study。
- Performance improvements。
- Good First Issue spotlight。

### Week 3

- How project discovery works。
- Prompt injection threat model。
- macOS vs Windows filesystem edge cases。
- Contributor interview。
- v0.1.1 release。

### Week 4

- Metrics retrospective。
- Public roadmap decision。
- Plugin proposal。
- Community call。
- v0.2 preview。

## 9. Metrics Dashboard

Daily：

- Repo visitors。
- Unique cloners。
- Release downloads。
- Stars。
- Issues。
- Installer success reports。
- Website → GitHub conversion。

Weekly：

- Activated installations。
- First scan completion。
- Search success。
- Retention。
- Most-used file types。
- Crash-free sessions。
- Contribution count。

不要以 Stars 取代產品使用指標。
