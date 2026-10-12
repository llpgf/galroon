# 區網 Core 規劃書：一個 Core、多台客戶端

狀態：**草案，待交叉審查**（2026-10-12）
範圍：讓同一區網內的多台機器連到同一個 Galroon Core，共用同一份收藏、標籤、清單與分類資料。

> **給審查者**：請先讀「9. 審查重點」。本文每一項現況描述都附了檔案與行號，請對照程式碼確認，不要只看文字。發現描述與程式碼不符、設計有漏洞，或有更簡單的做法，請直接指出。

---

## 1. 目標與非目標

**目標**

- 一台機器跑 Core，區網內其他 Windows 桌面程式透過配對連進來，可讀可寫（`desktop` 角色）。
- 其他裝置的瀏覽器可以唯讀瀏覽（`web` 角色）。
- 不在家時，可以透過 Tailscale 或 WireGuard 這類虛擬內網連線，Core 不需要額外功能。
- 預設行為完全不變：沒開啟區網模式時，Core 仍然只綁定 loopback。

**非目標**

- 多台機器同時開同一個 SQLite 檔（例如放在 SMB 上）。現在不支援，以後也不支援。
- 多份資料庫互相同步或合併。
- 直接對公網開放 Core、做連接埠轉發，或類似 Roon ARC 的雲端中繼。
- 從遠端啟動遊戲（目前產品本來就沒有啟動遊戲的功能）。

## 2. 為什麼選「單一 Core」

| 做法 | 結論 | 原因 |
|---|---|---|
| 共用資料庫檔 | 否決 | SQLite 在網路檔案系統上同時寫入不安全；文件已明確列為不支援（`docs/user/SUPPORT.md`「Core storage」）。 |
| 每台各一份資料庫再同步 | 否決 | 現有的 revision CAS、可續跑任務、需審核的檔案操作，都假設只有一個寫入者。改成同步等於重寫，衝突也沒有好解法。 |
| **單一 Core＋多客戶端（Roon 模式）** | **採用** | 現有的身分、配對、權限和 revision 機制本來就是為多客戶端設計的，只是被鎖在 loopback。 |

## 3. 現況（請逐條核對）

### 3.1 已經具備、可沿用的部分

- **三種身分**：本機 owner（控制通道 token）、配對桌面 `desktop`（bearer，90 天）、瀏覽器 `web`（cookie，12 小時，唯讀）。見 `crates/core/src/access.rs` 的 `authenticate`、`WEB_LIFETIME`、`DEVICE_LIFETIME`。
- **配對**：只有 owner 能產生配對碼。配對碼為 12 位 hex、5 分鐘過期、只能用一次、可撤銷（`access.rs` 的 `create_pair` / `redeem`）。
- **web 角色唯讀**：在 middleware 層攔截，不是靠隱藏 UI 按鈕（`access.rs` 的 `guard`）。
- **連線身分鎖定**：`x-galroon-library` / `x-galroon-device` 標頭不符時回 409 `collection_changed`（`access.rs` 的 `guard`）。
- **桌面端已支援「配對連線」**：連線資料存在 `connections.json`，憑證放 Windows 認證管理員；身分改變時不會自動改連別的 Core（`apps/desktop/src-tauri/src/paired.rs`）。
- **寫入衝突保護**：一般編輯都帶 revision，過期就失敗（`docs/development/API_CONTRACT.md`「Mutations, retries and jobs」）。
- **檔案操作在 Core 那台執行**：預覽 → 審核 → 執行，都由 Core 處理。
- **Core 自己提供 Web UI**：`crates/core/src/lib.rs:164` 的 `ServeDir`。

### 3.2 阻擋區網的地方

