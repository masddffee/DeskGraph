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
