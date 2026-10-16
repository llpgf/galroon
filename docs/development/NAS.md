# NAS 部署（PR N1b）

適用於 Synology DS920+（J4125、x86_64、DSM Container Manager）。本 PR 僅提供 N1 的部署工具；沿用 N1a 的 Linux CLI、Unix socket 與固定 `127.0.0.1:14800`。完整 N1 驗收還需要 **N1c** 的掃描／封存／watcher 相容性與**匹配計畫 Phase 1b**。此部署工具不提供 LAN、TLS 或 N3 搬移；Core 層級的 N1d `restore` 命令仍未提供，但可使用下方的 NAS 快照還原。

## 為何分開建置

相對於 LAN_CORE_PLAN §4.15，本 PR 刻意作以下調整：

- GitHub Actions `ubuntu-24.04` 建置 Linux x86_64 Core 與前端，產出一個 `galroon-nas-x86_64` artifact。NAS 可用記憶體約 1.3 GB，在 NAS 上執行 rustc 容易 OOM 且速度慢；Mac 也沒有 Docker，因此不採多階段 image 編譯或 Mac `docker save`／NAS `docker load`。
- NAS 只以預先建好的檔案組裝 runtime image。使用 `ubuntu:24.04`，與 CI 的 glibc 世代一致，取代計畫的 debian-slim。安裝 `7zip`（7-Zip 23.01，執行檔為 `/usr/bin/7z`）與 CA 憑證，設定 `GALROON_7ZIP`。Core 與 `web/` 放在 `/opt/galroon/`，符合 `lib.rs` 的 `ServeDir` 路徑。
- 不發布或拉取 Galroon registry image；image 只在 NAS 上建置及保留。首次建置仍需取得指定的 Ubuntu 基底 image 與 Ubuntu 套件。
- 只掛載實際需要的 `/volume1/Backup/Galgame`，保留 NAS 原路徑與唯讀限制，不掛載整個 Backup。
- uid/gid 由 compose 注入，不寫死在 image；部署預設 `1032:100`。使用絕對 executable 路徑執行 owner，以符合 N1a 與 image 的 PATH。

N1a 已在既有 `ci.yml` 提供 Linux build／Core lib tests，本 workflow 專責 release artifact。Rust 程式碼不需修改。

## 前置條件與目錄

Mac 需要 Git、Bash、tar、OpenSSH、已登入的 GitHub CLI (`gh auth login`)、`shasum`。前端開發 UI 另需 Node.js 22 與 npm。所有腳本可以從任何工作目錄呼叫，定位到它們所在的 checkout。

NAS 需要 Container Manager 的 `/usr/local/bin/docker` 與 `docker compose` plugin。`galroon-dev` 的 SSH key 登入與 `sudo -n /usr/local/bin/docker ...` 必須可用；腳本不使用 sudo 執行 mkdir、chown、cat 或其他命令，也不會要求／儲存 sudo 密碼。sudo 規則由 NAS 管理員事先設定。

`galroon-dev`（預期 `id -u = 1032`、`id -g = 100`）需可在 `/volume1/docker` 建立自己的 `galroon` 目錄，並對收藏擁有讀取及目錄穿越權限。資料目錄由此帳號 `mkdir -p`，採 `umask 077`；既有目錄的 uid/gid 不符或不可寫時直接停止，由管理員透過 DSM 修正擁有者／ACL，再重試，不以 sudo chown 修補。

| NAS 路徑 | 用途 |
| --- | --- |
| `/volume1/docker/galroon/data` | 持久資料、裝置身分、資料庫 |
| `/run/galroon/control`（container 內 tmpfs 的子目錄） | owner 控制通道 `core.sock` |
| `/volume1/docker/galroon/compose.yaml`、`.env` | 最近一次部署的 compose 與非機密設定 |
| `/volume1/docker/galroon/release.*` | 每次上傳的獨立 build context，保留供檢查 |
| `/volume1/docker/galroon/previous-image-id` | 更新前 container 的 image ID（若存在） |
| `/volume1/Backup/Galgame` | 唯讀收藏，同路徑掛入 container |

