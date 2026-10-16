# 內建自動備份與首次執行還原（Roon 式）

> 2026-10-16。Claude 規劃、Codex 實作；每個 PR 由 Claude 測試後開出，擁有者說「合」才合併。
> PR #76 的 NAS 腳本（`scripts/nas/galroon-backup.sh`＋DSM 工作排程器＋Mac launchd 拉回）是過渡方案。本文規劃把同等能力放進 Core：在 Backup 面板設定排程，Core 自己備份與清理；新 Core 第一次開啟時就能選「從備份還原」。

**關鍵假設**（不對請直接更正）：排程只需要「每日／每週＋時間＋保留 N 份」，不需要 cron 語法；完整備份（含憑證與身分）只給災難復原用，不做雲端或加密上傳；NAS 上 Core 只看得到 container 掛載的路徑，所以備份目的地需要新增一個掛載點。

## 需要擁有者決定的事項

每項都附建議。未回覆時 Codex 依「建議」實作。

| # | 決定 | 選項 | 建議 |
|---|---|---|---|
| D1 | 手動匯出（例如目的地填 `/data/backups` 的 15 份）是否套用排程的保留數 | (a) 不套用，手動的永遠由人刪除；(b) 套用同一個 N | **(a)**。自動清理只碰 Core 排程自己建立、manifest 標記 `origin:"scheduled"` 的資料夾。面板顯示每份手動匯出的大小，並提供逐份刪除。現有 4.9 GB 舊匯出用 PR #76 的 `prune-core-exports` 一次清掉 |
| D2 | 保留策略 | (a) Roon 式單一「保留 N 份」；(b) 沿用腳本的 7 daily＋4 weekly | **(a)**，預設每日 04:30、保留 14 份（含圖片約 430 MB × 14 ≈ 6 GB，NAS 剩約 14 TB）。(b) 的 GFS 可之後再加，不影響格式 |
| D3 | 排程備份預設模式 | 完整（full）或可攜（portable） | **排程預設 full**（目的就是災難復原），**手動匯出預設 portable**（可能拿去分享或除錯）。兩處都可切換 |
| D4 | full 備份是否加密 | (a) 不加密，靠檔案權限＋警告；(b) 用通關密語加密身分與憑證 | **(a) 第一版**。與目前腳本快照的保護等級相同。若之後要放雲端或異地硬碟，另開 PR 加密 `identity/` 與資料庫（見「安全取捨」） |
| D5 | NAS 備份目的地 | 新增 compose 掛載 `GALROON_BACKUPS` → container `/backups` | **建議 host 路徑 `/volume1/docker/galroon/core-backups`**（與腳本的 `backups/` 分開，並行期間不互相干擾）。同一 volume 無法防 volume 損毀，異地仍靠 Mac 拉回或 Hyper Backup |
| D6 | 舊腳本去留 | 立即移除／並行後退役／永久保留 | **並行 2 週後停用 DSM 排程**；Mac 拉回改抓 Core 格式；`galroon-backup.sh` 保留一個發行週期當緊急工具，之後刪除 |
| D7 | 首次執行還原是否也接受 PR #76 腳本快照格式 | 接受（加轉接層）／不接受 | **不接受**。腳本快照在並行期內自然輪替掉；這段期間要還原舊快照，照 NAS.md 用腳本還原 |
| D8 | NAS 還原後切換到新資料目錄的方式 | (a) Core 程序內重新啟動；(b) Core 結束，靠 Docker `restart: unless-stopped` 重啟 | **(a)**。不依賴 Docker 重啟政策，Mac／Linux 單機版也適用；(b) 留作失敗時的備案 |
| D9 | 還原 full 備份時，原 Core 可能還在 LAN 上 | 只警告／要求勾選「原 Core 已關閉」 | **要求勾選**。兩個相同 SPKI 的 Core 同時在線，已配對桌面會連到哪一台不確定 |

## 現況

