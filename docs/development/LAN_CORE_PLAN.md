# 區網 Core 規劃書：一個 Core、多台客戶端

狀態：**第 6 版：NAS 軌道已通過第 4 輪審查（`approve_with_changes`，修改已併入），開始執行**（2026-10-12）
範圍：讓同一區網內的多台機器連到同一個 Galroon Core，共用同一份收藏、標籤、清單與分類資料。

> **給審查者與實作者**：兩輪審查的處理結果見第 10 節。本文每一項現況描述都附了檔案與行號，請對照程式碼確認。

---

## 1. 目標與非目標

**目標**

- **主要部署目標：Core 跑在 Synology NAS 上**（使用者的 DS920+，x86_64，透過 Container Manager 執行 Docker）。也保留 Windows 機器當 Core 主機的方式。
- 區網內的 Windows 桌面程式透過配對連到 Core，可讀可寫（`desktop` 角色）。
- 其他裝置的瀏覽器可以唯讀瀏覽（`web` 角色）。
- 不在家時，可透過 Tailscale 或 WireGuard 讓客戶端直接連到 Core 的虛擬內網位址（見 4.13）。
- 預設行為不變：沒開區網模式時，Core 仍然只綁定 loopback，既有行為與 API 契約的回歸測試全部通過。

**非目標**

- 多台機器同時開同一個 SQLite 檔（例如放在 SMB 上）。現在不支援，以後也不支援。
- 多份資料庫互相同步或合併。
- 直接對公網開放 Core、做連接埠轉發，或類似 Roon ARC 的雲端中繼。
- 從遠端啟動遊戲（目前產品本來就沒有這個功能）。
- 第 1 階段**不支援**：`tailscale serve` 或任何反向代理放在 Core 前面、MCP 連遠端 Core、Synology（x86_64 Docker）與 Windows 以外的 Core 主機（例如 ARM 機型、其他 NAS 品牌、未容器化的 Linux）。

## 2. 為什麼選「單一 Core」

| 做法 | 結論 | 原因 |
|---|---|---|
| 共用資料庫檔 | 否決 | SQLite 在網路檔案系統上同時寫入不安全；文件已明確列為不支援（`docs/user/SUPPORT.md`「Core storage」）。 |
| 每台各一份資料庫再同步 | 否決 | 現有的 revision CAS、可續跑任務、需審核的檔案操作，都假設只有一個寫入者。改成同步等於重寫，衝突也沒有好解法。 |
| **單一 Core＋多客戶端（Roon 模式）** | **採用** | 現有的身分、配對、權限機制本來就區分多種客戶端，只是被鎖在 loopback。 |

## 3. 現況（已依第 1 輪審查更正）

### 3.1 已經具備、可沿用的部分

- **三種身分**：本機 owner（程序內 `App.token`，經控制通道取得）、配對桌面 `desktop`（bearer，固定 90 天）、瀏覽器 `web`（cookie，固定 12 小時）。`last_used` 不會延長期限。見 `crates/core/src/access.rs:11`、`access.rs:40`。
- **配對碼**：只有 owner 能產生；12 位 hex、5 分鐘過期、交易內一次性兌換、同時最多 5 組有效。**目前沒有逐筆撤銷未兌換配對碼的 API**；`revoke` 撤銷的是已發出的 session，改 owner 密碼才會清掉所有配對碼（`access.rs:29`、`access.rs:38`、`access.rs:86`）。
- **web 角色禁止收藏寫入**：middleware 擋下已認證 web session 的非 GET/HEAD 請求（登出例外）。這不代表 GET 沒有副作用，例如快取或 session `last_used` 更新（`access.rs:61`、`artwork.rs:45`）。
- **連線身分鎖定**：請求**帶了** `x-galroon-library` / `x-galroon-device` 且不符時回 409；省略標頭仍可通過。`/api/health`、`/api/access/status`、login、pair 不經這項檢查（`access.rs:61`、`access.rs:68`）。
- **桌面端已支援配對連線**：連線資料存 `connections.json`，token 存 Windows 認證管理員；連線時核對 library/device，不會自動改接其他 Core（`apps/desktop/src-tauri/src/paired.rs:15`、`:55`、`:99`）。
- **寫入衝突保護**：**一般收藏編輯**在交易內檢查 revision。任務控制、建立來源、檔案計畫使用其他狀態或 digest 保護；不是所有 POST 都有 revision CAS（`lib.rs:183`、`lib.rs:186`、`plans.rs:117`）。
- **檔案操作在 Core 那台執行**。organize 等計畫有預覽 → 批准 → 執行；備份直接匯出；acquire 由選取結果建立背景任務。並非所有檔案功能都是相同的三階段流程（`lib.rs:215`、`:222`、`:242`）。
- **Core 自己提供 Web UI**：`lib.rs:164` 的 `ServeDir` 指向執行檔旁的 `web` 目錄。

### 3.2 阻擋區網的地方

| # | 位置 | 現況 |
|---|---|---|
| B1 | `crates/core/src/local_core.rs:59` | 正式 Core 綁定 `127.0.0.1:0`，由作業系統分配埠，**重啟後可能改變**。 |
| B2 | `crates/core/src/lib.rs:171` | `serve()` 綁定 IPv4 loopback；`serve_listener()` 接受外部傳入的 listener，不檢查綁定位址。 |
| B3 | `access.rs:59-60` | Host 只接受 `localhost` / `127.0.0.1` / `[::1]`，缺少或無法解析也拒絕。 |
| B4 | `access.rs:48-51`、`lib.rs:168` | CORS 是固定的本機白名單；`origin_allowed` 另外允許 `Origin == http://{Host}`，但只認 `http://`。只放寬 Host 的話，HTTPS 下的登入和配對會因為 Origin 不符而失敗。 |
| B5 | `apps/desktop/src-tauri/src/paired.rs:32-39` | 配對 URL 只接受 `http://127.0.0.1` / `localhost`（後者正規化為前者）。 |
| B6 | `apps/desktop/src-tauri/tauri.conf.json:24` | CSP 的 `connect-src` 對 Core 只允許 `http://127.0.0.1:*`（另有 `'self'`、`ipc:`、`http://ipc.localhost`）。 |
| B7 | 全部 | Core 沒有 TLS。現在因為只走 loopback 所以沒有暴露；**如果只放寬綁定位址**，bearer、cookie 和瀏覽器登入密碼就會以明文在區網上傳送。 |
| B8 | `crates/mcp/src/config.rs:22` | MCP 只接受 `http://127.0.0.1` / `localhost`。 |
| B9 | `crates/core/src/context.rs:46` | `/api/context` 寫死 `"location":"local","transport":"loopback"`。 |

### 3.3 開放區網後會出問題的地方