`/data` 必須位於 NAS 本機磁碟；N1a 拒絕 NFS、CIFS/SMB、FUSE。控制通道放在 container 內 tmpfs 的子目錄 `/run/galroon/control`，由 Core 啟動時自行以 0700 建立（Docker 24 在 container 重新啟動時不會重新套用 tmpfs 的 `mode=0700`，實測會變成 755），socket 為 0600。不放在 `/data` 底下，是因為 Synology 共用資料夾的 ACL 會讓目錄權限固定為 777（第一次實機部署時因此重啟迴圈），Core 與 owner CLI 皆用相同非 root uid，雙向驗證 peer uid。此目錄不可分享給其他 container；信任能存取 socket 的同 uid 程序，並不防護 NAS root 或 Docker 管理員。

Dockerfile 未設定固定 `USER`，因此**一律透過 compose 執行**；若手動 `docker run` 必須自行指定非 root `--user`。部署腳本拒絕 root uid/gid。

## 建置、下載與部署

1. 將 PR 推到 GitHub。修改 `docker/**`、`scripts/nas/**` 或 workflow 本身時會啟動 **NAS artifact**；也可以使用 Actions 的 `workflow_dispatch` 選定分支。透過 CLI 手動啟動（workflow 必須已能被 GitHub 識別，通常需先存在預設分支）：

   ```bash
   gh workflow run nas-image.yml --ref lan/n1b-docker
   gh run list --workflow nas-image.yml --branch lan/n1b-docker
   gh run watch <run-id> --exit-status
   ```

   workflow 在 `ubuntu-24.04` 依序執行 `cargo build --release -p galroon-core --bin galroon-core`、`npm ci && npm run build`。artifact 包含 `galroon-core`、`web/`、`BUILD_SHA`、`SHA256SUMS`；保留 30 天。PR run 預設建置 merge commit，`BUILD_SHA` 記錄**實際 checkout commit**，不是假設等於本機 HEAD。欲精確比較同一 commit，優先使用明確 ref 的手動 run。

2. 在此 checkout 下載成功的 run：

   ```bash
   scripts/nas/fetch-artifact.sh <run-id>
   # 或選該分支最新成功的 NAS artifact run：
   scripts/nas/fetch-artifact.sh lan/n1b-docker
   cat target/nas/BUILD_SHA
   ```

   run ID 最明確；分支模式可能選到較早成功的 run，必須核對 SHA。workflow 尚未在預設分支註冊時，可直接使用 PR 的 run ID。腳本以 `gh run download` 下載，檢查內容與 checksum，成功後才置換 `target/nas/`；前一份下載保留在 `target/nas-fetch.*.previous`。checksum 用來偵測損毀，不取代對來源 repo／workflow 的信任。

3. 確認 NAS 帳號與權限，首次部署使用全新資料目錄。以下為預設值；需要自訂時，在**每次部署**的 shell 匯出相同設定：

   ```bash
   export GALROON_NAS=galroon-dev@192.168.50.154
   export GALROON_UID=1032 GALROON_GID=100
   export GALROON_DATA=/volume1/docker/galroon/data
   export GALROON_LIBRARY=/volume1/Backup/Galgame
   scripts/nas/deploy.sh
   scripts/nas/status.sh
   ```

   `GALROON_NAS` 適用於全部 SSH 腳本；使用者若改帳號，uid/gid 必須與 SSH 帳號相符。`GALROON_DATA` 限制在 `/volume1/docker/galroon/` 下，禁止 `..` 與 symlink；路徑允許空白，但不接受 compose 分隔符或 shell 插值字元。收藏必須事先存在，腳本不建立或修改收藏。自訂資料目錄不可與收藏目錄重疊。

   部署透過 tar 串流與 `ssh -T` 上傳到全新的 `release.*`。在 `/volume1/docker/galroon/` 執行 `sudo -n /usr/local/bin/docker build`，以該 release 為 context，標記 `galroon:<BUILD_SHA 前 12 碼>` 與 `galroon:local`。`.dockerignore` 只允許 binary、Web 與 Dockerfile，不傳入資料庫。接著檢查 compose 設定、寫入 `.env`，執行 `docker compose up -d --no-deps galroon`；compose 選用 SHA tag、`pull_policy: never`，避免誤拉 registry image。只對 project/service `galroon` 操作；若同名 container 屬於其他 project/service 則停止。

   每次部署會重寫 `.env`，不會自動繼承之前手動修改或 polling 設定。腳本不做 image prune、container 清理、compose down、孤立服務移除或資料刪除。失敗的 build context 也會保留；請勿同時執行兩次部署。

