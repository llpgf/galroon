# VNDB 雙向遊玩狀態與評分同步

「設定 → 帳號 → VNDB 雙向同步」連接 [VNDB 個人權杖](https://vndb.org/u/tokens)，必須同時授予 `listread` 和 `listwrite`。權杖保存在執行 Core 的 macOS Keychain 或 Windows Credential Manager；Linux Core（NAS）存成裝置資料夾下 `credentials/` 的擁有者專用檔案（資料夾 0700、檔案 0600），不進入任何備份。

## 使用方式

1. 取得同步預覽。已有相同 VNDB ID 的本機作品會列為候選，需要確認對應；同一 VN 的原版與重製版不自動混用。
2. 遠端有收藏但本機沒有卡時，可建立無資源收藏卡，包含狀態與評分，不建立檔案或下載任務。僅同步已關聯模式不自動建卡。
3. 首次狀態或評分不同時，分別選擇 Galroon → VNDB 或 VNDB → Galroon。之後每個欄位依上次成功值判斷方向；兩邊同時改成不同值則保留衝突。
4. 作品頁可編輯「我的評分」，支援 1.0–10.0、0.1 間距；留空代表取消評分。這是個人評分，不是 VNDB 的社群平均分。
5. 作品頁可用 `v17` 或 `https://vndb.org/v17` 關聯已有作品。後續資源可掛到無資源卡，合併作品會移轉同步關聯、狀態與評分。
6. 首次確認後可啟用自動同步。Core 每 10 秒檢查本機待上傳變更，每 5 分鐘讀取遠端；大量項目會依序處理並節流，實際完成時間取決於項目數及網路。

## 對應規則

| Galroon | VNDB 標籤 ID |
| --- | --- |
| 無狀態 `none`（新作品預設） | 不在清單或沒有 1–5 標籤 `0` |
| 遊玩中 | Playing `1` |
| 完成 | Finished `2` |
| 暫緩 | Stalled `3` |
| 放棄 | Dropped `4` |
| 想玩 | Wishlist `5` |

願望單可與遊玩標籤並存；讀取時優先採用唯一的 Playing/Finished/Stalled/Dropped，只有願望單則為想玩。寫入有狀態的值時只增減這四種遊玩標籤，想玩時加上願望單；既有願望單、黑名單及自訂標籤保留。遠端同時有多個遊玩標籤（`-1`）才標為 unsupported，評分仍可獨立同步。只有黑名單／評分／自訂標籤時自然對應無狀態 `0`，匯入為 `none` 並保存基準 `0`；多重遊玩標籤匯入為 `none`，基準仍為空，留待明確選擇。

本機 `none` 且遠端不在清單或狀態也是 `0` 時視為一致，不建立清單項目；遠端有狀態且基準為空或 `0` 時自動拉取。已有同步基準、遠端未變而本機清成 `none` 時，推送 `{"labels_unset":[1,2,3,4,5]}`，保留黑名單、自訂標籤及評分。兩邊都改變且不同時仍保留衝突。

VNDB 評分為整數 10–100，對應畫面上的 1.0–10.0。取消評分傳送 `vote: null`。只改評分不寫標籤，只改狀態不寫評分；日期、筆記、遊玩時數、發行版本及遊戲存檔不在同步範圍。新增遠端收藏與評分沿用 VNDB 的帳號標籤隱私設定，不宣稱自動設為私密。

## 衝突與資料保留

預覽與套用均會確認相關欄位的本機及遠端值。狀態與評分可以同時往不同方向同步。VNDB 沒有條件寫入 API，遠端讀取和寫入之間仍有競爭窗口；下一輪重新讀取。同步途中再次編輯本機值會保留為下一次待同步修改。

遠端未回傳條目不自動刪除本機卡，也不自動重新建立遠端收藏；必須明確選擇建立。先建立其中一個欄位後，另一個欄位需重新預覽。網路、權限或限流錯誤會保留成功基準及待同步修改，稍後重試。

無資源卡可隱藏並暫停同步，從「顯示全部」恢復。略過與解除作品關聯會阻止自動重新匯入。解除帳號綁定保留作品卡、評分與關聯，但清除帳號憑證、遠端快照及基準。收藏備份保留卡片和評分，不包含帳號設定或 Token；還原後須重新連接帳號。

## Core API

Schema 53 將未曾同步成想玩的 `backlog` 遷移為 `none`，移除其狀態覆寫並遞增 revision；VNDB 基準 `5` 或 Bangumi 基準 `1` 的作品保留想玩。新作品預設為 `none`。

Schema 50 增加 `works.personal_rating`（nullable 整數 10–100），以及 `vndb_sync_links`、`vndb_sync_remote`、`vndb_sync_ignored`。同一 VN 僅允許一張卡參與同步，避免多版本卡互相覆寫。Bangumi 及 VNDB 使用獨立帳號、憑證與同步基準；同一張卡的遊玩狀態是共用的。

- `GET /api/vndb_sync`：帳號摘要及預覽列，不含憑證。
- `POST /api/vndb_sync/connect`：`{token}`，驗證 `/authinfo` 與權限。
- `POST /api/vndb_sync/disconnect`：解除帳號。
- `POST /api/vndb_sync/settings`：`{mode: "full" | "existing", automatic}`。
- `POST /api/vndb_sync/preview`：背景取得完整收藏，更新預覽。
- `POST /api/vndb_sync/apply`：`{preview, choices: [{subject_id, field, fingerprint, action, work_id}]}`。subject_id 為 VN 數字部分；field 為 `status`、`rating` 或未關聯的 `entry`；action 為 `local`、`remote`、`same`、`import`、`match`、`skip`。
- `POST /api/vndb_sync/link`：`{work_id, revision, subject}`，空 subject 解除關聯。
- `POST /api/vndb_sync/hide`：`{work_id, hidden}`。
- `POST /api/vndb_sync/rating`：`{work_id, revision, rating}`，rating 為整數 10–100 或 null；只儲存本機評分，不直接呼叫 VNDB。

預覽與套用立即回傳 `running: true`，前端輪詢 GET 直到完成；結果儲存在帳號同步狀態。關閉頁面不取消已啟動的工作，Core 重啟則保留已成功項目的基準，剩餘項目再次預覽後重試。

操作限可寫入的桌面／擁有者角色，唯讀瀏覽器不能使用同步 API。官方契約：[VNDB Kana API](https://api.vndb.org/kana)。

## 驗證

使用隔離 Core 與模擬 VNDB HTTP 測試欄位獨立同步、衝突、取消評分、標籤保留、失敗重試、分頁、無資源卡、合併、備份與遷移。瀏覽器驗收涵蓋同步選擇及本機評分儲存／清除；不使用真實帳號，不代表真實 Token 或 Windows 憑證端到端已驗收。