- `backup::export`（`crates/core/src/backup.rs:43`）：以 SQLite backup API 取得 `library.sqlite` 一致快照，`clean_credentials` 刪除 `sessions`、`pairing_codes`、`access_attempts`，以及 `schema_version`、`organization.managed_root` 以外的**所有** `settings`（包含 `owner_password`）。另外快照 `artwork-library.sqlite`，寫 manifest v2／v3，最後記入 `backup_history`。不含 `device.json`、`network.json`、`lan-*.pem`。
- `inspect` 拒絕 `includes_credentials:true` 的 manifest；`restore_new` 只寫入**尚不存在**的目錄，會再清一次憑證、把 jobs 標成 interrupted、計畫退回 ready、watcher 全部停用。
- 桌面版還原（`apps/desktop/src-tauri/src/controller.rs:68`）：還原到 `<base>/collections/<id>`，先停舊 Core，替舊收藏做保護快照（`.galroon-restore-snapshots`），再原子改寫 `<base>/active-library.json`。Core 本身不認得這個指標，由桌面 controller 決定 `--data-dir`。
- NAS Core（`docker/Dockerfile`）：`serve --data-dir /data`，`device_dir` 預設等於 data dir，所以身分檔在 `/data` 根目錄。沒有 Core 層級的 `restore`（LAN 計畫 N1d 尚未做）。
- 裝置身分：`device.json`（`context.rs:28`）首次啟動自動產生；`lan_tls.rs` 的金鑰與憑證「只產生、不覆寫」，一個缺檔就拒絕啟動。已配對桌面釘選 SPKI 指紋，bearer token 的雜湊存在收藏資料庫的 `sessions`。
- 所以「還原後不必重新配對」需要**兩者都保留**：身分檔（SPKI 不變），以及 `sessions`＋`owner_password`（token 仍有效）。LAN 計畫 §4.10「還原會讓所有已配對裝置失效」只對 portable 模式成立。
- `BackupPanel.tsx`：只有手動匯出、歷史清單、桌面版還原。資料夾選擇器選的是**客戶端**路徑（LAN 計畫 R4），遠端連線時不能用。
- Core 背景工作由 `Services`（`services.rs`）統一啟停；`updates.rs` 是「device_dir 下 JSON 狀態＋定時 worker＋可隨時停止」的現成範本。

## 設計

### 1. 備份模式與格式（manifest v4）

`export(db, destination, Options{mode, origin})`：

- `mode:"portable"`：與現在完全相同，仍呼叫 `clean_credentials`。
- `mode:"full"`：保留 `owner_password`、`sessions` 與其他 `settings`；仍刪除 `pairing_codes`（5 分鐘就過期）與 `access_attempts`。另把 `device_dir` 的 `device.json`、`network.json`（存在時）、`lan-key.pem`、`lan-cert.pem` 複製到 `identity/`。`lan-tls.lock`、`device.lock` 不複製。金鑰缺一個就整次失敗，不產生「半套身分」。
- `origin:"manual"|"scheduled"`，寫進 manifest，也寫進 `backup_history` 新欄位 `origin`、`mode`（schema 51）。

manifest v4 新增欄位：`mode`、`origin`、`includes_credentials`（full 為 true）、`identity:{files:[{name,bytes,sha256}], spki_fingerprint, device_id}`、`core_version`。資料夾另寫一份 `SHA256SUMS`（`sha256sum -c` 格式），讓 Mac 拉回與外部工具不必理解 manifest。

`inspect` 接受 v1–v4。v4 full 必須逐檔核對 `identity/` 的大小與雜湊，並確認 `lan-cert.pem` 的 SPKI 等於 manifest 記錄值、金鑰與憑證成對（沿用 `LanTls::load_or_generate` 的解析）。`includes_credentials:true` 只在 `mode:"full"` 時允許。舊版 Core 讀到 v4 會拒絕，這是預期行為。

目錄與檔案權限：Unix 上資料夾 0700、檔案 0600（`identity/lan-key.pem` 一律 0600）。Synology ACL 可能放寬，文件寫明。

### 2. Core 排程備份

新模組 `crates/core/src/backup_schedule.rs`。

**設定與狀態**放在 `device_dir/backup-schedule.json`（跟 `update-check.json` 同一套原子寫入、拒絕符號連結、損毀視同不存在並記 diagnostics）：

```json
{"version":1,"enabled":true,"destination":"/backups","frequency":"daily","minute_utc":1230,
 "weekday":null,"keep":14,"mode":"full","artwork":true,
 "last_started":null,"last_success":null,"last_error":null,"last_path":null}
```