4. 檢查 `/api/health`、Web UI 與日誌。`up -d` 成功只表示已提交啟動，並不代表 Core 已就緒；必須做下方通道與 HTTP 檢查。埠衝突、資料目錄 ACL 等會使 Core 啟動失敗。不要改綁 `0.0.0.0` 解決問題。

## owner 與 SSH 通道

```bash
scripts/nas/owner.sh setup                          # 終端隱藏輸入；密碼經 stdin
scripts/nas/owner.sh change-password                # 變更密碼並撤銷所有 session
scripts/nas/owner.sh pair --name 'N1 desktop' --role desktop
scripts/nas/owner.sh devices
scripts/nas/owner.sh revoke <device-id>
scripts/nas/tunnel.sh                              # 獨立終端持續執行，Ctrl-C 關閉
```

`setup` 與 `change-password` 使用 `ssh -T` 與 `docker exec -i`，不把密碼放入 argv、環境或檔案；也可由可信的密碼工具直接 pipe 一行到 `owner.sh setup` 或 `owner.sh change-password`。勿啟用 shell trace。其他 owner 命令的 stdout 保留原始 JSON。包裝器固定 `--data-dir /data`，不支援另指資料庫；支援 `pair --role desktop|web`、`pairing-codes` 與 `network show/set`；不提供瀏覽器還原。

通道只綁 Mac 的 `127.0.0.1:14800`，轉送到 NAS 的 `127.0.0.1:14800`；若本機埠被佔用會失敗。通道開啟後可執行：

```bash
curl --fail http://127.0.0.1:14800/api/health
curl --fail http://127.0.0.1:14800/ -o /dev/null
```

第二項檢查 image 內的正式 Web UI。在 Mac 開啟 `http://127.0.0.1:14800`，以擁有者密碼登入即可新增來源、掃描與編輯收藏，路徑請輸入 NAS 上的路徑。密碼登入取得 `web_owner`，有效期 30 天，具有桌面配對相同的權限，但不能管理密碼、配對碼、裝置或網路設定。瀏覽器配對碼登入仍是唯讀 `web`，有效期 12 小時；既有 `web` session 不會自動升級，須重新以密碼登入。變更密碼會撤銷全部 session。

也可直接使用 LAN HTTPS：

```bash
scripts/nas/owner.sh network set --address 192.168.50.154 --port 14830 --hostname nas.local --enable
scripts/nas/owner.sh network show
```

套用設定後重新啟動 Core，再在 Mac 開啟 `https://192.168.50.154:14830`（或已設定的主機名稱）。先透過可信 SSH 執行 `network show`，記下 `certificate_sha256`，並與瀏覽器顯示的整張憑證 SHA-256 比對，勿誤用 SPKI 指紋。確認相符後，匯出該憑證並匯入 Mac「鑰匙圈存取」，開啟憑證的「信任」設定，將 SSL 設為信任；這個動作只需一次，憑證更換後須重新比對。重新開啟網址，以擁有者密碼登入。登入與寫入都需要相同 HTTPS Origin；LAN cookie 帶 Secure，owner bearer 仍禁止經 LAN 使用。

需要本機 owner 管理權限的開發 UI 使用：

```bash
mkdir -p target/nas-session
chmod 700 target/nas-session
(umask 077; scripts/nas/owner.sh session > target/nas-session/session.json)
npm ci
GALROON_DEV_SESSION="$PWD/target/nas-session/session.json" npm run dev
```

開啟 `http://127.0.0.1:1420`。session 包含 owner bearer，只放在本機受保護且已被 Git 忽略的 `target/`；不要分享開發伺服器、session 或畫面內的憑證。Core 重啟後須重新取得 session，再重新整理 UI。

## N1 實機驗收（對應計畫 §8）