| # | 位置 | 問題 |
|---|---|---|
| R1 | `access.rs:31`、`access.rs:82`、`lib.rs:93` | 嘗試次數以 `kind` 為單位全域共用：視窗開始後 60 秒內超過 10 次就拒絕。另有全域 `login_gate`（一次只處理一個登入）。區網上任何一台機器都能影響所有人。 |
| R2 | `access.rs:33`、`access.rs:40` | 瀏覽器登入用的是「owner 密碼」，但它**只能換到 `web` session**，換不到 owner 權限（owner 權限只來自 `App.token`）。真正的風險是：一個長期共用的密碼會在區網上被反覆輸入和傳送。 |
| R3 | `access.rs:82`、`access.rs:87` | 登入與登出的 `Set-Cookie` 都沒有 `Secure`。 |
| R4 | `src/api.ts:105` 與 6 個呼叫點 | 本機資料夾選擇器 `folder()` 選的是**客戶端**的路徑，但 Core 會在**自己的機器**上使用。受影響的有：新增來源（`main.tsx:138`）、整理目的地（`main.tsx:106`）、重複檔隔離（`DuplicatePanel.tsx:10`）、acquire 目的地（`AcquirePanel.tsx:20`）、備份匯出（`BackupPanel.tsx:17`）、來源重新對應（`SourceRemap.tsx:9`）。其中整理和隔離**只能**用選擇器，遠端連線時根本沒有正確的輸入方式。 |
| R5 | `src/api.ts:73`、`:95` | 前端用 `fetch` 直接連 Core，這條路沒有憑證綁定能力。WebView2 原生層有憑證錯誤事件可以掛接，但需要另外寫 Tauri 整合並驗證。 |
| R6 | `local_core.rs:32`、`examples/core_control.rs:8` | owner 權限只能透過本機控制通道取得。同一使用者的本機程式可以呼叫 `request()`（`core_control` 範例就是這樣），但沒有正式支援的管理 CLI。 |
| R7 | `src/main.tsx:86`、`:93` | 前端定期輪詢的是 **jobs**，只有 job 狀態變了才整體重新載入。另一台只改作品或標籤、沒有產生任務時，畫面**可能一直不更新**。CAS 只能防止受保護的寫入互相覆蓋。 |
| R8 | `crates/core/src/main.rs:8` | 正式 Core 執行檔在非 Windows 平台直接結束。`ui_core`、`collection_ui` 等測試輔助程式可在其他平台跑，但不算正式支援。 |
| R9 | `local_core.rs:67` | Core 預設由桌面程式啟動。停止時若有進行中的任務或計畫會被拒絕；目前不會檢查遠端裝置是否還在使用。 |
| R10 | `lib.rs:172`、`local_core.rs:62`、`:73` | `serve_listener()` 自己會啟動 watching、automatch、smart updates；正式入口 `run_with_device()` 則自己啟動背景工作，再直接呼叫 `axum::serve`。**目前沒有重複啟動**，但新增第二個 listener 時若重用 `serve_listener()`，背景工作就會重複啟動。graceful shutdown 也只等待一個 HTTP server。 |
| R11 | `access.rs:63`、`api.ts:15`、`:102`、`paired.rs:43` | 還原收藏會清掉所有 session（`backup.rs:14`、`:56`）。之後遠端請求先拿到 401，還沒輪到 library/device 檢查。前端遇到 401 會進入重連；桌面端把 HTTP 錯誤轉成一般字串，沒有「憑證已撤銷/過期」的可辨識狀態，可能**一直重試**。 |
| R12 | `context.rs:21`、`lib.rs:173` | 沒有單一的「收藏變更序號」：`context.library.revision` 只在收藏改名時增加；dashboard 用 `smart_catalog_state.revision`；成員分頁又有自己的 revision。 |
| R13 | `lib.rs:174`、`resource_catalog.rs:7`、`lib.rs:238`、`:241` | 唯讀的 `web` 角色可以讀 `/roots`、`/resources`、`/plans`、`/quarantine`，回應裡有 Core 機器上的實體路徑。 |
| R14 | `lib.rs:183`、`plans.rs:117`、`access.rs:45` | 任務控制與計畫批准/執行**沒有記錄是哪個 session 做的**；A 可以操作 B 建立的任務。`last_used` 最多每分鐘更新一次，也沒有斷線訊號，無法精確說出「目前有幾台在線」。 |

## 4. 設計

### 4.1 Core 程序與監聽器（R10、B2、B3、B4、B9）

**程序層級的生命週期**（第一個 PR，行為不變）：

- 把 watching、automatch、smart updates 的啟動抽出成「每個程序只啟動一次」的元件；`serve_listener()` 不再自行啟動背景工作。
- 新增統一的停止管理：所有 listener 一起停止、一起 drain，之後才停背景工作並做 checkpoint。

**兩個監聽器**：

- **loopback 監聽器**：行為不變，仍是 `http://127.0.0.1:<作業系統分配埠>`。
- **區網監聽器**（預設關閉）：只提供 HTTPS，固定連接埠（預設 `14830`，可設定），綁定使用者選定的介面位址。
- 兩者共用同一份路由定義和同一個 `App`，但**各自套上 listener wrapper**，由 wrapper 在請求上標記 `Transport::{Loopback, Lan}`。這個標記**只由伺服器設定**，不從任何標頭推導。

**middleware 順序**（由外到內）：

1. **Transport / Host 檢查**：沒有 `Transport` 標記的請求一律拒絕，不預設為 loopback。這一層要放在 **CORS 外面**，否則 OPTIONS preflight 會被 CORS 直接回應而跳過檢查。
2. CORS。
3. 認證與角色檢查（現有的 `guard`，依 `Transport` 分流，見 4.3）。

靜態檔案（`ServeDir`）和未知路由也要經過第 1 層。

**組裝方式**：在所有 route、`fallback_service` 都建立完成之後，對整個 `Router` 套用 `.layer(...)`（不能用 `route_layer`，它不涵蓋 fallback），順序為：先套 `guard` 等現有 layer，再套 CORS，最後套 Transport / Host 層，使它位於最外側。`router(app)` 保持現有簽名，等同 `router_for(app, Transport::Loopback)`，讓既有測試與範例不用修改。測試需涵蓋 405、HEAD、OPTIONS、靜態檔與未知 API 路由。

**區網監聽器的限制**：

- 拒絕 owner bearer。owner 只能從 loopback 進入。
- Host 只接受 Core 自己的區網位址與設定的主機名稱；Origin 改為依 `Transport` 判斷 `https://{Host}` 同源。
- 啟動時檢查綁定位址：只允許 RFC 1918、`100.64.0.0/10`（Tailscale CGNAT）、IPv6 ULA（`fc00::/7`）。**第 1 階段不支援 IPv6 link-local**：scope ID 是客戶端本機的介面識別，無法由 Core 產生後跨機器沿用。公網位址直接拒絕，不提供繞過設定。
- `/api/context` 依實際 `Transport` 回報 `transport`（`loopback` / `lan`）。

**網路設定**：

- 存在裝置目錄（`--device-dir`）的 `network.json`，不寫進收藏資料庫，所以備份與還原收藏不會帶走或覆蓋它。
- `App` 目前只保存 `state_dir`（`lib.rs:74`），需要再加上 `device_dir`。
- 只有 owner 能修改；修改後需要重啟 Core。

### 4.2 TLS 與憑證綁定

- 第一次開啟區網模式時，用 `rcgen` 產生自簽憑證（ECDSA P-256）。私鑰存在裝置目錄，權限只限目前使用者。
- 伺服器使用 `tokio-rustls`（專案已經透過 reqwest 使用 rustls）。
- 指紋採 **SPKI 的 SHA-256**，所以換發憑證時可以保留同一把金鑰。
- **客戶端的綁定方式**：用自訂的 rustls `ServerCertVerifier`，**完整執行 TLS 握手簽章驗證**，只把「信任鏈與主機名稱檢查」換成「SPKI 雜湊必須等於已儲存的值」。**不能**先關閉所有憑證驗證、再拿字串比對指紋。
- 不檢查主機名稱，所以 Core 的 IP 改變不需要重新配對。

### 4.3 角色 × 連線方式的權限矩陣