- 理由：目的地是這台機器的路徑，跟著裝置走；portable 還原本來就清空 `settings`，放資料庫會在還原後失效或指到不存在的路徑。這份檔案**不放進 full 備份**；full 還原後面板提示「自動備份尚未設定」，並預填備份 manifest 記錄的原設定供確認。
- `minute_utc` 以 UTC 分鐘儲存，UI 用瀏覽器時區換算並同時顯示兩者（台灣無日光節約，不另處理時區資料庫）。`frequency:"weekly"` 時 `weekday` 為 0–6。
- `keep` 為 1–100 的整數。

**Worker**（加入 `Services`，可立即停止）：

- 每分鐘檢查一次是否到期。Core 錯過排程（關機、更新）時，啟動 5 分鐘後若 `last_success` 已超過一個週期就補跑一次，不連補多次。
- 執行前檢查：目的地存在、可寫、不在 data dir 或任何收藏來源內（避免掃描到；已有 `Galroon-backup-*` 保留字首）；可用空間至少為（目前收藏資料庫＋圖片庫）× 1.2 ＋ 1 GiB，否則整次失敗，不會悄悄略過圖片。
- 同一時間只允許一個備份（手動或排程）；有 plan 正在 executing 時延後，每 10 分鐘重試，最多延後 6 小時，之後照跑（SQLite backup API 本來就一致）。
- 成功後才清理：掃描目的地第一層名稱符合 `Galroon-backup-*`、不是符號連結、manifest 可解析、`origin:"scheduled"` 且 `library.id` 等於目前收藏的資料夾，依 `created` 保留最新 `keep` 份，其餘刪除。**不依賴 `backup_history`**，因為還原後的資料庫歷史可能較舊。其他收藏、手動匯出、無法解析的資料夾一律不動。
- 失敗寫入 `last_error` 與 diagnostics；連續失敗不會比每小時一次更頻繁地重試。

**API**（全部只限 owner／desktop，web 拒絕；寫入 `docs/development/API_CONTRACT.md`）：

- `GET /api/backups/schedule`：設定＋狀態＋`running`＋下次執行時間＋目的地可用空間。
- `POST /api/backups/schedule`：更新設定（`deny_unknown_fields`），儲存前驗證目的地。
- `POST /api/backups/run`：立即依排程設定執行一次（算 `origin:"scheduled"`，參與清理）。
- `POST /api/backups` 新增選填 `mode`（預設 portable，維持相容）。
- `DELETE /api/backups/{id}`：只刪 `origin:"manual"` 且資料夾仍符合上述檢查的那份，並把歷史標記為已刪除（解決 NAS.md 提到「刪了資料夾、面板仍列出」）。
- 選擇 full 模式需要 owner 權限；desktop 角色只能 portable（full 備份可以冒充這台 Core，見安全取捨）。

**UI**（`BackupPanel.tsx` 拆出 `BackupSchedule.tsx`）：

- 「自動備份」區塊：開關、目的地（Core 端路徑文字欄；只有本機桌面才顯示「瀏覽」）、每日／每週、時間、保留份數、模式、是否含圖片、「立即備份」。顯示上次成功時間、上次錯誤、下次時間、可用空間。
- full 模式旁固定顯示警告：「此備份含 owner 密碼雜湊、已配對裝置憑證與 LAN 私鑰，拿到的人可以冒充這台 Core。」
- 歷史清單標示「自動／手動」「完整／可攜」，手動的可刪除。
- 面板頂端：啟用中但超過兩個週期沒有成功備份時顯示警示；首頁不加通知（之後可沿用 UpdateBanner 的位置）。
- 所有字串加進 `src/locales/zh-TW.ts` 與 `i18n.ts`，390 px 不可水平溢出。

### 3. 首次執行「從備份還原」

**何謂新 Core**：`owner_password` 不存在、`roots`／`works`／`sessions` 皆為 0。只有這種狀態才提供還原；有資料的 Core 一律回 409，沿用桌面版既有流程或 N1d 離線還原。

**入口**：