1. 確認 N1a、N1b、N1c 與匹配 Phase 1b 都在待測 commit。以**相同 commit、同一份收藏、同樣匹配設定**，在 Mac 經 SMB 重新建立基準，記錄 commit、收藏檔案清單雜湊與設定。舊 `nas-2026-10-11-v6` 的 561 筆是 algorithm 5，不能直接當基準。本 PR 的 sandbox 不執行 SMB／收藏操作。
2. 在 NAS 以新的資料目錄啟動，先記錄收藏檔案清單與 mtime、NAS 可用記憶體／swap、其他服務既有狀態。透過上述 owner session＋SSH 通道＋開發 UI 新增 `/volume1/Backup/Galgame`，執行完整掃描與自動匹配，記錄兩者耗時與設定。唯讀 bind mount 與 galroon-dev 的唯讀 ACL 全程保留。
3. 從 container 啟動直到掃描、匹配結束，定期執行 `scripts/nas/status.sh` 保存輸出。它只 inspect `galroon`、顯示最後 50 行日誌，優先讀 host `/sys/fs/cgroup/memory/docker/<id>/memory.max_usage_in_bytes`（v1），再試 systemd／v2 `memory.peak` 路徑及 container 內的 counter，並標示版本與實際路徑；找不到時明確回報 unavailable，**不可當作 0 或驗收通過**。
4. 記憶體尖峰初始預算 **600 MiB = 629145600 bytes**，首次實測後確認。cgroup counter 包含 `7z` 子程序與快取，不使用會扣掉部分快取的 `docker stats` 顯示值取代。compose `1g` 是硬上限，不是 600 MiB 驗收門檻。同時記錄 host `MemAvailable`、`SwapTotal - SwapFree`、`OOMKilled`、`RestartCount` 與啟動時間；不得 OOM 或異常重啟。峰值需在停止／重建前保存，重建 container 會重新計量；若曾重啟或 counter 無法取得，該輪須調查後重測。
5. 保存一致的 NAS 收藏庫副本：先記錄峰值與停止日誌，再依下節停止 Core，將 data 目錄（含必要的 SQLite WAL 檔）複製到 Mac 的獨立驗收目錄；不要對執行中的 SQLite 只複製主檔。重新啟動服務。使用匹配 Phase 1b 的工具比較：

   ```bash
   match_replay <NAS收藏庫副本> --baseline <Mac基準報告> \
     --root-map /Volumes/Backup/Galgame=/volume1/Backup/Galgame \
     --answers ~/GalroonTestData/nas-answers.json \
     --fail-on changed,false,lost,missing --out n1.json
   ```

   報告保存在 `~/GalroonTestData/nas-n1-<日期>/`。結束碼必須為 0：錯誤匹配、changed、lost、missing、重複 key 均為 0；added／removed 只能是測試期間已記錄的資料夾變動；全部 `unreviewed` 要人工核對。該命令是後續實機驗收說明，本 PR 不實作或執行 Phase 1b。
6. 比較掃描前後收藏檔案清單與 mtime，證明 Core 沒有寫入。由有權限的測試操作者在 NAS 新增一個遊戲資料夾，驗證 watcher 能偵測；將這項刻意新增獨立記錄，不視為 Core 寫入。部署腳本本身不新增／刪除收藏檔案。
7. 測試 N1c 監看備援時，以 `GALROON_WATCH_FORCE_POLLING=1 GALROON_WATCH_POLL_SECONDS=30 scripts/nas/deploy.sh` 重建測試 container（自訂 data 等設定仍須保留）。確認 N1c 的診斷顯示備援原因，且定期掃描**確實找到新資料夾**。此兩個變數在 N1b 僅傳入；需 N1c 的實作才會生效，並依該 PR 確認錯誤注入開關的確切語意。不要在共用 NAS 調低 inotify 配額；真正耗盡測試留在 CI／隔離 VM。完成後清除兩個環境變數，重新部署恢復預設空值。
8. 有進行中任務時，執行下節的 60 秒正常停止，確認暫停、drain、checkpoint，重啟後資料完整。保留停止後的日誌；正常關閉不應被 60 秒到期的 SIGKILL 截斷。測試 NAS 重開後會自動啟動（重開前應先以 `up -d` 維持 running；`unless-stopped` 會尊重手動停止）。NAS 重開機需由操作者安排維護時段；部署腳本不會重開 NAS。
9. 核對 NAS 其他 container 未受部署與測試命令影響。N2 的 LAN 配對、N3 的還原／寫入功能不在這次驗收範圍。

## 狀態、停止、更新與回退

```bash
scripts/nas/status.sh
# 下列命令只針對本專案的 galroon service：
ssh galroon-dev@192.168.50.154 'cd /volume1/docker/galroon && sudo -n /usr/local/bin/docker compose --project-name galroon --env-file .env -f compose.yaml stop -t 60 galroon'
ssh galroon-dev@192.168.50.154 'cd /volume1/docker/galroon && sudo -n /usr/local/bin/docker compose --project-name galroon --env-file .env -f compose.yaml up -d --no-deps galroon'
```