| 功能 | owner（只能 loopback） | `desktop`（loopback 或 LAN） | `web` @ loopback | `web` @ LAN |
|---|---|---|---|---|
| 收藏讀取 | ✓ | ✓ | ✓ | ✓（依 4.8 的投影） |
| 收藏編輯 | ✓ | ✓ | ✗ | ✗ |
| AI 建議讀取（schema 47） | ✓ | ✓ | ✓（路徑投影） | ✓（路徑投影） |
| AI 建議建立／確認／批次確認／略過 | ✓ | ✓ | ✗ | ✗ |
| Core 檔案操作（見 4.7） | ✓ | ✓ | ✗ | ✗ |
| 密碼登入 | — | — | ✓（維持現狀） | **✗** |
| 瀏覽器配對碼兌換 | — | — | ✓ | ✓ |
| 產生配對碼、裝置管理、改密碼、網路設定 | ✓ | ✗ | ✗ | ✗ |
| 診斷、封面快取管理、`/api/timeline` | ✓ | 維持現狀 | ✗ | ✗ |
| 停止、還原 Core | 僅本機控制器 | ✗ | ✗ | ✗ |

實作時要附上「角色 × 連線方式 × 端點」的自動化測試矩陣，包含靜態檔、未知 API 路由與 OPTIONS。

### 4.4 配對字串與配對碼角色

**配對字串**（只在 Core 主機的 owner 畫面顯示，可複製）：

```text
galroon-pair:v1?addr=<host>:<port>&code=<12 hex>&fp=<SPKI SHA-256，base32 無填充，52 字元>
```

- `host` 是 IPv4、方括號包住的 IPv6（**不含 zone / scope ID**，link-local 位址在第 1 階段不支援），或主機名稱（小寫、IDNA 正規化）。
- 整串最長 256 bytes；未知欄位、重複欄位或格式錯誤一律拒絕。
- 第 1 階段只支援複製貼上，QR code 延後。
- **不使用 6 位數短碼比對**：6 位數約 20 bits，與完整 SHA-256 不等價；要做短碼得另外設計雙向確認協定。

**客戶端流程**：

1. 用 4.2 的 verifier 建立 TLS 連線；SPKI 不符就中止，**不送出配對碼**。
2. 送出配對碼，取得 bearer。
3. 把指紋、`library_id`、`device_id` 存進 `connections.json`；之後每次連線都檢查。

指紋不符時顯示明確錯誤並要求重新配對，**不提供「仍然信任」的按鈕**。

**配對碼加上角色**：

- `pairing_codes` 新增 `role` 欄位（`desktop` / `web`），走既有的遷移流程並附遷移測試。
- 角色在產生時由 owner 決定，**伺服器不接受客戶端提交的角色**。
- `web` 碼兌換後只發 cookie，不回傳 bearer；`desktop` 碼不能拿來換 cookie。
- 新增「撤銷未兌換配對碼」的 owner API。

### 4.5 桌面客戶端的傳輸方式（R5）

**採用：本機轉發代理**（2026-10-15 實機驗收後啟用本節原定備案）。IPC 原型的 5 MiB 圖片延遲與記憶體未通過下列門檻；JSON 分頁已通過。原始 IPC 方案保留於下文，實測記錄見 `LAN_PR5_REPORT.md`。

代理只綁定 `127.0.0.1` 的動態埠，每次桌面啟動使用新的隨機密鑰，接受指定桌面 Origin。`core_session` 的遠端 `token` 仍為空；另外回傳 `proxy_url` 與 `proxy_key`。NAS bearer 只留在原生端，轉送仍經 SPKI verifier，固定到選定的連線並核對 library/device。大型圖片分段轉送，套用同一大小上限、60 秒總逾時、8 請求限制與連線取消。代理不持久化密鑰，不接受任意上游或 redirect，不重送寫入。

**原始 IPC 方案**：

- 前端的網路出口已經集中在 `request()` 和 `artworkBlob()`（`api.ts:66`、`:87`）。遠端連線時，這兩個函式改呼叫一個通用的 Tauri command；由 Rust 端用 4.2 的 verifier 連到 Core。
- WebView 拿不到真正的 bearer，也不需要開本機轉發埠，CSP 不用改。
- 封面用 `tauri::ipc::Response::new(bytes)` 回傳原始 bytes（一般的 `Vec<u8>` 回傳仍會被 JSON 序列化）。status 與回應標頭另外包成一個小的信封物件回傳。
- **修改 `core_session` 的回傳契約**（`apps/desktop/src-tauri/src/main.rs:12`）：遠端連線時不再回傳 token，前端只知道「目前是遠端連線，走 IPC」。這樣「WebView 拿不到遠端 bearer」才真正成立。

**通用 command 的限制**：

- 上游固定為目前選定的連線；只接受 `/api/` 開頭的路徑，禁止 redirect 和任意 URL。
- **寫入請求不自動重試**。
- 每個請求都帶上已儲存的 library/device 標頭；前端傳來的值若與已儲存值不符，直接拒絕。
- 原樣回傳 status、錯誤 body、身分標頭和二進位內容。
- 明確設定：單一回應上限（JSON 與封面分開設）、逾時、同時進行的請求數。不沿用 `paired.rs:41` 給配對流程用的 10 秒 / 64 KiB。
- 切換連線時，取消舊請求並丟棄它們的結果。

**實測門檻**（同一台機器，連本機 Core 走 IPC 與走 `fetch` 對照，各跑 50 次）：

| 情境 | 合格條件 |
|---|---|
| 60 筆作品分頁、成員分頁 | IPC 的 p95 延遲比 `fetch` 多不超過 15 ms |
| 最大封面（以 5 MB 測試圖） | 桌面程式的尖峰記憶體增加不超過封面大小的 3 倍；p95 多不超過 30 ms |
| 8 個請求同時進行 | 全部成功，沒有請求被其他請求卡住超過逾時 |
| 取消請求、切換連線 | 舊請求的結果一律丟棄，不會出現在新連線的畫面 |
| 錯誤回應（400、401、403、409、503） | status 與 body 與直接連線完全相同 |

任一項不合格，就改用備案：本機轉發代理。代理需要每次啟動新產生的密鑰，並套用上面相同的限制。

### 4.6 瀏覽器客戶端（R2、R3）

- **區網監聽器停用密碼登入**：`/api/access/login` 在 LAN 上一律 403。理由是不讓一個長期共用的密碼在區網上反覆傳送；它本來就換不到 owner 權限。
- 改用 4.4 的 `web` 配對碼。`WebLogin.tsx` 需要新增配對碼表單。
- LAN 上發出的 cookie（含登出時清除用的那個）都加上 `Secure`。
- **瀏覽器的保證比桌面弱**：瀏覽器無法做 SPKI 綁定，使用者第一次連線時會看到自簽憑證警告。第 1 階段的做法是讓 Core 主機畫面顯示**整張憑證的 SHA-256**（瀏覽器憑證資訊顯示的是這個值，不是 SPKI），並在文件寫出逐步比對程序。威脅模型（第 5 節）分開描述桌面與瀏覽器。
- 本機 CA 或正式憑證延後到第 2 階段評估。

### 4.7 Core 檔案路徑與遠端檔案權限（R4）

**統一的「Core 路徑輸入」元件**：

- 連到遠端 Core 時，R4 列出的 6 個地方都改用這個元件：顯示「Core 主機名稱上的路徑」、讓使用者輸入文字路徑、送給 Core 驗證；不顯示本機資料夾選擇器。
- 連本機 Core 時維持現在的選擇器。
- 第 2 階段再加 Core 端目錄瀏覽 API（只列資料夾、不列檔案、只開放 owner 與 `desktop`）。

**權限模型（待使用者確認，見 10.4）**：

- 預設：`desktop` 是**受完全信任的 Core 檔案操作者**，可以新增來源、掃描、指定備份目的地、批准並執行檔案計畫，範圍是 Core 程序能存取的所有路徑。產生 `desktop` 配對碼時，畫面上要明確說明這一點。
- 備案：把檔案操作拆成獨立權限，或限制在 owner 核准的目錄。若採用，限制必須涵蓋**所有**讀寫路徑，包括 UNC，不能只限制新的目錄瀏覽 API。現有的 `canonicalize` 與防連結檢查不等於授權白名單（`plans.rs:21`）。

