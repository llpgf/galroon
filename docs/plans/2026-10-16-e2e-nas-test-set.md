# 端到端驗收改用 NAS 測試集

> 2026-10-16。原本的外接測試碟（`/Volumes/新增磁碟區/sandbox_data`，31 項）已停用。
> Playwright 的 31 條故事與 golden 答案卷都綁在那份資料上，現在無法執行。本文規劃改用從 NAS 收藏挑出的固定測試集。

## 現況

- 單元層在 master `23935d7` 全綠：Rust Core＋MCP 517 passed，前端 204/204。
- 真實收藏的錯配檢查改由 `match_replay` 承擔（見文末「常設檢查」）：2026-10-16 對 NAS 收藏（793 項資源）跑出錯配 0。
- golden 的 31 個資料夾中，有 18 個在 NAS 的 `/volume1/Backup/Galgame` 找得到，13 個不在。
  - 找不到的案例：
    - 純數字商品碼資料夾 5 個（1261651、1327292、1327332、1337486、1369164）。1327332 正是「append 只是本篇的一個 release → 配本篇」的案例。
    - remake 1 個（1334584）。
    - DLsite `VJ` 碼 1 個。
    - 舊作短標題 1 個（牝教師4）。
    - 長副標題 1 個（kelp）。
    - 特典尾綴 3 個（CRYSTALiA、CloverGAME 飛べない蝶、マガルミナ）。
    - 重複副本 1 個（ママ×カノEX 的 mdf 版，與 NAS 上的 Vocal CD 版組成「一張卡、一個版本」案例）。

## 目標

- 31 條故事全部回到 M 層可執行。
- 答案卷由我（owner）逐筆審核，錯配 = 0 仍是硬性條件。
- 不再依賴外接碟，也不需要掛載 NAS 共用資料夾。

## 做法

### 1. 測試集組成（38 個頂層資料夾，77.7 GB）

| 來源 | 數量 | 說明 |
|---|---|---|
| 沿用 golden、NAS 上找得到的 | 18 | 答案不變 |
| 替代找不到的 13 個 | 13 | 從 NAS 挑同類案例：純數字碼、VJ 碼、remake（同級生2リメイク已確認有獨立 2024 卡）、重複副本（例如 `[210924][1143704]` 人妻天国 在 NAS 上有兩份）、短標題、長副標題、特典尾綴、append 屬於本篇 release |
| 10-11 之後新增的規則 | 4–6 | 純合集綁到每部作品（キスイロ、アリス2010）、合集以其中一作命名（君が望む永遠 EER）、Chinese/CHS 子資料夾（夜のひつじ/…）、只有補丁的資源保持不處理 |

逐筆清單與依據見 [NAS 測試集候選清單與答案卷](2026-10-16-nas-testset-candidates.md)。2026-10-16 owner 已逐筆確認，答案卷 `tests/e2e/golden/nas_testset.json` 全部為 `source: user`。

### 2. 資料放哪裡

把挑出的資料夾用 `rsync` 經 SSH 從 NAS 複製到 Mac 內建磁碟：`~/GalroonTestData/testset-2026-10/`。

- NAS 端是唯讀讀取，不會改動收藏。
- 複製完設為唯讀；G3「不動檔案」照舊用前後指紋比對。
- 2026-10-16 已複製：124 個檔案、83,427,574,020 bytes，與 NAS 清單逐檔大小一致，抽樣 5 檔 SHA-256 相同；已設唯讀，指紋存於 `~/GalroonTestData/testset-2026-10.fingerprint.tsv`。

替代方案（不建議）：以 SMB 掛載整個共用資料夾並用 `sandbox-exec` 禁寫。缺點是：

- 每次都要先掛載。
- macOS SMB 對 NFD 檔名會間歇失敗。
- 掃描整個 793 項資源太慢，也不是固定集合。

### 3. 程式與文件改動（交 Codex 實作，我審查與執行）

- `tests/e2e/support/core.ts`：`SANDBOX` 改讀必填的 `GALROON_TESTSET`。缺少時直接以清楚訊息失敗，不再預設外接碟路徑。
- 新答案卷 `tests/e2e/golden/nas_testset.json`，格式同舊檔，另加 `works`（合集綁到每部作品）。每筆標 `source`，只有 `user` 才算定稿。
- 各 spec 中寫死的資料夾名稱與數量（例如 31 項、28 配）改從答案卷讀取。
- `docs/acceptance/USER_STORIES.md`：更新 M 層的資料說明與 golden 段落，舊答案卷保留為歷史紀錄。
- 加一支 `scripts/testset/sync.sh`：依清單從 NAS `rsync`、設唯讀、輸出指紋。

### 4. 驗收

- 新測試集上連續跑兩次完整 Playwright：31/31 綠，golden 全部吻合或屬於允許的「未配」，錯配 0。
- 結果記錄在 `docs/acceptance/`。

## 常設檢查：真實收藏的錯配

每次發版前，以及匹配規則變更之後，都對 NAS catalog 跑一次。步驟如下：

1. 在 NAS 上用 `sqlite3 ... ".backup"` 取一致副本。這是線上備份，Core 不必停機。
2. 執行 `match_replay <copy> --answers ~/GalroonTestData/nas-answers.json --fail-on false`。

2026-10-16 結果（algorithm 15）：

| 層 | 吻合 | 錯配 | 未配（有答案） | 無答案 |
|---|---|---|---|---|
| NAS 實際綁定 | 554 | **0** | 42 | 89 |
| master 重放 | 556 | **0** | 40 | 89 |

- 限制 1：答案卷 596 筆中只有 35 筆是你審過的，其餘 561 筆來自 10-11 的自動結果。
- 限制 2：89 筆「無答案」我已對 VNDB 標題逐筆看過，沒發現錯誤，但仍需你確認。
- 審核表：`~/GalroonTestData/replay-2026-10-16/unreviewed-89.csv`。確認後併入答案卷，標為 `user`。

## 已決定（2026-10-16）

1. 規模：38 個資料夾（每次完整跑預計 15–25 分鐘）。
2. 複本位置：Mac 內建磁碟 `~/GalroonTestData/testset-2026-10/`。
3. 89 筆無答案的匹配與 561 筆自動答案的複核：另案處理，不阻擋本計畫。