- UI：`/api/access/status` 回報 `configured:false` 且呼叫者是 owner 連線（loopback 的 app token；NAS 上就是 `owner.sh session`＋SSH 通道）時，顯示首次執行畫面，兩個選項：「建立新收藏」（現有 owner 設定）與「從備份還原」。LAN 上未登入的瀏覽器看不到也呼叫不了。
- CLI：`galroon-core owner restore --from <container 內路徑> [--confirm-original-offline]`，經控制通道呼叫同一段程式，給沒有 UI 的情況。

**API**（只限 owner transport，且僅在新 Core 狀態）：

- `GET /api/setup/restore/candidates`：列出排程目的地與 `/backups`（若存在）第一層的備份資料夾，附 manifest 摘要。路徑一律是 Core 端路徑，另可手動輸入。
- `POST /api/setup/restore/inspect {path}`：回 manifest、模式、作品／資源數、schema、SPKI 指紋。
- `POST /api/setup/restore {path, sha256, original_offline:bool}`：full 備份要求 `original_offline:true`（D9）。回 202，之後 Core 重新啟動。

**流程**（以 NAS 為例，`data root = /data`）：

1. `restore_new(folder, /data/collections/<id>)`。full 模式改用新的 `restore_new_full`：不呼叫 `clean_credentials`，其餘（jobs interrupted、計畫退回、watcher 停用）照舊。
2. full 模式還原身分：把新 Core 首次啟動產生的 `/data/device.json`、`lan-*.pem`、`network.json` **移到** `/data/replaced-identity-<UTC>/`（不刪除），再從備份 `identity/` 以 `create_new`＋`sync_all` 寫入，權限 0600。
3. `network.json` 的位址若不屬於這台主機的任何介面（換硬體、換 IP），寫入時設 `enabled:false`，並在 UI 提示「更新 LAN 位址後再開啟」。否則 `configured_listeners` 綁定失敗會讓整個 Core 起不來。已配對桌面之後可用既有的「更新端點」功能改連新 IP，因為 SPKI 不變，不必重新配對。
4. 原子寫入 `/data/active-library.json`（與桌面版同格式：`current`、`previous[]`），`previous` 記錄被取代的空收藏。
5. Core 程序內重新啟動（D8）：`unix_core::run` 外面加一層迴圈，收到 relaunch 請求時照正常停止流程（暫停 jobs、HTTP drain、checkpoint），關閉控制通道，重新解析指標，以新收藏目錄 `CoreHost::initialize`，重新綁定 loopback／LAN 與控制通道。失敗則退回 `previous` 並記 diagnostics，Core 仍以空收藏啟動，不會兩邊都壞。
6. UI 在等待期間輪詢 `/api/health`；loopback token 是新的 app token，所以 NAS 上要重新 `owner.sh session`（畫面明確說明）。portable 還原後回到 owner 設定畫面；full 還原後直接可用，已配對桌面照常連線。

**Core 層級的指標**：新模組 `active_collection.rs`，提供 `resolve(data_root)`（沒有指標就回 data root 本身，所以現有 NAS 的 `/data/library.sqlite` 不受影響）與 `commit(data_root, new, previous)`。`serve --data-dir` 改為 data root，收藏目錄由指標決定，`device_dir` 預設仍是 data root。桌面 controller 改呼叫同一個模組，行為不變（同格式、同 `collections/` 位置）。

**桌面版範圍**：Windows 本機 Core 的首次執行還原沿用 `controller.restore`（portable）。桌面版 full 身分還原延後，因為 Windows 本機 Core 很少開 LAN，且需要另外處理 controller 的 Core 生命週期。

### 4. 與 PR #76 腳本並存與遷移

| 項目 | 腳本（現在） | 內建（之後） |
|---|---|---|
| 排程 | DSM 工作排程器 04:30 | Core worker，Backup 面板設定 |
| 內容 | `.backup` 快照＋身分檔，MANIFEST／SHA256SUMS | manifest v4 full＋`identity/`＋SHA256SUMS |
| 保留 | 7 daily＋4 weekly | 保留 N 份（D2） |
| 失敗通知 | DSM 異常時寄信 | 面板警示＋diagnostics（無寄信） |
| 還原測試 | `restore-test`（`--network none` container） | `restore-test` 改吃 Core 格式，呼叫 `galroon-core restore` |
| 異地 | Mac launchd 06:00 拉回 | 同一個 launchd，改拉 Core 格式 |

遷移步驟（B6 與實機驗收時執行）：