### 4.8 唯讀瀏覽器可見的資料（R13）

依使用者決定（10.4），投影套用到**所有** `web` session。

**做法**：

1. 列出所有 `web` 可讀的端點（從 `lib.rs` 的路由表逐條判定），每條歸入下列三類之一，清單寫進 `API_CONTRACT.md`：
   - **拒絕 `web`**：唯讀瀏覽用不到、且以路徑為主要內容的端點。至少包含 `/plans`、`/plans/{id}`、`/quarantine`、`/duplicate-checks/{id}`（`hashing.rs:50`）、`/resources/{id}/acquire-preview`（`acquire.rs:57`）、`/roots/*/remap/preview`。
   - **投影**：保留給 `web`，但移除絕對路徑。至少包含 `/roots`（只留名稱與狀態）、`/resources`、`/resources/page`、`/works/{id}/assets`（共用 `resource_catalog.rs:7` 的 `root_path`）、`/resources/{id}/members`（`membership.rs:27` 的 `path`，改為相對路徑）、`/jobs`、`/jobs/{id}`（`job_history.rs:29`、`:50` 的 `current_path`）。
   - **原樣**：回應本來就不含路徑。
2. 投影集中在一個函式，依角色套用，不在各 handler 各寫一份。
3. 錯誤訊息也要檢查：給 `web` 的錯誤文字不得包含路徑。

**測試**：

- 測試收藏的來源根目錄、整理目的地、隔離目的地、一個 UNC 形式的路徑、任務歷史中的路徑，以及一則含路徑的錯誤，全部使用同一個獨特的標記字串。
- 以 `web` 身分走訪第 1 點清單中**所有**可讀端點，回應（含錯誤回應）都不得出現該標記字串。
- 新增可讀端點時，若沒有登記到清單，測試就失敗。

**相容性例外**：這會改變 loopback 上 `web` 的回應契約。現有測試 `resource_page.rs:35` 斷言「`web` 與 `desktop` 的回應相同」，要改成「除了投影移除的欄位以外相同」，並在第 6 節與 `API_CONTRACT.md` 記錄這個契約變更。目前內附的唯讀 Web UI 若有顯示這些欄位，要一起調整。

### 4.9 多客戶端的畫面更新（R7、R12）

**第 1 階段**：

- 新增 `catalog_change_seq`（需要 schema 遷移），由 trigger 或寫入程式遞增。
- 前端在頁面可見時，每 10 秒查一次；有變化就重新載入目前畫面。
- 衝突時顯示「這筆資料已被其他裝置修改，請重新載入」，不顯示一般錯誤。

**必須遞增序號的變更**（實作時先產出完整的「畫面資料 → 寫入的表或事件」對照表，放進 PR 說明）：

| 類別 | 至少涵蓋 |
|---|---|
| 收藏本體 | works、editions、resources、`files`、resource-file 與 work-edition 等關聯表 |
| 個人分類 | 標籤、自訂標籤、手動清單、智慧清單定義 |
| 修正與覆寫 | 關係修正、欄位覆寫（entity overrides） |
| 來源 | roots 的新增、移除、設定，以及**來源上線/離線**（目前只更新 `smart_catalog_state`，`smart_updates.rs:49`） |
| 背景結果 | 自動匹配寫入、**smart snapshot 計算完成**（`smart_snapshots.rs:87`）、issues、plans 狀態 |
| 收藏層級 | 收藏改名（`context.library.revision`） |

**不得遞增序號的寫入**（否則會不停觸發重新載入）：`sessions.last_used`、`access_attempts`、各種快取與租期、診斷記錄。

**驗收**：自動匹配完成、smart 計算完成、來源離線、只改關聯表，這四種情況都要讓另一台客戶端在 10 秒內更新；只讀取不寫入時，序號不得變化。

**第 2 階段**：改成 long-poll 或 SSE 推送。

### 4.10 憑證失效與還原（R11）

- Core 對「token 不存在、已撤銷、已過期」回 401，並附上可辨識的 `code: credential_invalid`；一般的「沒帶憑證」維持現狀。
- 桌面端收到 `credential_invalid` 時進入「需要重新配對」狀態：保留目前選取的連線、停止自動重試，畫面提示重新配對。
- 文件寫明：**還原收藏會讓所有已配對裝置與瀏覽器失效**，必須重新配對。保留的 TLS 指紋不代表 bearer 還有效。

### 4.11 嘗試次數限制（R1）

- 三層上限：**每個 IP**（嚴格）、**每個 IPv6 /64**（較寬）、**全域**（最寬）。`kind` 欄位本來就是 TEXT，可以存複合 key，不一定需要遷移。
- 只信任 socket 的對端位址（axum `ConnectInfo`），不讀任何 `X-Forwarded-*`。
- IP 先正規化（IPv4-mapped IPv6 轉回 IPv4）。來源紀錄要有最大數量與淘汰規則。
- `login_gate` 只保護密碼登入（argon2 很耗 CPU）。LAN 停用密碼登入後，它只會在 loopback 上被用到。配對碼兌換不經過它。

### 4.12 Core 主機的生命週期（R9、R14）

- Core 主機上按「停止 Core」時，顯示「最近 10 分鐘內有使用的工作階段」清單（依 `last_used`，不宣稱精確的在線數），再要求確認。現有「有進行中任務就拒絕停止」的規則保留。
- **自動啟動延後**：它必須跟著 `active-library.json`，不能記住還原前的資料目錄，而這段邏輯目前在桌面端 controller（`controller.rs:6`）。第 1 階段文件寫明「Core 主機需先開啟 Galroon」。
- **操作者紀錄延後到第 2 階段**：任務與計畫加上建立者、最後操作者的 session id。第 1 階段文件寫明「已配對的桌面可以控制彼此建立的任務」。

### 4.13 Tailscale 與反向代理

- 第 1 階段支援的方式：Core 的區網監聽器直接綁定 Tailscale 介面位址（`100.64.0.0/10`），客戶端用一般的配對字串連線。
- **不支援** `tailscale serve` 或任何反向代理。原因是代理會把遠端請求變成 loopback 請求，等於取得 loopback 的權限（密碼登入，甚至 owner bearer）；而改成信任 `X-Forwarded-*` 也不安全。若之後要支援，必須另外提供「代理專用入口」，仍然套用 LAN 規則。

### 4.14 延後的項目

- **mDNS 自動發現（第 2 階段）**：只在區網模式開啟時廣播 `_galroon._tcp`；TXT 只放 `device_id` 和指紋前 16 字元，不放收藏名稱。信任仍然只來自配對字串。
- **客戶端路徑對應（未排程）**：目前沒有需要在客戶端碰實體檔案的功能，等有需求再設計。
- **MCP 連遠端 Core（未排程）**：第 1 階段 MCP 仍只連本機。
- **（已移到 4.15，第 1 階段）無畫面 Core**：讓 `galroon-core` 可在 Linux 上跑（控制通道改用權限 0600 的 Unix socket）；把 `core_control` 範例整理成正式的 `galroon-core owner pair` 命令；改用系統的 `7zz` / `7z`；提供 Docker image；偵測到資料目錄在網路檔案系統上就拒絕啟動。

### 4.15 NAS 部署（Synology DS920+）

使用者確認最終成品會把 Core 部署在 NAS 上（2026-10-12），因此無畫面 Core 從第 3 階段提前到第 1 階段，並安排 N1–N3 三個 NAS 驗收里程碑（第 7、8 節）。

**實測環境**（2026-10-12 以 SSH 唯讀檢查）