| # | 位置 | 現況 |
|---|---|---|
| B1 | `crates/core/src/local_core.rs:59` | 正式 Core 綁定 `127.0.0.1:0`，**每次啟動連接埠都不同**。 |
| B2 | `crates/core/src/lib.rs:171` | `serve()` 綁定 `Ipv4Addr::LOCALHOST`。 |
| B3 | `crates/core/src/access.rs:60` | Host 不是 `localhost` / `127.0.0.1` / `[::1]` 一律 403。 |
| B4 | `crates/core/src/access.rs:51`、`lib.rs:168` | Origin 與 CORS 白名單只列本機來源。 |
| B5 | `apps/desktop/src-tauri/src/paired.rs:35` | 配對 URL 只接受 `http://127.0.0.1` / `localhost`。 |
| B6 | `apps/desktop/src-tauri/tauri.conf.json:24` | WebView CSP 的 `connect-src` 只允許 `http://127.0.0.1:*`。 |
| B7 | 全部 | 沒有 TLS；bearer token、cookie、**web 登入用的 owner 密碼**都以明文傳送。 |

### 3.3 開放區網後會出問題的地方（不是阻擋，但必須處理）

| # | 位置 | 問題 |
|---|---|---|
| R1 | `access.rs:31` `budget()` | 登入與配對的嘗試次數限制是**全域**的（每種一列，每分鐘 10 次）。區網上任何一台機器都能讓所有人被鎖住一分鐘。 |
| R2 | `access.rs:82` `web_login` | 瀏覽器用 **owner 密碼**登入。owner 密碼是最高權限憑證，不應在區網上被輸入或傳送；即使有 TLS，自簽憑證的警告也容易被使用者略過。 |
| R3 | `access.rs:82` | cookie 沒有 `Secure` 屬性。 |
| R4 | `src/api.ts:105` `folder()` | 新增來源資料夾時，跳出的是**客戶端本機**的資料夾選擇器，但 Core 會在**自己的機器**上找這個路徑。遠端連線時會選到錯的路徑。 |
| R5 | `src/api.ts` | WebView 直接用 `fetch` 呼叫 Core。WebView2 無法對自簽憑證做指紋綁定，只會拒絕連線。 |
| R6 | `local_core.rs` 控制通道 | owner 只能透過 Windows named pipe 取得。管理配對與裝置都需要 Core 那台機器上有桌面程式。 |
| R7 | 前端輪詢 | 沒有推送；其他客戶端的修改要等下次重新抓取才看得到（revision CAS 保證不會覆蓋，但畫面可能是舊的）。 |
| R8 | `crates/core/src/main.rs` | 非 Windows 平台直接結束（`This executable currently supports Windows`）。只有 `examples/ui_core.rs` 能在 macOS 跑。 |
| R9 | Core 生命週期 | Core 由桌面程式啟動，常駐在背景，但開機後要等有人開桌面程式才會啟動。Core 主機上按「停止 Core」會讓所有遠端客戶端斷線。 |

## 4. 設計

### 4.1 兩個監聽器，loopback 保持原樣

- **loopback 監聽器**：完全不改，仍是 `http://127.0.0.1:<隨機埠>`，供本機 owner 與桌面使用。
- **區網監聽器（新增，預設關閉）**：只提供 HTTPS，固定連接埠（預設 `14830`，可設定），綁定使用者選定的網路介面位址。
- 兩個監聽器共用同一個 router。每個請求附帶 `Transport::{Loopback, Lan}` extension，讓 `guard` 判斷請求來自哪個監聽器。
- **區網監聽器的額外限制**：
  - 拒絕 owner bearer。owner 只能從 loopback 進來，作為縱深防禦。
  - Host 只接受 Core 自己的區網位址和設定的主機名稱（防 DNS rebinding；TLS 本身也會擋）。
  - 啟動時檢查綁定位址：只允許私有網段（RFC 1918）、`100.64.0.0/10`（Tailscale CGNAT）、IPv6 ULA 與 link-local。公網位址直接拒絕綁定，也不提供繞過的設定。
- 區網設定存在**裝置目錄**（`--device-dir`）的 `network.json`，**不寫進收藏資料庫**。這樣備份與還原收藏時不會帶走或覆蓋網路設定，也不需要 schema 遷移。
- 只有 owner 能開關區網模式，改設定後需要重啟 Core。

### 4.2 TLS 與憑證指紋

