# 遊玩狀態：新增「無狀態」並把「待玩」改名為「想玩」

日期：2026-10-16　狀態：計畫，等待確認

## 目標

- 新增狀態 `none`（無狀態），作為新作品的預設值。
- `backlog` 的內部值不變，顯示名稱從「待玩／Backlog」改為「想玩／Want to play」。它的語意是「明確想玩」，對應 Bangumi 想玩 `1` 和 VNDB Wishlist `5`。
- 現有的 `backlog` 作品，只有「已經和遠端同步成想玩」的保留，其餘一律遷移成 `none`。
- Bangumi／VNDB 同步把 `none` 視為「沒有遊玩狀態」，並且絕不因為 `none` 去建立或覆蓋遠端收藏。

## 前提假設（請確認）

1. 「已與遠端同步成想玩」的判定方式：`bangumi_links.base_status = 1` **或** `vndb_sync_links.base_status = 5`。只看遠端快照、但從未成功同步的作品不算。
2. 使用者以前在作品頁手動選過「待玩」的作品（`overrides.status = 'backlog'`），也照你的決定遷移成 `none`，不另外保留。
3. 內部值 `backlog` 不改名，所以智慧清單定義、收藏備份、localStorage 中的首頁欄位順序都不需要遷移。

## 狀態對應

| Galroon | 顯示 | Bangumi `type` | VNDB 標籤 |
| --- | --- | --- | --- |
| `none` | 無狀態 | 未收藏（同步時以 `0` 表示） | 不在清單，或在清單但沒有 1–5 標籤（`status()==0`） |
| `backlog` | 想玩 | 1 想玩 | 5 Wishlist |
| `playing` | 遊玩中 | 3 在玩 | 1 Playing |
| `completed` | 已完成 | 2 玩過 | 2 Finished |
| `on_hold` | 暫緩 | 4 擱置 | 3 Stalled |
| `dropped` | 已放棄 | 5 拋棄 | 4 Dropped |

已確認 Bangumi v0 API 的 `/v0/users/-/collections/{subject_id}` 只有 GET/POST/PATCH，**沒有 DELETE**，所以 Galroon 無法把 Bangumi 收藏改回「未收藏」。

## Core

### C1. Schema 53 遷移（`crates/core/src/db.rs`）

- `SCHEMA_VERSION` 從 52 改為 53。現有的 migration-backups 快照機制會自動保護舊資料庫。
- 新資料庫：`CREATE TABLE works` 的 `status` 預設值改為 `'none'`。
- 既有資料庫的欄位預設值：SQLite 不能用 ALTER 修改 DEFAULT。採用 SQLite 官方文件允許的 `writable_schema` 做法（只改 `sqlite_schema` 中 `works` 的 DEFAULT 文字，再遞增 `schema_version` 並跑 `PRAGMA integrity_check`）。另外，三個正式寫入點都明確寫入 `status`（見 C2），作為雙重保險。
  - 替代方案：用 12 步驟重建 works 表。由於 works 被很多 FK 和 trigger 引用，風險較高，不採用。
- 資料遷移，在同一個交易內執行：
  ```sql
  UPDATE works SET status='none', revision=revision+1,
         overrides=json_remove(overrides,'$.status')
   WHERE status='backlog'
     AND NOT EXISTS(SELECT 1 FROM bangumi_links b WHERE b.work_id=works.id AND b.base_status=1)
     AND NOT EXISTS(SELECT 1 FROM vndb_sync_links v WHERE v.work_id=works.id AND v.base_status=5);
  ```
  這會影響 merged_into 的舊卡，但沒有關係，因為合併時狀態已經移轉到目標卡。
- 遷移會觸動 catalog 變更追蹤。要確認 `catalog_changes` 和 smart_catalog_state 的 revision 有遞增，讓已開啟的瀏覽器重新整理。

### C2. 寫入路徑

- `lib.rs` `update_work`：允許值加上 `none`。
- `lib.rs` `create_work_in`、`automatch.rs:929`、`grouping.rs:29`：INSERT 明確寫入 `status='none'`。合併建立的新卡如果另有沿用來源狀態的邏輯，維持原樣。
- `collection_query.rs:27`：status 篩選允許值加上 `none`。
- 智慧清單的 status 欄位選項加上 `none`。後端如果有驗證允許值，同步更新。

### C3. Bangumi 同步（`crates/core/src/bangumi/`）

- `remote_status("none") → 0`；`local_status(0) → "none"`。只有 `local_status` 在處理遠端回應時，才繼續拒絕 0（`remote.rs:67,75` 驗證遠端回傳值必須在 1–5）。
- `classify` 增加 `none` 規則，放在一般規則之前：
  - 本機 0、遠端未收藏：`same`，不列入「missing」，也不提示建立遠端收藏。
  - 本機 0、遠端 r，base 為 None 或 0：`pull`（無狀態不會蓋掉遠端）。
  - 本機 0、遠端 r，base == r（使用者把本機清成無狀態）：新類型 `cleared`。不自動處理，預覽列顯示「本機已清除狀態；Bangumi 無法取消收藏，遠端保留」，可選「改用遠端狀態」或「忽略」。
  - 其他情況沿用現有規則（例如兩邊都改了就是 conflict）。