| 項目 | 實測值 |
|---|---|
| 機型與系統 | DS920+，DSM 7.4.1（build 90080），x86_64，4 核心 |
| 記憶體 | 共 3.7 GB；其他服務已在使用，**實際可用約 1.3 GB** |
| Docker | Container Manager，Docker 24.0.2；NAS 上已有其他 container，不得影響它們 |
| 收藏 | `/volume1/Backup/Galgame`，768 個項目、1,107 個子資料夾 |
| inotify | `fs.inotify.max_user_watches` = 8,192 |
| 連接埠 | 14800、14830 在 host 網路上未被使用 |

**執行方式**

- 以 Docker container 執行，container 名稱 `galroon`，使用 **host 網路模式**：container 的 loopback 就是 NAS 的 loopback，區網監聽器直接綁定 NAS 的區網位址。
- 重啟策略 `unless-stopped`，NAS 開機後自動啟動。Windows 主機的自動啟動仍照 4.12 延後。
- **停止**：container 收到 SIGTERM 時，走與 Windows 控制通道 `stop` 相同的流程（PR 1 的 `Services` 與 `serve_listeners`）：有進行中的任務就先暫停、drain HTTP、停背景工作、checkpoint。compose 的 `stop_grace_period` 設為足夠完成這個流程（先定 60 秒，N1 實測後調整）。
- **記憶體**：container 上限 1 GB。N1 量測實際尖峰，合格條件見第 8 節。

**程式結構**（PR N1a）

- 不能只拿掉 `main.rs:7` 的平台限制，因為 `local_core` 整個模組只在 Windows 編譯（`lib.rs:67`）。要把它拆成兩部分：
  - **共用生命週期**：初始化、`Services`、`serve_listeners`、停止與 drain、checkpoint。
  - **平台控制通道**：Windows named pipe（現有）、Unix domain socket（新增）。
- Windows 的現有入口與參數（`--data-dir`、`--device-dir`）維持不變。
- Linux 入口：`galroon-core serve --data-dir <dir> [--device-dir <dir>] [--loopback-port <port>]`。

**控制通道的安全前提**

- container 以**專用的非 root uid** 執行（不用 0）。
- socket 放在資料目錄內一個 **0700 的私有子目錄**，socket 本身 0600；這個目錄不得掛載給其他 container。
- **雙向驗證**：
  - server 以 `SO_PEERCRED` 確認 client 的 uid 與自己相同。
  - client 確認 socket 檔的擁有者是預期的 uid，並以 `SO_PEERCRED` 確認 server 程序的 uid。
  - 預期的 uid 一律取自程序自己的 euid 或可信的設定，不取自對方提供的資料；**驗證完成之前不交換任何敏感資料**（例如 owner token）。
  - 這**不等同** Windows 版檢查 server 執行檔路徑（`local_core.rs:25`）：`SO_PEERCRED` 只能確認 uid，無法分辨同 uid 的冒充程式。這在下面的信任範圍內是可以接受的。
- **殘留的 socket**：啟動時先取得資料目錄的程序鎖（現有的 `core.lock`）；拿到鎖之後，才刪除擁有者相符的舊 socket 檔並重建。拿不到鎖就直接失敗。
- **信任範圍要寫清楚**：信任「能存取這個 socket 的同 uid 程序」。不防護 NAS 的 root 或 Docker 管理員；能執行 `docker exec` 的人本來就等同 root。只能連到 loopback 埠的程序**不會**因此取得 owner，因為仍然需要 bearer（`access.rs:42`）。

**固定 loopback 埠**

- 無畫面模式使用固定埠（預設 `14800`），這是 Linux 入口的特例；Windows 仍由作業系統分配。
- 埠被佔用時**直接啟動失敗**並說明原因，不得改用其他位址，更不得改綁 `0.0.0.0`。
- SSH 通道接到 loopback 埠時，`Host: 127.0.0.1:<port>` 通過現有檢查，這是 N1 的測試方式。若通道接到**區網監聽器**，請求仍標記為 `Lan`，不會因為 Host 是 loopback 就取得 loopback 權限。

**owner 命令列**

| 指令 | 作用 |
|---|---|
| `galroon-core owner setup` | 設定 owner 密碼，從 stdin 讀取，不放在命令列參數 |
| `galroon-core owner pair --name <名稱> [--role desktop\|web]` | 產生配對碼；PR 3 之後輸出 4.4 的配對字串 |
| `galroon-core owner devices` / `owner revoke <id>` | 列出、撤銷已配對的裝置 |
| `galroon-core owner network ...` | 區網設定（PR 2、PR 3 之後） |
| `galroon-core owner session` | 開發測試用：輸出含 owner token 的 session JSON，語意與 Windows 控制通道的 `session` 相同，只給通過雙向驗證的呼叫者 |
| `galroon-core restore --from <備份> --to <新資料目錄>` | **離線**還原：Core 停止時，以一次性 container 執行（見搬移流程），呼叫 `backup::restore_new` |

在 NAS 上以 `docker exec galroon galroon-core owner ...` 執行。owner 仍然**只能從 Core 主機本機**取得。

**目錄與權限**

- 收藏資料夾以**與 NAS 相同的路徑**掛進 container（`/volume1/Backup` → `/volume1/Backup`），讓路徑和使用者在 DSM 看到的一致。
- **測試期間一律唯讀掛載（`:ro`）**。整理、隔離等寫入功能只在 NAS 上獨立的測試資料夾驗證（N3）。
- Core 資料目錄：`/volume1/docker/galroon/data`（NAS 本機磁碟）。資料目錄在網路檔案系統上（`statfs` 判斷 NFS、CIFS/SMB、FUSE）時拒絕啟動。

**平台差異**

- **7-Zip**：目前只找 `.exe` 或環境變數 `GALROON_7ZIP`（`acquire.rs:12`）。image 安裝 `7zz` 並設定 `GALROON_7ZIP=/usr/bin/7zz`；另外在 Linux 上新增依序尋找 `PATH` 中 `7zz`、`7z` 的邏輯。都找不到時，封存功能在 capability 中標示為不可用。
- **檔案監看的備援**：目前 watcher 失敗只會重試並提示手動掃描（`watching.rs:90`）。N1a 要新增：watcher 無法建立或 watch 配額用盡時，**自動改為定期掃描**（預設每 15 分鐘），並在診斷與來源狀態中顯示原因。附「watch 配額耗盡」的測試。
- **檔名**：掃描目前用 `to_string_lossy()`（`scan.rs:99-110`），非 UTF-8 檔名會被改寫。N1a 改為：遇到非 UTF-8 檔名時記錄為來源問題並略過，不以改寫後的名稱寫入資料庫。大小寫、NFC/NFD 與 DSM 的 rename、ACL 行為在 N3 的測試資料夾驗證。

**建置與部署**（PR N1b）

- `Dockerfile`：多階段建置。builder 階段建置 `galroon-core` 與前端，前端產物放到執行檔旁的 `web/`（`lib.rs:164` 從那裡提供 Web UI）；runtime 為 debian-slim，含 `7zz` 與 CA 憑證，以專用 uid 執行。
- compose 檔：`network_mode: host`、`restart: unless-stopped`、`stop_grace_period`、記憶體上限、`user:`、收藏 `:ro` 掛載、資料目錄掛載。
- `scripts/nas/`：建置 image、以 `docker save` / `docker load` 經 SSH 傳到 NAS、啟動與停止。不使用公開的 image registry。
- CI 新增 Linux 的 `cargo build` 與 `cargo test -p galroon-core --lib`。

**從現有收藏搬到 NAS**（N3，需要 PR N1d）