- 第一次開啟區網模式時，用 `rcgen` 產生自簽憑證（ECDSA P-256），私鑰存在裝置目錄，權限只限目前使用者。
- 伺服器端用 `tokio-rustls` 或 `axum-server` 的 rustls 支援；專案已經透過 reqwest 使用 rustls。
- 指紋採 **SPKI 的 SHA-256**，不用整張憑證的雜湊，這樣之後換發憑證時可以保留同一把金鑰。
- 憑證不設定 SAN 綁定 IP 的檢查（客戶端靠指紋驗證，不靠主機名稱），所以 Core 的 IP 改變不需要重新配對。

### 4.3 配對字串

owner 在 Core 主機上按「新增裝置」，畫面顯示一組可複製的配對字串（也可以顯示成 QR code）：

```text
galroon-pair:v1:<host>:<port>:<12 位配對碼>:<SPKI 指紋 base32，52 字元>
```

客戶端流程：

1. 用 TLS 連線，比對伺服器提供的 SPKI 指紋與配對字串是否一致。不一致就中止，不送出配對碼。
2. 送出配對碼，取得 bearer。
3. 把指紋和 `library_id`、`device_id` 一起存進 `connections.json`。之後每次連線都檢查指紋。

指紋不符時，顯示明確錯誤並要求重新配對；**不提供「仍然信任」的按鈕**。

### 4.4 桌面客戶端：本機轉發代理（解決 R5，盡量不動 B6）

WebView 無法綁定自簽憑證，所以由桌面程式的 Rust 端在 `127.0.0.1:<隨機埠>` 開一個轉發代理：

- 代理用 reqwest 連到遠端 Core，綁定指紋，並由代理加上真正的 bearer。
- WebView 只拿到代理位址和**每次啟動新產生的代理密鑰**，拿不到真正的遠端憑證。這比現在把 bearer 交給 WebView 更安全。
- 代理拒絕沒有代理密鑰的請求，避免本機其他程式借用代理。
- `api.ts` 和 CSP 幾乎不用改，WebView 仍然只連 `http://127.0.0.1:*`。
- 代理需要正確轉送 `x-galroon-library` / `x-galroon-device`、`Content-Type` 和 status code，並移除或改寫 `Host`。
- 請求大小和逾時要與 Core 端的限制一致，不能讓代理變成繞過限制的管道。

**替代方案**：每個 API 請求都改走 Tauri IPC。要改 `api.ts` 的所有呼叫路徑，而且大型回應會被序列化兩次，所以不採用。

### 4.5 瀏覽器客戶端（解決 R2、R3）

- **區網上停用 owner 密碼登入**：區網監聽器收到 `/api/access/login` 一律 403。
- 改成「瀏覽器配對碼」：owner 在 Core 主機產生一組短期配對碼，瀏覽器輸入後取得 `web` cookie。可以重用現有的 `pairing_codes` 表，加一個 `role` 欄位，或另開一張表。
- 區網監聽器發出的 cookie 加上 `Secure`。
- 瀏覽器會看到自簽憑證警告。Core 主機的設定頁顯示指紋，讓使用者手動比對。文件同時建議 Tailscale 使用者改用 `tailscale serve` 提供的正式憑證。

### 4.6 嘗試次數限制改為「每個來源」（解決 R1）

- `access_attempts` 的 key 從 `kind` 改成 `kind + 來源 IP`。
- 另外保留一個較寬鬆的全域上限，避免大量不同來源灌爆。
- loopback 來源維持現有行為。
- 這需要拿到連線的遠端位址（axum 的 `ConnectInfo`）。

### 4.7 新增來源資料夾（解決 R4）

- 桌面端判斷目前連的是遠端 Core（配對連線）時，**隱藏本機資料夾選擇器**，改成輸入「Core 主機上的路徑」，由 Core 驗證路徑是否存在、是否可讀。
- 第 2 階段再加 Core 端的目錄瀏覽 API：只允許 owner 與 `desktop`，只列資料夾、不列檔案，並沿用現有的路徑安全檢查。

### 4.8 多客戶端的畫面更新（R7）