若改 `GALROON_NAS`，手動命令也需換成同一個 SSH 目標。不要使用無範圍的 `docker stop $(docker ps -q)`、prune 或 `--remove-orphans`。持久 data 不隨 container 更新移除。上述 compose 停止會套用 60 秒；直接操作時等價為 `sudo -n /usr/local/bin/docker stop -t 60 galroon`，不要用 Docker 預設較短的 timeout。

更新順序：保存驗收資料與一致的資料備份 → 下載指定 run → 核對 `BUILD_SHA` → 用原本的環境設定執行 `deploy.sh` → `status.sh` → 重新取得 owner session 並檢查 health／UI。NAS 不編譯 Rust，也不執行 npm。

每次更新會保留原 SHA image tag，並將當時**實際執行中的 image ID** 標為 `galroon:previous`，另記錄 `previous-image-id`。即使 `:local` 已被新 build 覆蓋，也不會誤把新版當前一版。不要刪除尚需回退的 SHA tag；`previous` 只保留最近一次更新前的 image。

需要回退時，先確認舊版支援現在的資料 schema；**image 回退不等於資料還原**。如 schema 不相容，應停止並使用先前的一致備份安排復原。Core 層級的 N1d `restore` 命令仍未提供；NAS 快照可依下節還原至全新資料目錄，再搭配相容的 image 啟動。在 NAS SSH shell 中執行：

```bash
cd /volume1/docker/galroon
# 記下欲回退的既有 SHA tag；沒有記錄時可用 galroon:previous。
rollback_image=galroon:previous
sudo -n /usr/local/bin/docker image inspect "$rollback_image" --format '{{.Id}}'
# 明確選擇回退 image，避免 sudo 丟棄 shell 環境變數。
sed "s/^GALROON_IMAGE=.*/GALROON_IMAGE=$rollback_image/" .env > .env.rollback
chmod 600 .env.rollback
sudo -n /usr/local/bin/docker compose --project-name galroon --env-file .env.rollback -f compose.yaml config --quiet
sudo -n /usr/local/bin/docker compose --project-name galroon --env-file .env.rollback -f compose.yaml up -d --no-deps galroon
mv .env.rollback .env
```

再跑 `status.sh`、health／UI，並重新取得 session。若需跨多版回退，將 `rollback_image` 換成已記錄的 `galroon:<12碼SHA>`。這些步驟不動其他 container，也不刪除 `/volume1/Backup` 內的任何資料。

## 自動備份與還原

NAS 腳本 `galroon-backup.sh` 提供完整災難復原快照：以 SQLite `.backup` 線上備份 `library.sqlite`，預設每天也備份 `artwork-library.sqlite`，另複製 `device.json`、`network.json` 與 `lan-*.pem`。不複製 `lan-tls.lock`、Core 自有的 `backups/`、`migration-backups/`，也不複製遊戲收藏。Core 的 `backup::export` 會移除憑證與裝置身分，因此不適合此用途。每個資料庫各自取得一致快照；兩個資料庫與身分檔不是同一瞬間的跨檔案交易，變更配對／網路設定時請避開備份時段。

預設從 `/volume1/docker/galroon/.env` 解析 `GALROON_DATA`（例如 `/volume1/docker/galroon/data-production-20261016`），不執行 `.env`。快照存放在 `/volume1/docker/galroon/backups/<UTC時間>-daily|weekly`。最近 6.5 天內沒有完成的 weekly 時，本次為 weekly，否則為 daily；預設保留最新 **7 份 daily＋4 份 weekly**。先在 `.incomplete-*` 目錄完成 SQLite 完整性、筆數與 SHA-256 記錄，轉為不需 WAL 的副本，執行 `sync` 後改名，才清理舊快照。失敗會回傳非零狀態並移除本次暫存目錄；舊 `.incomplete-*` 只警告、不自動刪除。若留下 `.lock`，先確認沒有備份／清理作業執行，再依錯誤訊息用 `rmdir` 移除。