1. 在現有 Core 建立備份。
2. 停止 NAS 上的 Core。
3. container 已停止，不能用 `docker exec`。改用同一個 image 啟動一次性 container 執行還原：`docker compose run --rm --no-deps -v <備份所在目錄>:/backup:ro -v <新資料目錄>:/data-new galroon galroon-core restore --from /backup/<備份> --to /data-new`，以與正式 container 相同的 uid 執行。
4. 把 compose 的資料目錄改指向新目錄，啟動 Core。
5. 用既有的來源重新對應（`/roots/{id}/remap`）把舊路徑改成 NAS 路徑。從 Windows 搬過來時，資料庫裡的相對路徑使用反斜線（`roots.rs:14`），重新對應時要正規化成 `/`；這需要程式修改，放在 PR N1d。
6. 檢查作品、標籤、清單、手動修正都保留。

**跨資料庫的匹配結果比較**（由匹配計畫的 Phase 1b 提供，N1 的前置條件）

`match_replay --baseline` 目前要求 resource ID 與相對路徑都相同（`examples/match_replay.rs:38`），但 NAS 上的全新掃描會產生新的 ID（`scan.rs:134`）。匹配計畫（`docs/plans/2026-10-12-matching-accuracy-and-ai.md`）的 Phase 2 也需要同樣的能力，因此由匹配計畫以 **Phase 1b** 實作（2026-10-12 與該計畫協調確定），本計畫不另開 PR：

- ID 不同時，以「套用 `--root-map <舊>=<新>` 後的根路徑＋相對路徑」的 NFC 形式識別同一個資源；任一側出現重複 key 就失敗（退出碼 2）；只出現在一側的資源列為 added / removed。
- **actual**（已儲存的匹配結果）與 **replay**（重新排序的候選）分開報告，各有 gained / lost / changed。
- `--fail-on changed,false,lost,missing`：N1 使用全部四項。
- 答案檔沒涵蓋的強匹配列為 `unreviewed`，供人工核對。
- 摘要列出 added、removed、gained、lost、changed、false、missing 的數量。

## 5. 威脅模型

| 攻擊者 | 桌面客戶端 | 瀏覽器客戶端 |
|---|---|---|
| 區網上被動竊聽 | TLS | TLS |
| 區網上中間人 | SPKI 綁定，不符就中止，沒有「仍然信任」 | **僅靠使用者第一次連線時人工比對憑證指紋**；略過警告就沒有保護 |
| 暴力嘗試配對碼 | 48 位元配對碼、5 分鐘過期、一次性，加上分層的嘗試次數限制 | 同左 |
| 讓其他人無法登入或配對 | 分層嘗試次數限制；LAN 上沒有密碼登入 | 同左 |
| 竊取瀏覽器登入密碼 | 不適用 | LAN 上停用密碼登入 |
| 本機惡意程式取得遠端憑證 | WebView 拿不到 NAS bearer（原生轉發；只取得短期本機密鑰）；bearer 在 Windows 認證管理員 | 不適用 |
| 惡意網頁做 DNS rebinding | Host 限制，加上憑證不符 | 同左 |
| 被偷走的 `desktop` 憑證 | owner 可撤銷；**持有者可以做所有 Core 檔案操作**（4.7） | 不適用 |
| 從反向代理繞進 loopback 權限 | 不支援反向代理（4.13） | 同左 |
| NAS 上的其他使用者或 container 取得 owner 權限 | 專用非 root uid、0700 私有目錄內的 0600 socket、不掛載給其他 container、雙向 `SO_PEERCRED` 驗證。**信任**能存取該 socket 的同 uid 程序；**不防護** NAS 的 root 與 Docker 管理員 | 同左 |
| 誤綁公網位址 | 啟動時拒絕非私有網段 | 同左 |

## 6. 相容性

- API 版本維持 `1`。`/api/context` 新增 `capabilities.lan_access`，並依實際連線方式回報 `transport`。
- 需要 schema 遷移的有：`pairing_codes.role`（4.4）、`catalog_change_seq`（4.9）。都走既有的遷移流程並附測試；兩者若在不同 PR 平行開發，合併前要協調遷移編號。匹配計畫（`docs/plans/2026-10-12-matching-accuracy-and-ai.md`）第 3 階段的 `match_suggestions` 遷移與 `suggest_match` MCP 寫入端點排在本計畫 PR 1 到 PR 4 合併之後，使用其後的下一個遷移編號，並納入 4.3 的權限矩陣。網路設定放裝置目錄，不進 schema。
- **契約例外**：`web` 角色的回應會移除絕對路徑，部分端點改為拒絕 `web`（4.8）。這是有意的契約變更，記錄在 `API_CONTRACT.md`。
- `connections.json` 新增 `fingerprint`。舊的項目沒有這個欄位，只能連 loopback。
- 沒開區網模式時：**既有行為與 API 契約的回歸測試全部通過**，而且不會開任何新的監聽埠。（不要求回應逐位元組相同，因為回應本來就含隨機 request ID 和 instance ID。）

## 7. 分階段與 PR 順序

**第 1 階段**（區網模式的開關到最後一個 PR 才出現在 UI）

1. **Core 程序生命週期重構**（4.1）：背景工作只啟動一次、多 listener 共同停止、`Transport` wrapper、middleware 順序。行為不變，只有 loopback。
2. **Core 區網監聽器**（4.1、4.2、4.3、4.11）：`network.json`、私有網段檢查、TLS、依 `Transport` 的權限規則、HTTPS Origin、`Secure` cookie、分層嘗試次數限制、`context.transport`。附角色 × 連線方式 × 端點測試矩陣。只能用測試設定開啟。
3. **Core 配對與憑證狀態**（4.4、4.10）：配對碼 `role` 遷移、配對字串、撤銷未兌換配對碼、`credential_invalid`、owner 網路設定 API。
4. **Core 資料邊界與變更序號**（4.8、4.9）：`web` 收藏投影、`catalog_change_seq` 遷移。
5. **桌面傳輸**（4.2、4.5）：SPKI verifier、IPC 傳輸與實測、`connections.json` 指紋、「需要重新配對」狀態。
6. **前端**（4.6、4.7、4.9、4.12）：Core 路徑輸入元件（6 個地方）、瀏覽器配對碼表單、衝突訊息、變更輪詢、停止前確認。
7. **開放與文件**：UI 上的區網模式開關、`SUPPORT.md`、`USER_GUIDE.md`、`API_CONTRACT.md`、瀏覽器憑證比對步驟、Tailscale 說明、NAS 部署與搬移說明。

**NAS 軌道**（4.15）

- **PR N1a：Linux 執行與控制通道**：拆出共用生命週期與平台控制通道、`serve` 入口、Unix socket 控制通道與雙向驗證、殘留 socket 處理、固定 loopback 埠、SIGTERM 停止流程、網路檔案系統拒絕啟動、owner 命令列（`setup`、`pair --role desktop`、`devices`、`revoke`、`session`）、CI 的 Linux 建置與測試。
- **PR N1b：Docker 與部署**：`Dockerfile`（含前端 `web/`）、compose 檔、`scripts/nas/`。可執行的驗收依賴 N1a。
- **PR N1c：掃描、封存與監看的 Linux 相容性**：Linux 的 7-Zip 尋找、watcher 定期掃描備援（含錯誤注入測試）、非 UTF-8 檔名處理。
- **PR N1d：離線還原與跨平台重新對應**：`restore` 指令、Windows 反斜線相對路徑在重新對應時正規化。**只阻擋 N3**，不阻擋 N1。
- **里程碑 N1**：PR N1a、N1b、N1c 與匹配計畫 Phase 1b 合併後，在 NAS 上以唯讀收藏跑一次完整掃描與匹配（第 8 節）。
- **里程碑 N2**：PR 2、PR 3、**PR 5** 合併後（現有桌面只接受 HTTP loopback，`paired.rs:32`，要等 PR 5 才能透過區網配對），NAS 上的 Core 開啟區網監聽器，桌面透過配對連線。owner 命令列在 PR 3 補上 `pair --role web` 與 `network`。
- **里程碑 N3**：第 1 階段結束時，第 8 節的手動驗收全部在 NAS 上進行，包含搬移流程。