- **第 1 階段**：不做推送。依靠現有的 revision 衝突檢查，確保衝突時顯示「這筆資料已被其他裝置修改，請重新載入」，而不是一般錯誤訊息。
- **第 2 階段**：新增輕量變更通知，讓客戶端知道收藏 revision 變了就重新抓取。可用 long-poll 或 SSE，二選一並說明理由。

### 4.9 Core 主機的生命週期（R9）

- 開啟區網模式後，在 Core 主機上提供「登入 Windows 時自動啟動 Core」選項（使用者層級的排程工作，不需要系統管理員權限）。
- 在 Core 主機上按「停止 Core」時，如果還有遠端裝置在使用（依 session `last_used` 判斷），先顯示有幾台裝置會被中斷，再要求確認。

### 4.10 mDNS 自動發現（第 2 階段）

- 只在區網模式開啟時廣播 `_galroon._tcp`。
- TXT 只放 `device_id` 與指紋前 16 字元，**不放收藏名稱**。
- 發現只是方便，信任仍然只來自配對字串裡的指紋。

### 4.11 路徑對應（延後）

目前產品沒有「在客戶端打開資料夾」或啟動遊戲的功能，所以**第 1、2 階段都不做**客戶端路徑對應。等之後真的需要在客戶端碰實體檔案時再設計；屆時存在客戶端的 `connections.json`，不存進 Core。

### 4.12 無畫面 Core（第 3 階段，R6、R8）

- 讓 `galroon-core` 可以在 Linux 上跑（拿掉 `main.rs` 的平台限制，控制通道在 Unix 改用權限 0600 的 Unix domain socket）。
- 提供 owner 命令列：`galroon-core owner pair --name <裝置名稱>`，透過本機控制通道產生配對字串。這樣 NAS 上不需要桌面程式。
- 7-Zip 改用系統安裝的 `7zz` 或 `7z`，找不到時把封存功能標示為不可用。
- 提供 Docker image。資料目錄必須掛載本機磁碟，啟動時偵測到資料目錄在網路檔案系統上就拒絕啟動。

## 5. 威脅模型摘要

| 攻擊者 | 第 1 階段之後的防護 |
|---|---|
| 區網上被動竊聽 | TLS；配對後所有流量加密。 |
| 區網上中間人 | 客戶端綁定 SPKI 指紋；指紋不符就中止，沒有「仍然信任」。 |
| 區網上暴力嘗試配對碼 | 48 位元配對碼、5 分鐘過期、只能用一次，加上每個來源的嘗試次數限制。 |
| 區網上阻斷登入（R1） | 每個來源分別計數。 |
| 竊取 owner 密碼 | owner 密碼不再在區網上使用（4.5）。 |
| 本機其他程式借用桌面代理 | 代理要求每次啟動新產生的密鑰。 |
| 惡意網頁做 DNS rebinding | 區網監聽器限制 Host，加上 TLS 憑證不符。 |
| 被偷走的 `desktop` 憑證 | owner 可撤銷（既有功能）；裝置列表顯示 `last_used`。 |
| 誤綁公網位址 | 啟動時拒絕非私有網段。 |

## 6. 相容性

- API 版本維持 `1`。在 `/api/context` 加上 `capabilities.lan_access`，舊的 Core 不會出現這個欄位。
- 收藏 schema **不變**，網路設定放在裝置目錄。唯一可能的 schema 變更是 4.5 的配對碼 `role` 欄位；若採用，就照現有的遷移流程進行，並附上遷移測試。
- `connections.json` 新增 `fingerprint` 欄位。舊的連線項目沒有這個欄位，只能連 loopback。
- 沒開區網模式時，行為與現在**逐位元組相同**；要有測試證明這一點。

## 7. 分階段與 PR 拆分

**第 1 階段：區網可用（家用區網）**

