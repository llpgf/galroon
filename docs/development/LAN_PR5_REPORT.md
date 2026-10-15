# PR 5 實作報告

> 下文保留原始 IPC 實作與歷次驗收。2026-10-15 的 release 效能驗收未通過大型圖片門檻，因此目前改用計畫 §4.5 的本機轉發代理；最新結果見文末。

實作範圍：桌面透過配對字串連接 LAN Core，SPKI pinning、IPC、持久化重新配對狀態。未新增 crate、未調整 Cargo features／lockfile、未修改 CSP／HTTP API／schema、未執行 cargo fmt、未建立 commit。

## 變更檔案

| 檔案 | 內容 |
|---|---|
| `crates/core/src/lan_client.rs` | 自訂 rustls verifier、配對、端點與指紋驗證、每連線 HTTP pool、限流／取消、proxy、無 bearer 的遠端 session、二進位 IPC envelope。 |
| `crates/core/src/lan_client_tests.rs` | TLS 1.2／1.3 真實握手、壞簽章／換金鑰、LAN listener 配對、proxy 邊界／錯誤／大小、取消及 envelope 測試。 |
| `crates/core/src/lib.rs` | 匯出 `lan_client`。 |
| `apps/desktop/src-tauri/src/paired.rs` | 相容舊 JSON 的 `fingerprint`／`needs_repair`，LAN 配對、Credential Manager 儲存及失敗回滾，HTTPS 端點與連線驗證、重新配對標記。 |
| `apps/desktop/src-tauri/src/controller.rs` | 快取選取連線，切換後取消舊連線；持久化 401 `credential_invalid`；遠端 session 不包含 bearer。 |
| `apps/desktop/src-tauri/src/main.rs` | 註冊 `pair_lan_connection`、`core_request`，更新 `core_session` 回傳。 |
| `src/api.ts` | remote session 走 IPC，二進位 envelope 還原為 Response，沿用既有錯誤／身分處理，加入 `repair_required`。 |
| `src/ConnectionsPanel.tsx` | 增加配對字串表單，保留 loopback 配對。 |
| `src/ConnectionBanner.tsx`、`src/main.tsx` | 重新配對提示與停止重連，仍可開啟連線管理。 |
| `src/i18n.ts`、`src/locales/zh-TW.ts` | 英文／繁體中文提示，更新過時的「LAN 尚未啟用」說明。 |
| `src/ConnectionsPanel.test.tsx`、`src/lanTransport.test.ts` | 配對表單、IPC JSON／原始圖片、錯誤透傳、失效配對停止重試、取消及過期結果。 |
| 本報告 | 交接、驗證結果與 Windows 待驗項目。 |

## 新增公開介面

以下 `R<T>` 皆為 `Result<T, String>`。

```rust
pub fn pinned_client(fingerprint: [u8; 32]) -> R<reqwest::Client>;
pub fn decode_fingerprint(value: &str) -> R<[u8; 32]>;
pub fn endpoint(raw: &str) -> R<String>;
pub async fn pair_with_string(pairing: &str, device_name: &str) -> R<PairedLan>;
pub async fn proxy(conn: &LanConnection, request: ProxyRequest) -> R<ProxyResponse>;

impl LanConnection {
    pub fn new(url: String, fingerprint: String, token: String,
        library_id: String, device_id: String) -> R<Self>;
    pub async fn session(&self) -> R<RemoteSession>;
    pub fn cancel(&self);
    pub fn is_cancelled(&self) -> bool;
}
impl ProxyResponse {
    pub fn credential_invalid(&self) -> bool;
    pub fn into_ipc_bytes(self) -> R<Vec<u8>>;
}
```

- `PairedLan`：`url, fingerprint, token, library_id, device_id, name`，皆為 String；僅供原生端保存，沒有直接作為 Tauri command 回傳。
- `LanConnection`：由上述五個參數建立；私有且不可變的上游／credential／identity，加上建立一次的 reqwest client、8 個 semaphore permits、取消通知。指紋由 client verifier 保存。這樣無法修改欄位後誤用舊的 TLS pool。
- `ProxyRequest`：`method: String, path: String, body: Option<Vec<u8>>, content_type: Option<String>, library_id: String, device_id: String`；method 在 core 嚴格限制為 GET／POST／HEAD。
- `ProxyResponse`：`status: u16, headers: BTreeMap<String,String>, body: Vec<u8>`。
- `RemoteSession`：固定 `url: "ipc://core", token: "", remote: true`，以及 `library_id, device_id, api_min, api_max`。
- 桌面：`paired::pair_lan(base: &Path, pairing_string: &str) -> R<Connections>`；Tauri `pair_lan_connection(pairing: String)`、`core_request(method, path, body, content_type, library_id, device_id) -> Result<tauri::ipc::Response,String>`。JS 參數採 Tauri 預設 camelCase（例如 `libraryId`）。