**依賴與可平行的部分**：

- PR 1 必須最先合併。
- PR 2 與 PR 3：在 PR 1 固定 `Transport` 和 `network.json` 的介面後，可以平行開發。
- PR 4：可以獨立開發；與 PR 3 都有 schema 遷移，合併前協調遷移編號。
- PR 5：SPKI verifier 和 IPC 傳輸的原型（含實測）可以先做；整合需要 PR 2、PR 3。
- PR 6：Core 路徑輸入元件可以先做；其餘部分需要 PR 3 到 PR 5。
- PR N1a 到 N1d：在 PR 1 合併、並先固定共用生命週期、控制協定、CLI 參數、目錄與 loopback 設定的介面後，可以和 PR 2、PR 3 平行。N1b 的實際部署測試要等 N1a；N1c、N1d 建立在 N1a 的 Linux 入口上。
- PR 7 最後合併。

**第 2 階段**：mDNS、推送通知、Core 端目錄瀏覽、操作者紀錄、自動啟動、瀏覽器正式憑證評估、IPv6 link-local（需要客戶端選擇介面）。

**第 3 階段**：其他平台的 Core 主機（ARM 機型、其他 NAS 品牌）、公開的 image 發佈。

## 8. 測試與驗收

**自動測試**

- 區網模式關閉：既有回歸測試全部通過，沒有新的監聽埠。
- 背景工作在有兩個 listener 時只啟動一次；停止時兩個 listener 都會 drain。
- 綁定公網位址與 IPv6 link-local 被拒絕；`100.64.x.x`、`192.168.x.x`、`fd00::/8` 被接受。
- 角色 × 連線方式 × 端點矩陣（4.3），包含靜態檔、未知 API 路由、OPTIONS。
- 沒有 `Transport` 標記的請求被拒絕。
- 指紋不符時，客戶端在送出配對碼**之前**就中止（用兩組憑證的測試 Core 驗證）；握手簽章錯誤時也中止。
- 配對字串：方括號 IPv6、帶 zone ID 的位址被拒、超長、未知欄位、重複欄位。
- `web` 碼換不到 bearer；`desktop` 碼換不到 cookie。
- 嘗試次數：來源 A 被擋時，來源 B 仍可配對；IPv4-mapped 位址與原 IPv4 計為同一來源。
- IPC 傳輸：路徑範圍外被拒、寫入不重試、錯誤 status/body 原樣回傳、封面為原始 bytes、身分標頭不符被拒。
- 還原收藏後，遠端客戶端進入「需要重新配對」狀態，不會一直重試。
- `web` 角色走訪所有可讀端點，回應與錯誤都不含標記路徑（4.8）。
- 兩個 `desktop` 同時編輯同一筆資料：後送出者收到衝突，資料不被覆蓋；另一台在 10 秒內看到更新。

**NAS 里程碑 N1（Core 在 DS920+ 上，收藏唯讀掛載，全新資料目錄）**

操作方式：

1. 在 NAS 上以 compose 啟動 Core，執行 `docker exec galroon galroon-core owner session` 取得 session JSON，存到 Mac。
2. 在 Mac 上開 SSH 通道：`ssh -N -L 127.0.0.1:14800:127.0.0.1:14800 galroon-dev@192.168.50.154`。
3. 以 `GALROON_DEV_SESSION=<session 檔> npm run dev` 開啟開發版 UI，新增來源 `/volume1/Backup/Galgame`，執行掃描與自動匹配。（一般瀏覽器登入只有 `web` 權限，不能掃描。）

合格條件：

- **基準要可重現**：舊的 `nas-2026-10-11-v6`（561 個匹配）是 algorithm 5 的結果，不能直接當基準。N1 前以**與 NAS 相同的 commit**、同一份收藏、同樣的匹配設定，在 Mac 上透過 SMB 重建基準，記錄 commit、收藏檔案清單的雜湊與設定。
- 比較命令（匹配計畫 Phase 1b）：`match_replay <NAS 收藏庫的副本> --baseline <Mac 基準報告> --root-map /Volumes/Backup/Galgame=/volume1/Backup/Galgame --answers ~/GalroonTestData/nas-answers.json --fail-on changed,false,lost,missing --out n1.json`。報告存到 `~/GalroonTestData/nas-n1-<日期>/`。
- 合格：命令以 0 結束（**錯誤匹配為 0**，沒有 `changed`、`lost`、`missing`、重複 key）；added / removed 只能是測試期間新增或移除的資料夾；`unreviewed` 清單全部經人工核對。
- 收藏檔案完全沒有被寫入（唯讀掛載保證；另比對執行前後的檔案清單與修改時間）。
- **記憶體**：初始預算 **600 MiB**，第一次實測後再確認門檻。量測方式：
  - 指標：container cgroup 的記憶體尖峰（DSM 7.4 的 cgroup v1 用 `memory.max_usage_in_bytes`，v2 用 `memory.peak`），涵蓋 `7zz` 等子程序；不使用 `docker stats` 的顯示值，因為它扣除了部分快取。
  - 期間：從 container 啟動、掃描到自動匹配完成。
  - 同時記錄：swap 使用量、NAS 主機剩餘記憶體、是否發生 OOM 或異常重啟（不得發生）。
- 記錄掃描與匹配所花的時間。
- 在 NAS 上新增一個遊戲資料夾，檔案監看能偵測到。
- **監看備援**：不在共用的 NAS 上調低 watch 配額，以免影響其他服務。改用 PR N1c 提供的錯誤注入開關（只在測試設定中生效）讓 watcher 啟動失敗，確認自動改為定期掃描、顯示原因，而且**定期掃描確實找到新增的資料夾**。真正的配額耗盡在 CI 或隔離的 VM 中測試。
- `docker stop galroon` 走完正常停止流程（有進行中的任務時先暫停），重新啟動後資料完整；重新啟動 NAS 後 Core 自動啟動。
- NAS 上其他 container 不受影響。

**NAS 里程碑 N2**：桌面程式透過區網配對連到 NAS 上的 Core，瀏覽與編輯標籤；撤銷後立即失效。

**手動驗收（N3：Core 在 NAS 上，客戶端為 Windows 桌面與瀏覽器）**

- 從現有的收藏搬到 NAS（4.15 的搬移流程），作品、標籤、清單、手動修正全部保留。
- 整理、隔離等寫入功能只在 NAS 上獨立的測試資料夾驗證，包含 Synology ACL 對權限的影響。

- 配對、瀏覽、編輯標籤與清單，兩邊都看得到更新。
- 從遠端用 Core 路徑輸入新增來源、整理、隔離，檔案在 Core 主機上正確處理。
- 中斷網路後重連；Core IP 改變後重新輸入位址（指紋不變，不需重新配對）。
- 撤銷裝置後，該裝置立即失效並顯示「需要重新配對」。
- 瀏覽器照文件步驟比對憑證指紋後登入，畫面上看不到 Core 主機路徑。
- 透過 Tailscale 位址從外部網路連線。
- 遵守「零錯誤匹配」與現有使用者故事驗收規則；區網功能不能改變匹配結果。

## 9. 實作時的審查方式

每個 PR 完成後，Claude 依下列項目審查，全部通過才進行下一個：