1. Core：`network.json`、私有網段檢查、固定連接埠、`Transport` extension。區網監聽器先只回 `/api/health`。
2. Core：TLS 與憑證產生、SPKI 指紋、owner 用的「網路設定」API。
3. Core：`guard` 依 `Transport` 分流（拒絕 owner bearer、Host 規則、停用密碼登入）、每個來源的嘗試次數限制、cookie `Secure`。
4. Core＋前端：配對字串產生與顯示、瀏覽器配對碼。
5. 桌面：配對字串解析、指紋綁定、轉發代理、`connections.json` 加入 `fingerprint`。
6. 前端：遠端連線時改成輸入 Core 路徑（4.7）、衝突訊息改寫（4.8）。
7. 桌面：登入時自動啟動 Core、停止 Core 前的確認。
8. 文件：`SUPPORT.md`、`USER_GUIDE.md`、`API_CONTRACT.md`，以及 Tailscale 說明。

**第 2 階段**：mDNS、變更通知、Core 端目錄瀏覽。

**第 3 階段**：Linux 無畫面 Core、owner 命令列、Docker。

## 8. 測試與驗收

**自動測試（每個 PR 附上）**

- 區網模式關閉時，所有現有測試不變；沒有新的監聽埠。
- 綁定公網位址被拒絕；`100.64.x.x` 與 `192.168.x.x` 被接受。
- 區網監聽器：owner bearer 被拒、`/api/access/login` 被拒、錯誤 Host 被拒、`web` 角色寫入仍被拒。
- 指紋不符時，客戶端在送出配對碼**之前**就中止（用兩組憑證的測試 Core 驗證）。
- 嘗試次數：來源 A 被鎖時，來源 B 仍可配對。
- 代理：沒有密鑰被拒；大型回應與錯誤狀態碼原樣轉送；身分標頭正確轉送。
- 兩個 `desktop` 客戶端同時編輯同一筆資料，後送出的一方收到衝突，資料不被覆蓋。

**手動驗收（兩台實體機器）**

- Mac（或第二台 Windows）當 Core，Windows 測試筆電當客戶端：配對、瀏覽、編輯標籤和清單，兩邊都看得到。
- 中斷網路後重連；Core IP 改變後重新輸入位址（指紋不變，不需重新配對）。
- 撤銷裝置後，該裝置立即無法存取。
- 透過 Tailscale 從外部網路連線。
- 遵守「零錯誤匹配」與現有的使用者故事驗收規則；區網功能不能改變匹配結果。

## 9. 審查重點（請 Codex 逐項回答）

1. **現況核對**：3.1 到 3.3 的每一條描述，是否與程式碼一致？有沒有漏掉的 loopback 假設（例如其他寫死 `127.0.0.1` 的地方、測試輔助程式、前端的 URL 組合）？
2. **兩個監聽器還是一個**：共用 router 並用 `Transport` extension 分流，是否有請求會繞過 `guard`（例如 `fallback_service` 的靜態檔案、CORS preflight）？
3. **owner 不從區網進入**：這個限制會不會讓某個現有流程壞掉？
4. **轉發代理（4.4）**：與其他做法相比是否最簡單？代理有沒有可能被用來繞過 Core 的大小限制、逾時，或 `collection_changed` 的檢查？
5. **配對字串（4.3）**：52 字元的指紋只能複製貼上或用 QR code。是否應改用短字串比對（雙方畫面各顯示 6 位數字）？
6. **瀏覽器配對碼（4.5）**：取代區網上的 owner 密碼登入是否合理？能否不改 schema 就做到？
7. **每個來源的嘗試次數限制（4.6）**：IPv6 一台機器可能有很多位址，是否需要以 /64 為單位？
8. **網路設定不放進收藏資料庫**：與現有的裝置目錄、還原流程（`backup::restore_new`）是否衝突？
9. **新增來源資料夾（4.7）**：遠端 `desktop` 能讓 Core 驗證任意路徑是否存在，算不算資訊洩漏？要不要限制？
10. **遺漏**：有沒有本規劃沒提到、但開放區網後會出問題的功能？例如封面快取、診斷記錄、`/api/timeline`、背景任務、還原收藏時遠端客戶端的行為。
11. **範圍**：第 1 階段的 8 個 PR 是否有可以刪掉或延後的？有沒有順序依賴錯誤？

請以「同意／不同意＋理由＋建議修改」的格式回覆，並附上檔案與行號。