## 驗證與傳輸設計

`verify_server_cert` 先以 `rustls::server::ParsedCertificate::try_from` 驗證 DER，再呼叫既有 `lan_tls::spki_fingerprint`，避免無效 DER 觸發 panic。只接受完整 32 bytes SPKI SHA-256 相符；不檢查信任鏈與 hostname。

TLS 握手簽章沒有略過：`verify_tls12_signature`、`verify_tls13_signature` 分別呼叫 `rustls::crypto` 同名函式，傳入 ring provider 的 `signature_verification_algorithms`；`supported_verify_schemes` 也由同一 provider 取得。測試故意破壞 server signer 的輸出，在 TLS 1.2 和 TLS 1.3 都被拒絕；不同 SAN 但正確 SPKI 的握手可成功。

reqwest 原有 `rustls-tls` feature 已足夠。client 使用 ring、TLS 1.2／1.3、`https_only(true)`、`no_proxy()`、redirect none、`retry::never()`、10 秒連線／60 秒請求逾時。配對先完成無秘密的 `/api/health`，之後才送 code；每個後續 TLS 連線仍受同一 verifier 約束。

proxy 每連線最多 8 個上游請求，等待 permit 也納入 60 秒 deadline。路徑先於 URL 正規化檢查，也拒絕編碼 traversal；request identity 必須等於保存值，上游 headers 永遠取保存值。JSON／text 上限 32 MiB、artwork 上限 16 MiB，每次 chunk 在追加前檢查。401／403／409／503 不呼叫 `error_for_status`，保留原始 status/body。

切換或更新選取連線時取消舊 core connection，正在等候／讀取的 proxy future 隨之丟棄；回傳前再檢查取消狀態。前端的 session epoch 也會丟棄舊結果。個別 AbortSignal 立即停止前端等待並丟棄結果；該原生請求在連線取消或 60 秒 deadline 時結束。POST 不重送。

`needs_repair` 寫入 connections.json，保持 selected；重啟後、session/proxy 及連線驗證都拒絕使用該項目。使用者可從連線管理取得新配對、明確選取新項目。

## IPC 圖片回傳

單一 `tauri::ipc::Response::new(bytes)`，格式為：

```text
4 bytes：big-endian metadata 長度
N bytes：UTF-8 JSON {status, headers}
其餘：原始 response body
```

headers 只包含 content-type、x-galroon-library、x-galroon-device、retry-after。圖片不進 JSON、不做 base64；metadata 與 body 同一回應，無須另一個 command 維護結果暫存或做競態配對。前端用 byte view 建立 ReadableStream／Response，保留原本 JSON 與 artwork 錯誤處理。

## 測試結果（macOS sandbox）

- 指定 `cargo test -p galroon-core -p galroon-mcp --lib --bins --tests` 已執行；core lib 遇到 socket 限制後 Cargo 停止。補用相同命令加 `--no-fail-fast` 跑完全部目標。
- 最終 core lib：**402 通過、25 失敗、1 忽略**。25 項全部是在 socket bind 遭 sandbox 拒絕（`Operation not permitted`），包含既有 TCP／Unix socket 測試與 3 項新增 LAN listener 測試。
- MCP lib：**2 通過**；MCP e2e：**3 項全部被 socket bind 阻擋**。其餘 bin／Windows 條件測試在此平台為 0 項。
- 新增 core 測試共 8 項：**5 通過、3 被 bind 阻擋**。通過項目涵蓋真實 memory-stream TLS 握手（兩個 TLS 版本、壞簽章、錯誤指紋、重新產生金鑰）、無效 DER、端點／路徑／method／identity 限制、base32、binary envelope、body 邊界、status/body/header、取消與並行上限。
- 新增實際 socket 測試已編譯：正確配對與 context／無 bearer session；錯誤指紋下 code 未兌換且 attempts 不增加；POST 次數與 401／403／409／503 body；HEAD／binary；chunked 大小限制。**尚未執行到 HTTP assertions，不能宣稱整合測試通過。**
- `cargo clippy -p galroon-core -p galroon-mcp --lib --bins --tests`：成功；保留既有 warnings，新增 LAN module 沒有 warning。
- `npx vitest run --configLoader runner --no-cache`：**26 檔、147 項全部通過**（既有 node_modules 實際執行 Vitest 3.2.7）。
- `npx tsc --noEmit`、`git diff --check`：成功。
- 未編譯 galroon-desktop；未量測 Windows IPC 相對 fetch 的 p95／尖峰記憶體。沒有可用的 Windows WebView 執行環境，這些是剩餘驗收條件。

