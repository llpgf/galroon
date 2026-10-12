# NAS 部署（PR N1b）

適用於 Synology DS920+（J4125、x86_64、DSM Container Manager）。本 PR 僅提供 N1 的部署工具；沿用 N1a 的 Linux CLI、Unix socket 與固定 `127.0.0.1:14800`。完整 N1 驗收還需要 **N1c** 的掃描／封存／watcher 相容性與**匹配計畫 Phase 1b**。不提供 LAN、TLS、N1d 的 `restore` 或 N3 搬移。

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
| `/volume1/docker/galroon/data` | 持久資料、裝置身分、資料庫、`control/core.sock` |
| `/volume1/docker/galroon/compose.yaml`、`.env` | 最近一次部署的 compose 與非機密設定 |
| `/volume1/docker/galroon/release.*` | 每次上傳的獨立 build context，保留供檢查 |
| `/volume1/docker/galroon/previous-image-id` | 更新前 container 的 image ID（若存在） |
| `/volume1/Backup/Galgame` | 唯讀收藏，同路徑掛入 container |

`/data` 必須位於 NAS 本機磁碟；N1a 拒絕 NFS、CIFS/SMB、FUSE。`control/` 為 0700，socket 為 0600，Core 與 owner CLI 皆用相同非 root uid，雙向驗證 peer uid。此目錄不可分享給其他 container；信任能存取 socket 的同 uid 程序，並不防護 NAS root 或 Docker 管理員。

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
scripts/nas/owner.sh pair --name 'N1 desktop' --role desktop
scripts/nas/owner.sh devices
scripts/nas/owner.sh revoke <device-id>
scripts/nas/tunnel.sh                              # 獨立終端持續執行，Ctrl-C 關閉
```

`setup` 使用 `ssh -T` 與 `docker exec -i`，不把密碼放入 argv、環境或檔案；也可由可信的密碼工具直接 pipe 一行到 `owner.sh setup`。勿啟用 shell trace。其他 owner 命令的 stdout 保留原始 JSON。包裝器固定 `--data-dir /data`，不支援另指資料庫；N1a 目前僅接受 `desktop` 配對，不支援 `web`、`network` 或 `restore`。

通道只綁 Mac 的 `127.0.0.1:14800`，轉送到 NAS 的 `127.0.0.1:14800`；若本機埠被佔用會失敗。通道開啟後可執行：

```bash
curl --fail http://127.0.0.1:14800/api/health
curl --fail http://127.0.0.1:14800/ -o /dev/null
```

第二項檢查 image 內的正式 Web UI；一般瀏覽器登入取得的是唯讀 `web` 角色，無法新增來源或掃描。N1 的 owner 開發 UI 使用：

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

需要回退時，先確認舊版支援現在的資料 schema；**image 回退不等於資料還原**。如 schema 不相容，應停止並使用先前的一致備份安排復原，本 PR 不提供 `restore`。在 NAS SSH shell 中執行：

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

## 本 PR 驗證界線與風險

本次 sandbox 已通過全部 `scripts/nas/*.sh` 的 `bash -n`、PyYAML 解析與 `scripts/check_repository.py`。另以 mock SSH／gh 驗證參數引號（含空白、單引號、換行）、setup 僅經 stdin、危險設定拒絕、遠端 sh 語法、run ID／分支下載，以及損毀 artifact 不覆蓋前次下載。ShellCheck 未安裝，略過。

sandbox 無法啟動 Docker、連線 NAS 或證明實機 runtime 相依套件、DSM ACL、7-Zip、compose plugin、cgroup 路徑、記憶體門檻與 graceful shutdown。上述項目都需實際 CI 與 N1 驗收確認。

Ubuntu 同版本可降低 glibc 不相容風險，但不代表已驗證所有動態函式庫；首次啟動必須檢查 loader 錯誤。基底 image、apt 套件與 stable Rust 工具鏈會隨時間更新，目前沒有以 digest 固定，應保留已驗收的 image tag。artifact 下載與 release context 都會累積磁碟使用；工具不會自動清理。SHA tag 識別來源 commit，不保證同一 commit 在不同日期重建的 image 完全相同。