1. diff 只包含該 PR 範圍內的變更。
2. Mac 上 `cargo test -p galroon-core -p galroon-mcp --lib --bins --tests`、`cargo clippy`、`npm test` 全部通過。
3. 動到 Windows 專屬程式碼（`local_core.rs`、桌面端）時，在 Windows 測試機上編譯並跑對應測試。
4. 第 8 節中屬於該 PR 的測試都已加入。

## 10. 審查紀錄

### 10.1 第 1 輪（Codex `gpt-6-astra`，2026-10-12）：判定 `request_major_revision`

已更正的現況描述：B1 措辭、B4（真正問題是 HTTPS Origin）、B7（目前沒有暴露）、R2（密碼只換得到 `web`）、R5（WebView2 有憑證事件可掛接）、R6（本機程式可透過 `request()` 取得 session）、R7（可能一直不更新）、R8、R9、3.1 的配對碼撤銷、身分鎖定與 CAS 範圍。新增 B8、B9、R10 到 R14。

### 10.2 第 1 輪意見的處理

| 審查意見 | 處理 |
|---|---|
| 信任邊界不完整（HTTPS Origin、middleware 順序、瀏覽器信任、反向代理） | 採納：4.1、4.3、4.6、4.13；第 5 節拆開桌面與瀏覽器 |
| 檔案操作只處理新增來源 | 採納：4.7 涵蓋 6 個地方，並明訂遠端權限模型 |
| 背景工作會重複啟動、停止漏掉 listener | 採納：4.1，並排為第一個 PR |
| 還原或撤銷後會無限重試 | 採納：4.10 |
| 沒有通用的變更序號 | 採納：4.9 新增 `catalog_change_seq` |
| `web` 看得到實體路徑 | 採納：4.8 |
| 沒有操作者紀錄、在線數不精確 | 部分採納：停止前顯示「最近使用」；操作者紀錄延後到第 2 階段並寫進文件 |
| IPC 不必改每個呼叫點 | 採納：4.5 改以 IPC 為主方案，代理為備案 |
| 不要用 6 位數短碼取代指紋 | 採納：4.4 |
| 配對字串需處理 IPv6 | 採納：4.4 定義文法 |
| pinning 必須保留握手簽章驗證 | 採納：4.2 |
| 配對碼要有伺服器決定的角色 | 採納：4.4 新增 `role` 欄位 |
| IPv6 限流 | 採納：4.11 三層上限 |
| 網路設定需要 `device_dir`、還原後要重新配對、自動啟動要跟 active pointer | 採納：4.1、4.10、4.12（自動啟動延後） |
| PR 順序 | 採納：第 7 節重排，開關最後才出現 |
| 「逐位元組相同」不可行 | 採納：第 6 節改為回歸測試 |
| MCP、context 寫死 loopback | 採納：B8、B9；MCP 第 1 階段不支援遠端 |

### 10.3 第 2 輪（Codex `gpt-6-astra`，2026-10-12）：判定 `approve_with_changes`

第 1 輪的 17 項處理中，有 3 項第 2 輪認為未完整處理，已修改：

| 審查意見 | 處理 |
|---|---|
| 變更序號涵蓋範圍不完整（`files`、關聯表、覆寫、來源上下線、smart 計算完成等） | 4.9 改為列出必須與不得遞增的類別，並新增驗收條件 |
| `web` 投影漏了多個端點；套用到所有 `web` 會違反現有「web 與 desktop 回應相同」的測試 | 4.8 改為三類清單（拒絕/投影/原樣）、標記字串走訪測試；相容性例外寫入 4.8 與第 6 節 |
| IPv6 link-local 的 scope ID 屬於客戶端，不能由 Core 產生 | 第 1 階段不支援 link-local（4.1、4.4），移到第 2 階段 |
| IPC 的實測門檻無法驗收；`Vec<u8>` 仍會被 JSON 序列化；`core_session` 仍回傳 token | 4.5 新增量化門檻、改用 `ipc::Response`、修改 `core_session` 契約 |
| middleware 組裝方式要寫明 | 4.1 新增組裝方式與 405/HEAD 測試 |
| R10 措辭：目前沒有重複啟動 | 已更正 |
| PR 之間的依賴與可平行部分 | 第 7 節新增 |


### 10.4 使用者決定（2026-10-12）

1. **遠端 `desktop` 的檔案權限**（4.7）：**完全信任**。產生 `desktop` 配對碼時，畫面明確說明持有者可以對 Core 能存取的所有路徑做檔案操作。
2. **`web` 投影的範圍**（4.8）：**套用到所有 `web` session**，不分 loopback 或 LAN。只維護一種投影，比較單純。
3. **瀏覽器信任**（4.6）：第 1 階段接受**自簽憑證＋人工比對憑證指紋**，正式憑證留到第 2 階段評估。
4. **NAS 部署**（2026-10-12）：最終成品把 Core 部署在 NAS（DS920+）上。無畫面 Core 提前到第 1 階段，新增 N1–N3 里程碑（4.15）。測試期間收藏一律唯讀掛載。

### 10.5 第 3 輪（Codex，2026-10-12）：判定 `request_major_revision`

第 3 輪只審查 NAS 部署軌道（第 4 版），意見已全部併入第 5 版：

| 審查意見 | 處理 |
|---|---|
| 不能只拿掉平台限制，要拆出共用生命週期與平台控制通道 | 4.15「程式結構」 |
| Linux 上找不到 `7zz` | 4.15「平台差異」：環境變數＋`PATH` 尋找 |
| watcher 失敗沒有定期掃描備援 | 4.15「平台差異」，N1 驗收加入配額耗盡測試 |
| 非 UTF-8 檔名會被 `to_string_lossy()` 改寫 | 4.15「平台差異」 |
| 同 uid 不足以區分 container；Windows 版有 client 驗證 server | 4.15「控制通道的安全前提」：專用 uid、私有目錄、雙向驗證、殘留 socket、寫明信任範圍 |
| 固定埠衝突、SSH 通道與 LAN 的關係 | 4.15「固定 loopback 埠」 |
| `match_replay` 無法比較兩次獨立掃描 | 由匹配計畫 Phase 1b 實作（第 4 輪後協調確定） |
| N2 還依賴 PR 5 | 第 7 節 |
| SIGTERM 停止、Web UI 打包 | 4.15「執行方式」、「建置與部署」 |
| NAS 沒有還原入口；Windows 反斜線路徑 | 離線 `restore` 指令與搬移流程，放在 PR N1d |

### 10.6 第 4 輪（Codex，2026-10-12）：判定 `approve_with_changes`

| 審查意見 | 處理 |
|---|---|
| PR N1a 範圍太大 | 拆成 N1a（執行與控制通道）、N1c（Linux 相容性）、N1d（還原與重新對應，只阻擋 N3） |
| `SO_PEERCRED` 不等同 Windows 檢查執行檔 | 4.15 修正措辭；預期 uid 取自 euid，驗證前不交換敏感資料 |
| 600 MB 沒有量測定義 | 第 8 節定義 MiB、cgroup 指標、期間、OOM 與 swap 紀錄；首次實測後確認門檻 |
| 561 是 algorithm 5 的結果，基準不可重現 | 第 8 節改為以同一 commit 重建 Mac 基準，並寫出比較命令 |
| 不應在共用 NAS 調低 watch 配額 | 改用錯誤注入；真正耗盡在 CI 或 VM 測試 |
| container 停止後不能 `docker exec` 還原 | 改用一次性 `docker compose run --rm` |

### 10.7 執行分工

Claude 負責規劃、撰寫每個 PR 的任務說明和審查；Codex 負責實作。每個 PR 在獨立分支完成，Claude 審查並跑完測試後才進行下一個。