完整本機輸出在 `target/lan-pr5/`（不納入版本控制）：`tests-final.log`、`clippy-final.log`、`vitest-final.log`、`tsc-final.log`。

## Windows 編譯風險與交接

新增／變動桌面程式所使用的 Rust APIs：

- Tauri：`#[tauri::command]`、`tauri::State<'_, Controller>`、`tauri::generate_handler!`、`tauri::ipc::Response::new(Vec<u8>)`、`Manager::manage`；`core_session` 現回傳 `serde_json::Value`。
- core：`lan_client::{pair_with_string, pinned_client, decode_fingerprint, endpoint, proxy, LanConnection, ProxyRequest}`；`LanConnection::{new,session,cancel,is_cancelled}`；`ProxyResponse::{credential_invalid,into_ipc_bytes}`。這些 core APIs 已在 Mac 編譯。
- reqwest（paired wrapper）：`Client::{get,post}`、`RequestBuilder::{header,bearer_auth,json,send}`、`Response::{status,content_length,chunk}`、`StatusCode::{as_u16,is_success}`。遠端使用 core 建立的 pinned client。
- serde／serde_json：Serialize／Deserialize derive、`#[serde(default)]`、`Value`、`json!`、`to_value`、`from_slice`、`from_value`（測試）。
- std／Tokio：`Arc::{new,clone}`、`tokio::sync::Mutex::{new,lock}`、`Drop`、`Option::{as_ref,is_some_and,take}`、`Result::{as_ref,is_err_and}`、`PartialEq/Eq` derive；既有 fs／Path／fs2 launch lock 與 `local_core::Session` 繼續沿用。
- Windows Credential Manager：沿用原有 `credential_write/read/delete`，其內部 `CredWriteW`、`CredReadW`、`CredDeleteW`、`CredFree`、`CREDENTIALW` 等未改寫，未引入新 Windows API。

請 Claude 優先確認：

1. Tauri command 巨集對 `Result<ipc::Response,String>` 的 async/Send 限制，以及 `State<Controller>`、Arc／Mutex／fs::File guard 的組合；此 wrapper 無法在 Mac 編譯。
2. WebView2 的 raw IPC 回傳確實是 ArrayBuffer；核對 JS camelCase 與 Rust snake_case 參數對應，以及 command 註冊。
3. 新 Controller 欄位與 constructor、既有 Windows acceptance fixtures，`Connection` 新欄位的所有建構處；舊 JSON 與 `needs_repair` round-trip 測試已補在 paired.rs。
4. Credential Manager 寫入成功但 connections.json 儲存失敗時，credential 確實刪除；撤銷後首次 401 body 原樣回傳，重啟後仍保留 selected 並阻擋自動連線。
5. 在允許 socket 的環境重跑完整 core／MCP 測試，特別是上列 3 個新 TLS listener 整合測試。
6. 依計畫 §4.5 在 Windows 對 IPC／fetch 各量 50 次：60 筆分頁 p95 額外 ≤15 ms、5 MB 封面 p95 額外 ≤30 ms／尖峰記憶體增加 ≤3 倍、8 請求並行、切換連線取消、錯誤 body 一致。若不合格，才依計畫評估本機代理備案；本 PR 沒有臆測效能通過。

## 合併前驗證（2026-10-14，macOS，無 sandbox）

- Rebase 到 master（#53–#58）：`src/api.ts` 保留 master 的錯誤 `code`，加上 `credential_invalid` → 重新配對；`src/main.tsx` 保留更新提示列，加上 `repair_required`。
- 上面「尚未執行」的 socket 測試這次實際跑了。`pairing_checks_pin_before_sending_code_and_context_is_pinned` 原本用格式錯誤的 token，Core 依設計回 `Authentication required` 而不是 `credential_invalid`；測試改成撤銷剛配對的 session。
- `cargo test -p galroon-core -p galroon-mcp --lib --bins --tests`：core 453 通過、1 忽略；MCP 全部通過。`npx vitest run`：32 檔 184 項通過。`npx tsc --noEmit` 成功。
- galroon-desktop 在 macOS 仍無法編譯（build script 需要 Windows 資源），Windows 編譯與上面的 IPC 效能量測仍待驗。

## Windows 與 NAS 實機驗證（2026-10-15，香港時間）