- 推送時如果 value 是 0，一律拒絕（防呆）。
- `mod.rs:192` 的 dirty 查詢 `CASE base_status` 加上 `WHEN 0 THEN 'none'`，並確認 `cleared` 不會觸發自動同步的無限迴圈。
- 「missing」列提供的「建立遠端收藏」動作，在本機為 `none` 時隱藏或拒絕。

### C4. VNDB 同步（`crates/core/src/vndb_sync/`）

- `remote_status("none") → 0`；`local_status(0) → "none"`。`-1`（多個遊玩標籤）仍然是 unsupported。
- `store.rs:31`：`unsupported` 只套用在 `remote < 0`，`remote == 0` 改走一般 classify。
- `classify` 套用和 Bangumi 相同的 `none` 規則，但「使用者把本機清成無狀態」可以真的推送：
  - `remote::patch("status",0)` → `{"labels_unset":[1,2,3,4,5]}`，保留黑名單、自訂標籤和評分。
  - 遠端不在清單、本機為 0：`same`，不建立清單項目。
- 匯入（`store.rs:43`）：遠端 status 0 時，卡片狀態設為 `none`（取代現在的 `unwrap_or("backlog")`），`base_status` 設為 `Some(0)`。
- 文件中「只有黑名單／評分時先顯示待玩」的特例，改成自然對應到無狀態。

### C5. 統計與其他

- `StatsPanel` 對應的後端計數（`status_counts`）加上 `none`。
- 發售日曆（`release_calendar.rs`）照常回傳 status，由前端決定徽章文字。

## 前端

| 檔案 | 改動 |
| --- | --- |
| `src/locales/zh-TW.ts`、`src/i18n.ts` | `backlog` 改成「想玩」／"Want to play"；新增 `none`：「無狀態」／"No status"；Bangumi 預覽列新增 `cleared` 類型的說明文字 |
| `CollectionWorkDetail.tsx:29` | 下拉選單第一項是「無狀態」，接著想玩、遊玩中…… |
| `HomePanel.tsx:12` | 欄位維持五種有狀態的值，不新增 `none` 欄位；標題列旁邊顯示「無狀態 N 部」連結到收藏篩選。`savedOrder` 的長度檢查不變（STATUSES 不變） |
| `LibraryBrowser.tsx:15,78` | 分段與 facet 加上 `none`；封面角標改成 `w.status!=='none'` 才顯示，讓「想玩」出現在封面上 |
| `ReleaseCalendar.tsx:63` | 狀態為 `none` 時顯示「已收藏」，`backlog` 才顯示「想玩」（現在的邏輯已經是這樣，只需確認） |
| `StatsPanel.tsx:8`、`theme.css` | 加入 `none` 區段，使用最淡的 track 色；`backlog` 改用 accent-soft，和日曆徽章一致 |
| `SmartListsPanel.tsx:31`、`smartLists.ts:6` | 選項加上 `none`；新條件的預設值維持 `backlog`（想玩） |
| `bangumi.ts`、`vndb.ts` | 遠端值 0 顯示為「無狀態」（VNDB）／「未收藏」（Bangumi，原本就是 null） |
| `library.test.ts`、`paging.test.ts` | fixture 的預設狀態改成 `none` |

## 文件

- `docs/features/BANGUMI.md`、`docs/features/VNDB_SYNC.md`：更新對應表、`none` 規則，以及「Bangumi 不能取消收藏」的說明。

## 測試

Core（`cargo test --lib --bins --tests`）：
- 遷移測試：建立一個 v52 資料庫，包含以下作品：
  - backlog，未連結 → `none`
  - backlog，Bangumi base=1 → 保留
  - backlog，VNDB base=5 → 保留
  - backlog，Bangumi 連結但 base=NULL → `none`
  - playing → 不變
  - 遷移後新 INSERT（不指定 status）→ `none`；`integrity_check` = ok；重開資料庫不再重跑遷移
- Bangumi：none＋遠端未收藏 → same、不寫遠端；none＋遠端 3 且 base 為 None → pull 成 playing；base=1 且本機改成 none → cleared、不寫遠端；推送 0 → 拒絕。
- VNDB：none＋遠端只有黑名單 → same；本機清成 none → 寫入 `labels_unset:[1..5]`、黑名單仍在；匯入 status 0 → `none`。
- `update_work` 接受 `none`，拒絕未知值。

前端：`npm test`，並用 Playwright／手動檢查首頁、收藏篩選、作品頁下拉選單、統計頁和發售日曆。

## 部署注意

- NAS 正式資料庫第一次啟動 v53 時會遷移。預期大約 483 部作品中，除了已同步成想玩的以外，大部分 backlog 會變成 `none`。部署前先手動做一次 full 備份。
- 遷移後第一次 Bangumi 同步：原本 `initial`（本機預設 backlog、遠端不同）的列會變成自動 `pull`。這是預期行為，但會在 NAS 上一次改變很多作品的狀態，部署後要檢查預覽。

## 範圍外

- 不把 `backlog` 改名為 `wish`。
- 不同步評分以外的 Bangumi 欄位。
- 不提供「從 Bangumi 移除收藏」（API 不支援）。