1. 部署含 B1–B3 的 image，compose 加 `/backups` 掛載（D5），面板設定 full、每日 04:00、保留 14。腳本照舊 04:30 執行，兩者錯開。
2. 並行 2 週：每天確認兩邊都成功；用新版 `restore-test` 對 Core 格式最新備份測一次，核對作品／資源數與 SPKI 指紋。
3. 停用 DSM「Galroon backup」排程（文件寫步驟，由擁有者在 DSM 操作）；`backup.sh pull` 改抓 `/volume1/docker/galroon/core-backups` 最新 `origin:"scheduled"` 資料夾並以 SHA256SUMS 驗證；重新執行 `install-mac-schedule`。
4. 腳本快照目錄 `/volume1/docker/galroon/backups` 保留到最後一份 weekly 滿 4 週後，由擁有者手動刪除。
5. `/data/backups` 的 15 份手動匯出（4.9 GB）：用既有 `prune-core-exports --keep 3`，或在新面板逐份刪除。
6. 下一個發行週期：`galroon-backup.sh` 從 repo 移除，NAS.md「自動備份與還原」改寫成以 Core 為主，腳本段落刪除。

**DSM 寄信失去的部分**：內建版沒有郵件。若擁有者需要，可在 DSM 保留一個每日任務執行 `scripts/nas/status.sh` 類的檢查（查 `/api/backups/schedule` 的 `last_success`），失敗時由 DSM 寄信。列為可選，不在本計畫 PR 內。

## 安全取捨（full 模式）

full 備份包含：

- `lan-key.pem`：拿到的人可以架一台 SPKI 相同的假 Core，已配對桌面會信任它（中間人或冒充）。
- `owner_password` 的 Argon2 雜湊：可離線暴力破解，密碼至少 12 字元能拖慢但不能阻止。
- `sessions` 的 token SHA-256 雜湊：token 是隨機值，雜湊本身不能用來登入；但還原後這些 token 重新有效，所以**洩漏的舊 token 在還原後仍可用**，直到撤銷或過期。

對策（第一版）：

- 排程 full 只限 owner 設定；desktop 角色只能產生 portable。
- 檔案權限 0700／0600；UI 與文件明確警告不要放在 SMB 分享給其他帳號的資料夾、不要上傳雲端。
- 目的地不可以位於收藏來源內（收藏共用資料夾常被其他裝置讀取）。
- 還原 full 時要求確認原 Core 已關閉（D9）。
- 還原後在「已登入裝置」清單標示「從備份還原」，方便擁有者撤銷不認得的裝置。

不加密的理由：與目前腳本快照、NAS 上 data 目錄本身的保護等級相同；加密需要擁有者保管另一組通關密語，忘了就無法還原，對單人家用 LAN 的風險收益不划算。若 D4 選 (b)，另開 PR：`identity/` 與資料庫以通關密語（Argon2id 衍生金鑰）加密成 `secrets.age` 類檔案，還原時輸入。

portable 模式完全維持現狀，作為分享、除錯、跨機搬移收藏的格式。

## PR 拆分

依序執行；每個 PR 一個 Codex 任務，Claude 測試後開 PR。

