# Post-launch Operations

## First 24 Hours

Priority order：

1. Data loss / incorrect move。
2. Installer cannot open。
3. Crash on first scan。
4. CPU / memory runaway。
5. Scope / privacy violation。
6. Search broken。
7. UI polish。

Actions：

- Maintain public incident issue。
- Pause updater channel if critical。
- Publish hotfix with clear notes。
- Do not hide known limitations。
- Add regression test for every critical bug。

## First 7 Days

- Cluster issues by platform and workflow。
- Interview at least 10 activated users。
- Compare intended vs actual first-value path。
- Publish v0.1.1。
- Mark low-priority feature requests instead of immediately building.
- Create top 10 FAQ。
- Identify which positioning converts best。

## Days 8–30

Decide dominant wedge：

- Organizer-first。
- Search-first。
- Project Graph-first。
- MCP / Agent Context-first。

Decision inputs：

- Feature usage。
- Retention。
- User interviews。
- GitHub discussions。
- Contribution behavior。
- False move / undo data。
- Performance support burden。

## Kill / Pivot Criteria

Pause automatic organization if：

- Undo rate > 5%。
- False move reports exceed acceptable threshold。
- Users avoid action preview because explanations are unclear。

Pivot away from whole-computer indexing if：

- First scan completion < 40%。
- Resource complaints dominate。
- Most retained users only use one folder.

Focus on MCP if：

- MCP users retain 2× more。
- Most external contributions target integrations。
- Search API usage exceeds desktop UI search.

Focus on screenshots if：

- Screenshot workflows dominate activation and retention。
- OCR search is the main aha moment。