實機收藏目錄資料庫約 **72 MB**、圖片庫約 **358 MB**；每份完整快照約 430 MB，**11 份約 4.7 GB**，只保存收藏目錄則約 **0.8 GB**。NAS 尚有約 **14 TB** 可用空間，因此圖片預設每天保存（`always`），產生新快照時還需額外一份空間。這是 #61 的永久圖片封存庫，雖可重新抓取，但 VNDB 限流使恢復很慢，手動圖片連結也會遺失。圖片備份前要求可用空間大於圖片庫大小的 1.2 倍＋1 GiB；不足時整次備份失敗，不會悄悄略過。

可在 NAS 執行環境調整 `GALROON_BACKUP_DAILY`、`GALROON_BACKUP_WEEKLY`（整數且至少 1）、`GALROON_BACKUP_ARTWORK=always|weekly|never`（預設 `always`；`weekly` 僅週快照含圖片，`never` 不含圖片）、`GALROON_BASE`、`GALROON_BACKUP_DIR`、`GALROON_DATA`、`GALROON_SQLITE` 與 `GALROON_DOCKER`。備份目錄不得等於或位於 data 內。一般備份不需要 sudo；記錄 image 時只嘗試 `sudo -n /usr/local/bin/docker inspect`，失敗記為 `unknown`，不影響備份。NAS 需有 `sqlite3`、`sha256sum`（或 `shasum`）及基本 shell 工具。

**快照包含 owner 密碼雜湊、sessions 與 LAN 私鑰，請勿分享。** 腳本採 `umask 077`，目錄／檔案嘗試設定為 700／600；Synology 共用資料夾 ACL 可能強制目錄為 777，仍須在 DSM 限制其他帳號與共享服務存取。快照與正式資料位於同一個 volume，**無法防護 volume 損毀**；請定期拉回 Mac，及／或用 Hyper Backup 備份 `/volume1/docker/galroon/backups` 到另一個儲存目的地。

### 安裝與排程

在 Mac 的 checkout 執行（只上傳腳本，不啟動排程）：

```bash
scripts/nas/backup.sh install
scripts/nas/backup.sh backup
scripts/nas/backup.sh list
scripts/nas/backup.sh verify                    # 預設最新，也可指定快照名稱
scripts/nas/backup.sh pull                      # ~/GalroonBackups/nas/<快照名稱>
# scripts/nas/backup.sh pull --with-artwork /另一個本機目錄
scripts/nas/backup.sh install-mac-schedule       # 每天 06:00 拉回 Mac
# scripts/nas/backup.sh install-mac-schedule /另一個本機目錄
# scripts/nas/backup.sh uninstall-mac-schedule
```

`pull [--with-artwork] [local-dir]` 下載最新快照的收藏目錄、身分檔、MANIFEST 與 SHA256SUMS，預設略過 `artwork-library.sqlite`，約 **75 MB／天，保留 14 份約 1 GB**。圖片可重新下載，所以異地副本可選擇省略，但恢復較慢且手動圖片連結會遺失；需要完整副本時加 `--with-artwork`（選項放在本機目錄之前）。下載先放入 `.partial-<快照名稱>`，在 Mac 用 `shasum -a 256 -c` 驗證後才改名；失敗移除本次暫存目錄。最新快照若已在本機且驗證通過，印出訊息並成功退出，不重新下載。既有同名副本若驗證失敗則報錯，請先移開再重試；從 catalog-only 改拉完整副本時也應使用另一個目錄或移開原副本。

成功下載後依 `GALROON_PULL_KEEP`（預設 14，整數且至少 1）保留最新本機快照；只刪除名稱符合 `YYYYMMDDTHHMMSSZ-daily|weekly` 的目錄，不處理符號連結或其他目錄。MANIFEST／SHA256SUMS 保留 NAS 原始內容，省略圖片的副本不能直接當成含 artwork 的完整 NAS 快照通過 `verify`；要保留圖片復原能力，可用 `--with-artwork` 或 Hyper Backup。SSH 包裝器使用預設 NAS 備份路徑；自訂 `GALROON_BACKUP_DIR` 時，請直接在 NAS 使用腳本並自行安排異地複製。

Mac 排程安裝至 `~/Library/LaunchAgents/com.galroon.nas-backup-pull.plist`，每天 **06:00** 執行 `pull`（NAS 於 04:30 備份），不在載入時立即執行。Mac 睡眠錯過排程時，launchd 會在下次喚醒時補跑。輸出與錯誤均寫入 `~/Library/Logs/galroon-nas-backup-pull.log`。安裝時以 `plutil -lint` 檢查，再透過 `launchctl bootout`／`bootstrap` 載入；解除安裝會卸載並移除 plist。