| PR | 內容 | 依賴 | 主要檔案 |
|---|---|---|---|
| **B1** Core：備份模式與 manifest v4 | `export` 的 `mode`／`origin`、`identity/`、SHA256SUMS、`inspect` v4 驗證、`restore_new_full`（還原到新目錄＋寫出身分檔，但尚不切換）、schema 51 `backup_history.origin/mode`、`POST /api/backups` 的 `mode`、full 限 owner | — | `backup.rs`、`db.rs`、`lib.rs`、`API_CONTRACT.md` |
| **B2** Core：排程 worker | `backup_schedule.rs`、`Services` 接入、schedule／run／delete API、空間檢查、只清自己建立的資料夾 | B1 | `backup_schedule.rs`、`services.rs`、`lib.rs` |
| **B3** UI：自動備份面板 | 排程設定、狀態、模式警告、歷史標示與刪除、Core 端路徑輸入、locales | B2 | `BackupPanel.tsx`、`BackupSchedule.tsx`、`src/locales/zh-TW.ts`、`i18n.ts` |
| **B4** Core：首次執行還原 | `active_collection.rs`（桌面 controller 改用它）、新 Core 判定、setup/restore API、身分檔替換、`network.json` 位址檢查、`unix_core` 程序內重新啟動、`owner restore` CLI；順便提供 N1d 的離線 `galroon-core restore --from --to`（同一段程式） | B1 | `active_collection.rs`、`unix_core.rs`、`owner_cli.rs`、`access.rs`、`controller.rs` |
| **B5** UI：首次執行畫面 | 「建立新收藏／從備份還原」、候選清單、預覽、原 Core 已關閉確認、重新啟動等待與重新連線說明 | B4 | 新 `FirstRun.tsx`、`main.tsx`、locales |
| **B6** NAS 部署與遷移 | compose `GALROON_BACKUPS` 掛載、`deploy.sh` 傳遞、`restore-test`／`pull` 改 Core 格式、NAS.md 改寫、腳本標示淘汰；接著在實機執行遷移步驟 1–3 | B2（restore-test 需 B4） | `docker/compose.yaml`、`scripts/nas/*`、`docs/development/NAS.md` |

**STOP 關卡**：B1 合併後先停，Claude 在 NAS 用 owner session 手動產生一份 full 備份，量測時間與大小，並把它還原到隔離目錄（`--network none`）確認 SPKI 與 bearer 有效，結果回報擁有者後才開始 B2。B4 是唯一會改 Core 啟動方式的 PR，合併前必須在 NAS 上跑一次完整首次還原演練（見驗收）。

B3 可與 B4 平行；B5 等 B4；B6 的部署部分等 B2，`restore-test` 改寫等 B4。

## 測試與驗收

Core（Mac：`cargo test -p galroon-core --lib --bins --tests`）：

- portable 匯出的位元組內仍找不到密碼雜湊、session 雜湊與私鑰（沿用現有測試）；full 匯出找得到 session，但沒有 `pairing_codes`。
- full 還原到新目錄並啟動後：`spki_fingerprint` 與原 Core 相同；還原前發出的 desktop bearer 在還原後的 Core 仍可呼叫 `/api/access/me`；owner 密碼仍可登入；watcher 停用、jobs interrupted。
- 竄改 `identity/` 任一檔、只留金鑰或只留憑證、manifest SPKI 與憑證不符 → `inspect` 失敗。
- 排程：到期執行、錯過補跑一次、空間不足失敗且不寫半套、keep=3 時只刪自己建立且同 library 的舊資料夾，手動匯出、其他收藏、符號連結、無法解析的資料夾都不動；停止要求能中斷等待。
- web session 對所有新 POST／DELETE 回 403；desktop 角色要求 full 回 403。
- 首次還原：有任何作品／root／owner 的 Core 回 409；LAN transport 呼叫回 403；`network.json` 位址不在本機時以 `enabled:false` 寫入且 Core 正常啟動；重新啟動失敗時退回空收藏。
- `active_collection`：沒有指標時回 data root；指標指到 data root 外或符號連結時拒絕；桌面 controller 現有測試全數通過。

前端：vitest 覆蓋排程表單時區換算、警告顯示、首次執行判斷；Playwright／截圖確認 390 px 無水平溢出。

NAS 實機（B1 STOP 與 B6 後）：

- 實際收藏大小（約 72 MB＋358 MB）的排程 full 備份耗時、對正式 Core 回應的影響（備份期間 `/api/health` 延遲）。
- 首次還原演練：用正式 image 啟動一個全新、`--network none` 的測試 container，掛載備份，透過控制通道 `owner restore` 還原，確認重新啟動後作品／資源數、SPKI 指紋、owner 登入、已配對桌面的 bearer（用 loopback 呼叫）都正確。正式 Core 全程不停機。
- 並行 2 週的每日成功紀錄。

## 不在本計畫內

- 備份加密（D4 選 (b) 時另開 PR）、雲端上傳、郵件通知。
- 有資料的 Core 線上替換收藏（Roon 的「設定中還原」）；目前仍用桌面 controller 或 N1d 離線還原。
- Windows 桌面版的 full 身分還原。
- GFS 多層保留（D2 選 (b) 時再加）。
- 遊戲檔案、存檔備份（`includes_game_files` 永遠為 false）。
