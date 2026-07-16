# Computer Context Graph — Codex Implementation & Launch Pack

這是一套可直接交給 Codex 執行的完整規劃，目標不是做一個普通的 AI File Sorter，而是建立：

> **Local-first Computer Context Graph**
> 將電腦中的檔案、資料夾、專案、人物、應用程式、時間與關係轉換成可搜尋、可解釋、可安全整理的本地語意圖譜。

## 推薦執行方式

不要一次把所有內容貼給 Codex。依序執行：

1. 將 `templates/agents/AGENTS.md` 放到 Repository 根目錄。
2. 將本資料夾的計畫文件放到 `docs/planning/`。
3. 先執行 `prompts/00_MASTER_ORCHESTRATOR.md`。
4. 再按照 `prompts/01` 到 `prompts/15` 依序執行。
5. 每個階段必須通過驗收條件後，才能進入下一階段。
6. v0.1 完成後執行 `prompts/14_RELEASE.md`。
7. 發佈完成後立刻執行 `prompts/15_LAUNCH_MARKETING.md`。

## 推薦版本

採用 **Version B：Production Open-source MVP**。

它會同時完成：

- macOS 與 Windows 安裝程式
- Desktop / Downloads / Screenshots / Documents 掃描
- Local Context Graph
- OCR 與多語言語意搜尋
- Project Discovery
- Safe File Organization
- Dry Run、Preview、Undo
- Read-only MCP Server
- Benchmark
- GitHub Release
- README、Demo、Launch Assets
- 上線後 30 天成長流程

Version A 應作為 Version B 的中途里程碑，而不是獨立終點；Version C 則在 v0.1 公開後依真實使用資料決定。

## 不可妥協原則

1. **Local-first**：預設不傳送檔案內容、檔名、OCR 或 Embedding 到雲端。
2. **No destructive AI**：AI 不得永久刪除檔案。
3. **Preview before action**：第一次使用預設僅建議，不自動搬移。
4. **Everything reversible**：重新命名與搬移必須可 Undo。
5. **Graph with provenance**：每條推論關係都必須有信心分數、來源與時間。
6. **Rules first, embeddings second, LLM last**。
7. **Useful without an LLM**：使用者不下載本地 LLM 時，核心產品仍可使用。
8. **8GB-ready**：低記憶體模式是正式產品能力，不是備註。
9. **No setup maze**：一般使用者不得被要求安裝 Python、Node、Docker、Ollama 或 API Key。
10. **Release is part of Done**：功能完成但無安裝檔、文件、測試與 Release，不算完成。

## Codex 可自動完成與不能憑空完成的部分

Codex 可以在已授權的環境中：

- 建立程式碼、測試、文件、CI/CD。
- 建立 GitHub Repository、Issues、Tags、Release 與 PR。
- 產生 DMG、MSI/NSIS、AppImage 等建置流程。
- 產生 README、影片腳本、社群貼文與 Launch 頁素材。
- 在具備 GitHub CLI 登入與 Repository 權限時發佈 GitHub Release。

仍需要外部憑證或帳號的項目：

- Apple Developer ID、Notarization 憑證。
- Windows Code Signing 憑證。
- Product Hunt、X、Reddit、YouTube 等帳號授權。
- 網域與網站部署帳戶。
- 最終商標與產品名稱決定。

遇到上述憑證缺失時，Codex 不得假裝已完成；應完成所有可完成的內容，輸出一份精確的 `EXTERNAL_ACTIONS_REQUIRED.md`。