排程使用**此 checkout 的絕對路徑**，請在長期保留的主 checkout 安裝，不要使用暫時 worktree。安裝時會保存已設定的 `GALROON_NAS`，並設定包含 `/opt/homebrew/bin` 的 PATH。`pull` 的 SSH 一律使用 `-o BatchMode=yes`；SSH key 必須能在沒有互動式 agent／密碼提示的情況下使用，並事先確認主機金鑰。預設排程拉 catalog-only 並保留 14 份；若需排程也帶圖片，可在 plist 的 `pull` 後、本機目錄前加入 `--with-artwork`；自訂保留數則加入 plist 的 EnvironmentVariables，修改後重新載入。測試可用 `GALROON_LAUNCH_AGENTS`、`GALROON_LAUNCHCTL` 與 `GALROON_SSH` 替換目錄或命令。

DSM 設定如下：

1. **控制台 → 工作排程器 → 新增 → 排程的工作 → 使用者定義的指令碼**（Control Panel → Task Scheduler → Create → Scheduled Task → User-defined script）。
2. **一般（General）**：工作名稱 `Galroon backup`，使用者 `galroon-dev`。
3. **排程（Schedule）**：每日，`04:30`（DSM 本機時間；快照名稱使用 UTC）。
4. **工作設定（Task Settings）**：設定收件地址並啟用「異常終止時寄送電子郵件」，命令填入：

   ```sh
   /volume1/docker/galroon/bin/galroon-backup.sh backup
   ```

儲存後可手動執行一次排程，確認郵件通知設定及 `list`／`verify` 結果。需要單獨清理時使用 `scripts/nas/backup.sh prune`，只處理名稱符合時間＋daily／weekly 且有 MANIFEST 的快照目錄。

### Core 手動匯出與清理

data 內的 `backups/` 是 Backup 面板按 **Export**、目的地填 `/data/backups` 時，透過 `POST /api/backups` 產生的手動匯出；Core 不會自動建立，也不會自動刪除。實機目前有 15 個 `Galroon-backup-<epoch>-<uuid>` 目錄，每份包含 `collection.sqlite`、`artwork-library.sqlite` 與 `manifest.json`，約 40–660 MB，合計 **4.9 GB，約占 data 目錄 93%**。這些匯出會移除憑證與裝置身分，不能替代上述災難復原快照。

先由 owner 檢查清單，建議只保留最新幾份：

```bash
scripts/nas/backup.sh core-exports
scripts/nas/backup.sh prune-core-exports --keep 3 --dry-run
# 確認後才執行：
scripts/nas/backup.sh prune-core-exports --keep 3
```

`core-exports` 列出名稱、manifest.json 的 `created` epoch（無法解析時使用名稱中的 epoch）、`size_kib` 與總計。`prune-core-exports` 必須指定 `--keep N`（整數且至少 0）；以**目錄名稱中的 epoch** 排序，只處理 data/backups 直接下一層、名稱符合 `^Galroon-backup-[0-9]+-[0-9a-fA-F-]+$`、有 manifest.json 且不是符號連結的目錄。`--dry-run` 只列印，實際刪除也會逐一印出路徑。刪除後 **Core 的 `backup_history` 資料表仍保留紀錄，Backup 面板「recent backups」仍會列出已刪除的路徑**。

排程 `backup` **永遠不會呼叫 Core 匯出清理，也不複製 data/backups**。另外保留既有更新前手動備份 `/volume1/docker/galroon/backup-before-5147b4c-20261016-152528`，不要隨這次清理刪除。

### 隔離還原測試

```bash
scripts/nas/backup.sh restore-test
scripts/nas/backup.sh restore-test 20261016T043000Z-weekly --cleanup
```

先驗證並還原到 NAS 的 `restore-test-<UTC時間>`，將其中的 `network.json` 改名為 `network.json.disabled-for-test`，再用正式 `galroon` 的 image（查不到時用 `galroon:local`）啟動 `galroon-restore-test`。使用 SSH 帳號的非 root uid/gid、768 MiB 記憶體上限、獨立控制通道 tmpfs、唯讀收藏掛載與 **`--network none`**，只測 container 內的 loopback HTTP。既有同名測試 container 會使測試拒絕執行。