驗證桌面版本為 PR #59 的 `4a84fb6`。Windows 原生 Tauri App 使用獨立本機 profile，連到 NAS 上獨立的 `galroon-pr59-check` 容器；資料來自現有收藏的 catalog-only 備份，保留 620 個作品、793 個資源及 6,512 筆檔案索引，排除原有登入憑證。兩個 NAS Core 均將 `/volume1/Backup/Galgame` 唯讀掛載，測試實例另用資料目錄及 LAN port 14831。

NAS 沿用 `galroon:f0e5b43bcb1f`：它已包含本次使用的 LAN TLS、owner CLI 與配對 API；#59 的新增內容在桌面及 LAN client。只在獨立實例設定測試 owner，現有 Core 的 owner 認證未更動。

- Windows 執行 `npm ci`、`npm run build`、`cargo build --locked -p galroon-core --bin galroon-core` 及 `cargo build --locked -p galroon-desktop` 成功。桌面以 `TAURI_CONFIG={"build":{"devUrl":null},"bundle":{"resources":[]}}` 嵌入建置好的前端並略過安裝包資源複製；同目錄提供原生 Core。**驗證的是 debug App，尚未驗證 release installer。**
- 實際桌面 UI 貼上 NAS 配對字串、儲存配對、確認開啟收藏成功。首頁與收藏頁顯示 620 個作品，作品詳情、封面、角色資料及「NAS Galgame」資源正常載入。
- NAS owner 撤銷該桌面憑證後，UI 顯示「需要重新配對／此裝置的配對已失效」，`connections.json` 保留 selected 並寫入 `needs_repair:true`。取得新配對字串並明確開啟新連線後，恢復同一收藏的正常瀏覽。
- 補充原生 probe 直接使用此版本的 `Controller`、`paired` 與 `lan_client`。錯誤 SPKI 指紋被拒絕；desktop role、空的 WebView session token、兩頁各 60 個作品、作品詳情及 assets 通過，203,197 bytes 的封面以二進位 IPC envelope 回傳。
- probe 憑證撤銷後，代理保留 `401` 及 `credential_invalid`；失效狀態與 selected 持久化，重新建立 Controller 仍拒絕舊憑證，未啟動本機收藏作為 fallback。Credential Manager 測試在登入中的 Windows 桌面工作階段執行；SSH 工作階段不能寫入該 credential store。
- 測試快照的來源監控保持停用；未操作掃描、檔案取得／移動或收藏編輯。結束後逐列核對 `works`、`resources`、`resource_files`、`files` 與 `jobs`，均與備份完全相同，10 個既有 completed jobs 沒有增減。Core 的自動配對設定在執行中仍為 enabled，因此沒有將「停用自動配對」列為驗證結果。
- 遊戲庫 7,727 個目錄項目（6,618 個檔案、1,109 個目錄）的路徑、類型、大小、mtime 指紋前後一致：`4e94f4eb6f947b7916b22d09cbc392129b88c47d31f6f0127695ff896eac8295`。這是檔案 metadata 指紋，加上 Docker 唯讀掛載；沒有對約 3.3 TB 的檔案內容逐一計算 hash。
- 測試裝置全部撤銷，測試容器停止並移除，Windows 測試 App／Core 關閉，臨時排程及 client credentials 清除。現有 NAS Core 保持正常執行，LAN 設定恢復為未設定，owner 仍未設定密碼。

Windows MCP 測試的 watched-scan／automatic-matching 競態已修正，三項 e2e 連跑十次全部通過（30/30）；另將 Windows Core limiter fixture 的 1,000 筆資料改為同一 transaction 並統一 timestamp，避免慢速逐筆 commit 讓資料提前過期。`4a84fb6` 的 CI 四項工作均通過。

原始截圖、檔案指紋、唯讀掛載證據及 probe JSON 保存在執行此驗證的 worktree `test-output/lan-pr5-nas-live/`，不含於 Git。計畫 §4.5 的 IPC／fetch p95 與 5 MB 封面尖峰記憶體比較仍未量測；上述結果不能代替效能驗收。

## 最終 release 效能與安裝包驗收（2026-10-15）

驗證程式碼為 `5a30bf6`，包含最新 master。正常執行 `npm run desktop:build`，未省略 bundle resources；測試的是 NSIS 安裝後的 release App、內嵌前端與完整 Core/MCP。Windows 測試採獨立安裝路徑、profile、WebView profile 和資料庫快照。

### 效能結果與實際採用的傳輸

