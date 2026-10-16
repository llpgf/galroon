# Bangumi 雙向遊玩狀態同步

在「設定 → 帳號 → Bangumi 雙向同步」連接個人存取權杖。權杖可從 https://next.bgm.tv/demo/access-token 產生；Galroon 不接收 Bangumi 密碼。本版使用個人權杖，尚未提供 OAuth 瀏覽器授權。

## 使用方式

1. 選擇「完整收藏」或「僅同步已關聯的作品」，取得兩邊狀態並預覽。
2. 完整收藏會把 Bangumi 上沒有對應作品的遊戲建立為無本機資源的卡片。遇到相同名稱的現有作品，先確認關聯；不依名稱自動合併。比對名稱時忽略標點、空白、全形半形與括號內的讀音；只差副標題時，須發售日完全相同才列為候選，後面接數字的名稱視為續作，不列入。只有一個候選、且名稱完全相同或只差標點時，預覽會預先選取該作品；仍須檢查後按「套用」，自動同步不會自行關聯。也可以在作品頁貼上 Bangumi 遊戲網址或數字 ID。
3. 確認預覽的同步方向。第一次兩邊都有遊玩狀態且不同時，需要選擇 Galroon → Bangumi 或 Bangumi → Galroon。
4. 首次確認後，可以啟用自動雙向同步。Core 執行時，每 10 秒檢查本機待同步狀態，每 5 分鐘檢查遠端收藏。網路錯誤保留本機修改，稍後再試；實際完成時間取決於網路和待處理數量。

「僅同步已關聯的作品」只限制新作品匯入，已關聯作品仍然雙向同步。無本機資源的卡片同樣可以在 Galroon 修改狀態並上傳。收藏的資源篩選提供「有本機資源」與「沒有連結的資源」。

| Galroon | Bangumi 遊戲收藏 |
| --- | --- |
| 無狀態 `none`（新作品預設） | 未收藏（內部以 `0` 表示） |
| 想玩 `backlog` | 想玩 `1` |
| 遊玩中 `playing` | 在玩 `3` |
| 完成 `completed` | 玩過 `2` |
| 暫緩 `on_hold` | 擱置 `4` |
| 放棄 `dropped` | 拋棄 `5` |

本版同步遊玩狀態；評分、短評、標籤、時數、路線與遊戲存檔不在同步範圍。本機筆記不會上傳。更新既有 Bangumi 收藏只傳送 `type`；使用者明確要求建立遠端收藏時，預設為私密收藏。

## 衝突、隱藏與後續資源

系統比較本機狀態、遠端狀態與最後一次成功同步的狀態。只有一邊改變時向另一邊同步；兩邊都改成不同狀態時保留衝突，等使用者決定。同步前再次確認遠端與本機預覽，防止套用已過期的選擇。Bangumi 沒有條件寫入介面，因此無法完全排除遠端讀取與寫入之間的競爭；下一次同步仍會重新比較實際狀態。

遠端未回傳收藏時，不自動刪除本機卡片，也不自動重新建立遠端收藏。本機有遊玩狀態時，使用者可以明確選擇以上傳狀態建立私密收藏。

本機為 `none` 且遠端未收藏時視為一致，不建立收藏；本機為 `none`、遠端有狀態且同步基準為空或 `0` 時，自動採用遠端狀態。若基準與遠端相同而本機清成 `none`，預覽顯示 `cleared`：「本機已清除狀態；Bangumi 無法取消收藏，遠端保留」，可選「改用遠端狀態」或「忽略」。Bangumi API 沒有取消收藏的 DELETE 操作，Galroon 一律拒絕推送 `0`，也不對 `cleared` 進行即時自動重試。兩邊都改變且不同時仍保留衝突。

無資源卡可隱藏並暫停同步。設定中的「顯示全部」可以找回隱藏卡；下次同步不會重複建卡。略過的未關聯作品也會保留略過設定。解除作品關聯會阻止該條目自動匯入，直到手動重新選擇。

之後掃描到本機資源，可用既有的資源關聯或「合併作品」功能掛到已匯入的卡片。若將 Bangumi 卡併入另一張本機卡，Bangumi ID、遊玩狀態與同步基準一起移轉。兩張卡都有 Bangumi 關聯時，先解除其中一張的關聯再合併。

刪除或移除本機資源不影響 Bangumi 收藏。解除帳號綁定保留作品與條目對應，但移除權杖與遠端暫存；重新連接時重新建立同步基準。

## 儲存與 API

Schema 53 將未曾同步成想玩的 `backlog` 作品遷移為 `none`，移除其狀態覆寫並遞增 revision；Bangumi 基準為 `1` 或 VNDB 基準為 `5` 的作品保留 `backlog`。新作品預設為 `none`。

Schema 49 增加 `bangumi_links`（作品對應、同步基準、隱藏狀態）、`bangumi_remote`（遠端快照）、`bangumi_ignored`（略過清單）。帳號設定位於 `settings`，權杖位於執行 Core 的 Windows Credential Manager 或 macOS Keychain；Linux Core（NAS）存成裝置資料夾下 `credentials/` 的擁有者專用檔案（資料夾 0700、檔案 0600），不進入任何備份。

收藏備份不含帳號設定及憑證，還原後不會自動登入或上傳；作品卡與關聯則保留。唯讀瀏覽器不能讀取帳號同步介面，也不能修改同步設定。

- `GET /api/bangumi`：帳號摘要、預覽識別、同步差異；不回傳權杖或憑證參照。
- `POST /api/bangumi/connect`：`{token}`；驗證 `/v0/me` 後保存憑證。
- `POST /api/bangumi/disconnect`：解除連線。
- `POST /api/bangumi/settings`：`{mode: "full" | "existing", automatic}`。
- `POST /api/bangumi/preview`：取得完整分頁收藏，產生同步預覽。
- `POST /api/bangumi/apply`：`{preview, choices: [{subject_id, fingerprint, action, work_id}]}`；action 為 `local`、`remote`、`import`、`match`、`skip` 或 `same`。每筆獨立保留成功結果，遇到失敗停止，其餘保持待處理。
- `POST /api/bangumi/link`：`{work_id, revision, subject}`；空 subject 解除關聯。
- `POST /api/bangumi/hide`：`{work_id, hidden}`。

自動同步由 Core 的 Services 管理，與前端視窗無關。API 同步操作及背景同步共用同一個鎖；本機作品編輯仍可繼續，同步期間的後續修改會保留為下一次待同步狀態。

## 驗證範圍

Rust 測試以本機模擬 Bangumi HTTP 服務驗證雙向往返、衝突、分頁、預覽失效、同步途中本機編輯、部分失敗、無資源卡、隱藏、合併與備份遷移。前端測試涵蓋雙向選項、首次與衝突預設不覆蓋、隱藏恢復與繁體中文；瀏覽器驗收使用產生的 Bangumi 回應及隔離 Core。這些測試不使用真實帳號，不代表真實 Bangumi 授權或 Windows 憑證儲存的端到端驗收。

官方契約：https://github.com/bangumi/api/blob/master/open-api/v0.yaml