90 秒內須取得 HTTP 200、`"name":"Galroon"`，並核對 works／resources 筆數。成功或失敗都只停止（60 秒）及移除該測試 container；失敗先列出最後 50 行日誌。預設保留並印出 scratch 目錄，`--cleanup` 只在成功後刪除本次目錄。測試會修改 scratch 資料庫，它不再是原始快照。

### 正式還原

1. 公告維護停機時間，從 `list` 選擇快照；若需要完整圖片庫，選 MANIFEST 標示含 artwork 的快照。確認所用 image 支援快照的 schema，必要時參照 MANIFEST 的 image。先完成 `verify` 與 `restore-test`。
2. 還原到 `/volume1/docker/galroon/` 下**尚不存在**的目錄；父目錄必須存在，路徑不得含 `..` 或單引號。腳本不會切換正式服務，也不會寫入原 data：

   ```bash
   scripts/nas/backup.sh restore 20261016T043000Z-weekly /volume1/docker/galroon/data-restored-20261016
   ```

3. 進入停機時段後，以 `GALROON_DATA=/volume1/docker/galroon/data-restored-20261016 scripts/nas/deploy.sh` 部署（其他自訂部署環境設定也須保留）。或在 NAS SSH shell 停止 Core，編輯 `.env` 的 `GALROON_DATA` 指向新目錄，再啟動同一個 service：

   ```sh
   cd /volume1/docker/galroon
   sudo -n /usr/local/bin/docker compose --project-name galroon --env-file .env -f compose.yaml stop -t 60 galroon
   # 編輯 .env：GALROON_DATA='/volume1/docker/galroon/data-restored-20261016'
   sudo -n /usr/local/bin/docker compose --project-name galroon --env-file .env -f compose.yaml up -d --no-deps galroon
   ```

4. 執行 `scripts/nas/status.sh`，透過 SSH 通道檢查 `/api/health`、UI 與作品／資源筆數，重新取得 `scripts/nas/owner.sh session`，並檢查桌面配對連線。確認無誤前保留舊 data 目錄，必要時停止 Core 後切回。

還原副本保留原裝置身分，因此已配對桌面可繼續使用。**不要讓兩個相同身分的 Core 同時出現在 LAN**；`restore-test` 使用 `--network none` 隔離。選用 `weekly` 或 `never` 政策產生的不含 artwork 快照仍可復原收藏與身分，但不含圖片封存；需要圖片時優先使用完整快照，不自行混合不同時間點的資料庫。

本機驗證可執行 `scripts/nas/test-backup.sh`，需 SQLite 與 macOS 系統工具（含 `plutil`），不需 Docker／真實 SSH；涵蓋背景 WAL 寫入、預設 daily 圖片與明確 weekly 政策、checksum、權限、保留數、安全還原、Core 匯出清理、mock SSH pull 與 stub launchctl 安裝／移除。真正的 DSM／launchd 排程、ACL、Docker 啟動與實機圖片備份時間仍須在對應主機驗證。

## 本 PR 驗證界線與風險

本次 sandbox 已通過全部 `scripts/nas/*.sh` 的 `bash -n`、PyYAML 解析與 `scripts/check_repository.py`。另以 mock SSH／gh 驗證參數引號（含空白、單引號、換行）、setup 僅經 stdin、危險設定拒絕、遠端 sh 語法、run ID／分支下載，以及損毀 artifact 不覆蓋前次下載。ShellCheck 未安裝，略過。

sandbox 無法啟動 Docker、連線 NAS 或證明實機 runtime 相依套件、DSM ACL、7-Zip、compose plugin、cgroup 路徑、記憶體門檻與 graceful shutdown。上述項目都需實際 CI 與 N1 驗收確認。

Ubuntu 同版本可降低 glibc 不相容風險，但不代表已驗證所有動態函式庫；首次啟動必須檢查 loader 錯誤。基底 image、apt 套件與 stable Rust 工具鏈會隨時間更新，目前沒有以 digest 固定，應保留已驗收的 image tag。artifact 下載與 release context 都會累積磁碟使用；工具不會自動清理。SHA tag 識別來源 commit，不保證同一 commit 在不同日期重建的 image 完全相同。
