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