原始 binary IPC 的 JSON 分頁通過，但 5 MiB 圖片 p95 額外約 46.9 ms、記憶體增加約 80 MB，超過計畫門檻，因此啟用 §4.5 原先規劃的本機代理備案。完整緩衝的代理仍超過記憶體上限（18,247,680 bytes）；原生串流配合 JS TransformStream 也為 18,427,904 bytes。最終原生端分段轉送，有 Content-Length 的圖片直接由瀏覽器讀成 Blob；未知長度仍逐塊限制，兩條路徑都檢查最終大小、取消與收藏切換。

使用安裝後的真實 Tauri/WebView2，由測試頁載入同一版 `src/api.ts` 的 bundle，停止背景 UI 輪詢以量測傳輸。對照為同一台 release Core、相同 desktop role 的直接 HTTP fetch；每項先暖機 5 次，再各取 50 次並交替順序。資料為 620 作品／793 資源的獨立快照，選定資源含 2,021 個索引成員；測試圖片為有效的 5,242,880-byte PNG。

| 項目 | 直接 fetch p95 | 原生代理 p95 | 額外 p95 | 門檻 | 結果 |
| --- | ---: | ---: | ---: | ---: | --- |
| 60 筆作品 | 135.6 ms | 135.8 ms | 0.2 ms | ≤15 ms | 通過 |
| 60 筆檔案成員 | 41.8 ms | 43.5 ms | 1.7 ms | ≤15 ms | 通過 |
| 5 MiB 圖片 | 103.5 ms | 110.7 ms | 7.2 ms | ≤30 ms | 通過 |

- 首次大圖請求的 Private Bytes 每 5 ms 取樣；桌面與此 WebView 的全部程序合計尖峰增加 **6,443,008 bytes（6.14 MiB）**，低於三倍圖片大小的 15,728,640-byte 上限。原生程序增加 446,464 bytes，WebView 增加 5,996,544 bytes；合計使用同時間樣本，未把不同時間的尖峰相加。圖片雜湊在記憶體取樣結束後才計算，與 fixture 完全一致。
- 8 個並行請求全部回傳正確的 60 筆；取消回 AbortError，切換收藏拒絕舊回應。原生測試另涵蓋 8 個 slot、串流大小上限、取消、丟棄串流後釋放 slot、400/401/403/409/503 原始 body/header 與寫入不重送。
- WebView 遠端 session 的 token 為空。每次啟動的本機代理 key 不同，舊 key／端點失效；NAS bearer 留在 Credential Manager／原生端。代理的 Host、Origin、key、path/method 與固定上游限制皆有測試。

### 安裝、實機生命週期與清理

- 補齊 pinned `yasna` MIT／Apache license，正常產生 notices／inventory，release NSIS 成功建置。驗收包 SHA-256：`EA88F1F4EF307E1B0128E82977967ACD1F1504EF76CE61708018BD21E4739E65`。
- 新安裝及升級成功；安裝後 desktop/Core/MCP 與實際 NSIS payload 的 SHA-256 一致。修正 NSIS `FindFirst` 前未清除上一輪 error flag，確認舊 JS/CSS 清除、無關檔案保留。測試曾在 Stop-Process 尚未完全退出時立即升級，雜湊檢查抓到舊 executable；加入等待退出後再驗證成功。
- 本機測試 Core 撤銷回傳 `401 credential_invalid`，selected 與 needs_repair 保留，重配與重啟後仍開啟同一收藏。
- 最終安裝包再連 NAS 獨立實例，620 部收藏、60 筆分頁、作品詳情和 203,197-byte 封面皆成功；撤銷後真實 UI 顯示需要重新配對，重新配對恢復同一收藏。NAS 原始遊戲目錄維持唯讀掛載。
- 測試裝置已撤銷，Windows credentials、測試排程及安裝包已清理，原有安裝註冊與捷徑還原；原有安裝的 85 個檔案雜湊全部不變。NAS 清理時一次 owner CLI 回應逾時，另以唯讀查詢確認該測試 session 已 revoked；未把逾時當作未執行。
- 未部署正式 NAS Core，未操作原始遊戲檔案。Windows 效能快照的來源改成不存在的測試路徑、監控及自動配對停用。

最終回歸：Core **460 通過、1 忽略**，MCP 6 項通過；前端 **32 檔／188 項通過**；TypeScript／production build 通過；Windows 登入中的工作階段執行 desktop 原生 **9 項全通過**。Windows／Linux CI、frontend 與 repository checks 通過。原始量測、失敗輪次、安裝紀錄、測試輸出與截圖保留在本機 `GalroonTestData/pr59-release-2026-10-15`，不將私人收藏或憑證加入 Git。
