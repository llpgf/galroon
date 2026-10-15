# PR 4：web 資料邊界與收藏變更序號

> Codex 的實作交接報告，對應已合併的 #45。下面「未 commit」「待重跑」等描述是當時的狀態。

範圍：schema 45 → 46、所有 web session（loopback／LAN）的唯讀資料邊界、共用 jobs 排程器的 10 秒可見頁面變更輪詢、修訂衝突英／繁中訊息。未修改桌面傳輸，未新增 crate。

## 變更檔案與摘要

| 檔案 | 摘要 |
|---|---|
| `crates/core/src/web_projection.rs`、`web_projection/tests.rs` | 70 條 GET／HEAD 政策、集中投影與字串／鍵名路徑遮蔽、全路由角色／transport 測試。 |
| `crates/core/src/catalog_changes.rs`、`catalog_changes/tests.rs` | 獨立 seq、表／指定事件 triggers、`GET /changes`、migration／寫入／唯讀測試。 |
| `crates/core/src/access.rs`、`lib.rs` | 在認證 guard 套用 web 政策、註冊 changes 路由；啟動時準備 issue 結果。HTTP 測試 fixture 明確發布 issues。 |
| `crates/core/src/db.rs` | schema 46；依原有備份與交易流程遷移；更新 PR 3 遷移測試的最新版本。 |
| `crates/core/src/issues.rs`、`smart_updates.rs` | issue reconcile 移出 GET，接到既有背景工作；以 seq checkpoint 避免無變更時反覆重算。來源狀態仍由 observe_roots 發布，root_digest trigger 遞增 seq。 |
| `crates/core/src/smart_snapshots.rs`、`automatch.rs` | 智慧結果成功完成時顯式 bump；新增實際 auto-match 執行後 seq 增加測試。 |
| `crates/core/src/resource_page.rs` | 原有 web／desktop 相等契約測試改為明列移除 `items[].root_path` 後相等。 |
| `crates/core/src/acquire_issues.rs`、`editions.rs` | 舊版 schema fixture 在刪除舊欄位前移除新 UPDATE trigger；只調整測試。 |
| `src/catalogPolling.ts`、`catalogPolling.test.ts` | 沿用 jobs 的單一 setTimeout 排程；可見時 changes 每 10 秒，背景 jobs 15 秒；相同 seq 不刷新，失敗刷新可重試。 |
| `src/catalogRefresh.ts`、`catalogRefresh.test.tsx`、`SourcePath.tsx` | 依角色讀取當前畫面，web 跳過 plans／quarantine；來源無 path 時隱藏路徑；新增 UI／refresh 測試。 |
| `src/main.tsx`、`api.ts`、`TaskMessages.tsx` | 串接 changes 與既有 refresh／generation；投影欄位改 optional；web 不顯示計畫歷史。API 錯誤集中翻譯修訂衝突。 |
| `src/ListsPanel.tsx`、`SmartListsPanel.tsx`、`ListExplorer.tsx` | 清單訂閱 generation；保留編輯草稿，背景查詢期間維持已開啟探索與編輯元件。 |
| `src/EntityTags.tsx`、`RelatedTags.tsx`、`IssuesPanel.tsx`、`useWorkExploration.ts`、`useProfilePages.ts`、`useDiscoveryPages.ts` | 讓目前開啟的標籤／問題／作品／人物／探索結果隨 refresh generation 重新讀取。 |
| `src/coreText.ts`、`coreText.test.ts`、`i18n.ts`、`locales/zh-TW.ts` | 明列既有 CAS 文字，集中映射要求的英文與繁中訊息；保留其他錯誤。 |
| `docs/development/API_CONTRACT.md`、本報告 | 記錄契約例外、完整路由分類、seq 寫入對照與驗證／審查事項。 |

## 畫面資料 → 寫入 → seq 對照

此表依 router 與各寫入模組核對；序號是失效通知，非交易筆數。同一交易可以遞增多次。UPDATE 值未改變時不遞增。

| 畫面資料 | 寫入的表／事件 | seq 遞增方式 |
|---|---|---|
| 收藏、作品資料、狀態、收藏標記、備註、VNDB 標籤 | works（含 tags、source_json、overrides） | INSERT／UPDATE／DELETE trigger |
| 版本、平台、語言、預設版本 | releases（程式的 editions）、work_releases、work_preferences | 同上，單改 join 亦觸發 |
| 資源、檔案、來源成員、作品綁定 | resources、files、resource_files、resource_bindings | 同上 |
| 個人標籤與作品／人物／角色／公司關聯 | custom_tags、custom_tag_works、custom_tag_entities | 同上 |
| 手動清單、成員、排序、註記 | curated_lists、list_entries、list_entry_variants | 同上 |
| 智慧清單定義、釘選、刪除／還原 | smart_lists | 同上 |
| 關係決策、逐筆關係修正、人物等欄位覆寫 | relation_overrides、relation_corrections、entity_overrides | 同上；衍生索引不另計 |
| 來源新增／刪除／名稱／路徑／角色 | roots | 同上 |
| 來源監看設定與狀態 | source_watch 的 enabled、exclude、revision、status、message、needs_scan、job_id、pending | trigger；忽略 last_event／gap_after 等 bookkeeping |
| 來源上線／離線 | smart_updates::observe_roots → smart_catalog_state.root_digest | 只在 digest 真正改變時 trigger；不跟隨快取造成的 revision |
| 自動匹配狀態與結果 | match_items、works、releases、resources、resource_bindings、jobs；auto_match_enabled／auto_match_requested 設定 | 表 trigger＋指定 settings key trigger |
| 智慧清單結果完成 | smart_snapshots::build_using 完成 requests／entries 發布 | 成功末端顯式 bump；交易 rollback 一起回滾，request replay 不 bump |
| 智慧計算失敗／恢復 | smart_compute_state.error | 僅首次非空錯誤或 error 改變；retry 時間戳不 bump |
| 問題中心 | issues（掃描、匹配、來源與 acquire 問題） | INSERT／UPDATE／DELETE trigger；啟動與既有 smart worker 進行 reconcile，GET 不寫入 |
| 檔案計畫、位置、隔離狀態 | plans、plan_locations、operations、plan_reversals、quarantine | 同上 |
| 任務歷史與進度 | jobs、job_summary_entries | 同上 |
| 管理目的地設定 | settings['organization.managed_root'] | 僅指定 key，值相同不 bump |
| 收藏改名 | library_identity.name／revision（context.library.revision） | trigger |
| 認證、配對、讀取時間、嘗試次數 | sessions、pairing_codes、access_attempts | 不 bump |
| 探索、搜尋、圖片、分頁及智慧結果租期 | exploration／discovery settings、artwork cache、smart_snapshot_cache 等 | 不 bump；不追蹤 smart_catalog_state.revision |
| 診斷及追蹤事件 | diagnostics、events | 不 bump |

## 完整 web 路由分類

| GET／HEAD route | web 分類 | 說明 |
|---|---|---|
| `/api/health` | 原樣 | 無實體路徑欄位 |
| `/api/context` | 原樣 | 無實體路徑欄位 |
| `/api/access/status` | 原樣 | 無實體路徑欄位 |
| `/api/access/me` | 原樣 | 無實體路徑欄位 |
| `/api/access/pairing-codes` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/access/network` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/access/devices` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/changes` | 原樣 | 無實體路徑欄位 |
| `/api/dashboard` | 原樣 | 無實體路徑欄位 |
| `/api/roots` | 投影 | 只留 id、label、role、online、missing、unverified |
| `/api/roots/{id}/watch` | 投影 | 監看 message／exclude 的絕對路徑遮蔽 |
| `/api/jobs` | 投影 | 陣列與 ?paged=true 均移除 current_path；遮蔽 message |
| `/api/jobs/{id}` | 投影 | 移除 current_path；遮蔽 message |
| `/api/timeline` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/issues/{id}/task` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/resources/page` | 投影 | items 移除 root_path |
| `/api/resources` | 投影 | 移除 root_path；保留相對 path |
| `/api/works` | 原樣 | 無實體路徑欄位 |
| `/api/works/lookup` | 原樣 | 無實體路徑欄位 |
| `/api/works/{id}/assets` | 投影 | resources 移除 root_path |
| `/api/collection` | 原樣 | 無實體路徑欄位 |
| `/api/issues` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/diagnostics` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/vns/{id}/relationship-decisions` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/vns/{id}/relationship-history` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/vns/{id}/relationship-corrections` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/vns/{id}/relationship-corrections/history` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/relationship-candidates/{id}` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/entities/{id}/manual-works` | 原樣 | 無實體路徑欄位 |
| `/api/entities/{id}/field-decisions` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/entities/{id}/field-history` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/works/{id}` | 原樣 | 無實體路徑欄位 |
| `/api/works/{id}/edition-options` | 原樣 | 無實體路徑欄位 |
| `/api/releases/labels` | 原樣 | 無實體路徑欄位 |
| `/api/releases` | 原樣 | 無實體路徑欄位 |
| `/api/releases/{id}` | 原樣 | 無實體路徑欄位 |
| `/api/releases/{id}/membership` | 原樣 | 無實體路徑欄位 |
| `/api/resources/{id}/members` | 投影 | files[].path 改為 relative |
| `/api/resources/{id}/members/page` | 投影 | 已使用 relative；仍經共用投影與遮蔽 |
| `/api/resources/{id}/matching-history` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/matching` | 投影 | 背景工作文字遮蔽 |
| `/api/works/{id}/metadata` | 原樣 | 無實體路徑欄位 |
| `/api/works/{id}/exploration` | 原樣 | 無實體路徑欄位 |
| `/api/people/{id}` | 原樣 | 無實體路徑欄位 |
| `/api/characters/{id}` | 原樣 | 無實體路徑欄位 |
| `/api/companies/{id}` | 原樣 | 無實體路徑欄位 |
| `/api/discover` | 原樣 | 無實體路徑欄位 |
| `/api/discover/manual` | 原樣 | 無實體路徑欄位 |
| `/api/artwork` | 原樣 | 無實體路徑欄位 |
| `/api/artwork/cache` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/vns/{id}/exploration` | 原樣 | 無實體路徑欄位 |
| `/api/lists` | 原樣 | 無實體路徑欄位 |
| `/api/lists/{id}` | 原樣 | 無實體路徑欄位 |
| `/api/vns/{id}/related-tags` | 原樣 | 無實體路徑欄位 |
| `/api/works/{id}/related-tags` | 原樣 | 無實體路徑欄位 |
| `/api/entity-tags/{id}` | 原樣 | 無實體路徑欄位 |
| `/api/custom-tags` | 原樣 | 無實體路徑欄位 |
| `/api/custom-tags/{id}/membership` | 拒絕（403） | 既有 desktop 編輯權限 |
| `/api/smart-lists/options` | 原樣 | 無實體路徑欄位 |
| `/api/smart-lists/{id}/results` | 投影 | 遮蔽背景計算 update_error 中的路徑 |
| `/api/smart-lists` | 原樣 | 無實體路徑欄位 |
| `/api/smart-lists/{id}` | 原樣 | 無實體路徑欄位 |
| `/api/works/{id}/split-preview` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/backups` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/plans` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/plans/{id}` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/duplicate-checks/{id}` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/quarantine` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/resources/{id}/acquire-preview` | 拒絕（403） | 既有權限／編輯或檔案管理用途 |
| `/api/archive/capabilities` | 原樣 | 無實體路徑欄位 |

`/api/roots/{id}/remap/preview` 是 POST，沿用 web 禁止寫入的 403。GET 隱含的 HEAD 也套用相同權限。未知的已匹配 API 採拒絕預設。所有 JSON 回應（含原樣分類的異常資料、巢狀訊息及錯誤）最後經共用路徑遮蔽；圖片 bytes 不作文字解析。

## 驗證結果

- 指定 `cargo test -p galroon-core -p galroon-mcp --lib --bins --tests`：Core **375 passed / 22 failed / 1 ignored**。22 個失敗皆在 TCP／Unix socket 綁定時回報 `Operation not permitted`；未標為程式測試通過。
- 另以相同參數加 `--no-fail-fast` 完成其他 targets：MCP lib **2 passed**；MCP e2e **3 failed**，同樣因 socket binding 被 sandbox 擋下；兩個 bin 與非 Windows 的 windows_live target 各 0 tests。合計 **377 passed / 25 sandbox-blocked / 1 ignored**。
- 新增 PR 4 Core 測試全部通過：全路由分類涵蓋、70 routes × GET／HEAD × loopback／LAN、owner／desktop 原資料、API error 與 POSIX／drive／UNC 遮蔽；45→46 備份遷移、所有 trigger 表寫入／rollback、join-only insert／delete、source offline、smart completion／replay／cancel、實際 auto-match、cache／lease／認證與唯讀 API 不 bump。
- 最後定向重跑 `web_projection`：**3 passed**；`catalog_changes`：**5 passed**（包含 1 個既有 smart snapshot 測試）。
- `cargo clippy -p galroon-core -p galroon-mcp --lib --bins --tests`：**exit 0**。保留既有程式 warnings，沒有執行 fmt 或自動修正。
- 指定 `npx vitest run --configLoader runner --no-cache`：**24 files / 136 tests passed**。新增輪詢 4、web refresh／無 path 畫面 3、雙語衝突訊息 1 個測試。
- 額外 `npx tsc --noEmit`、`git diff --check`：通過。

### Sandbox 擋下的測試


- `access::lan_tests::disabled_opens_no_lan_listener_and_bind_conflicts_fail`
- `access::tests::http_browser_cookie_cannot_mutate_and_device_revocation_is_immediate`
- `access::lan_tests::real_tls_permissions_and_both_listeners_drain`
- `artwork::tests::downloader_bounds_declared_and_chunked_sizes_and_accepts_exact_payload`
- `artwork::tests::downloader_rejects_redirect_status_and_truncated_body`
- `core_host::tests::occupied_fixed_port_fails_without_fallback`
- `collection_catalog::tests::http_readonly_pages_and_owner_selection_enforce_scope_without_catalog_writes`
- `context::tests::context_is_private_readable_and_cross_library_writes_are_rejected`
- `matching::tests::release_identity_is_followed_and_compilations_stay_ambiguous`
- `matching::tests::retrieval_is_bounded_cached_and_surfaces_partial_failures`
- `matching::tests::resource_search_is_authenticated_and_never_writes_catalog`
- `resource_members_page::tests::http_requires_auth_and_serves_readonly_snapshot`
- `resource_page::http_tests::resource_http_enforces_auth_preserves_projection_and_rejects_stale_cursor`
- `services::tests::one_services_owns_workers_for_two_listeners_and_shared_shutdown`
- `unix_core::tests::bounded_protocol_rejects_oversized_and_malformed_requests`
- `unix_core::tests::client_rejects_wide_directory_or_socket_before_connect`
- `unix_core::tests::custom_control_directory_is_used_by_serve_and_owner`
- `unix_core::tests::stale_socket_rebuilt_only_after_lock_is_acquired`
- `unix_core::tests::same_uid_session_and_private_permissions`
- `work_catalog::tests::http_disconnect_interrupts_snapshot_query`
- `work_catalog::tests::http_routes_enforce_auth_and_preserve_page_and_detail_contracts`
- `work_catalog::tests::scoped_http_reads_require_auth_and_pin_lookup_revision`
- `mcp_session_tidies_the_catalog_without_touching_files`
- `requests_are_pinned_to_the_paired_collection`
- `pairing_creates_a_private_revocable_desktop_credential`

## 請 Claude 特別複查

1. **web 路徑邊界**：核心政策集中在 `web_projection.rs`，使用已匹配路由，未分類路由預設拒絕；所有 JSON 最後都掃描絕對路徑，圖片 bytes 保持原樣。請複查新增回應欄位、任意 metadata／錯誤文字與新的路徑編碼方式。字串採整段遮蔽，可能一併隱藏含路徑的正常敘述；32 MiB 的 web JSON 上限也屬刻意的 fail-closed 行為。來源根目錄很多時，字串比對及完整 JSON buffering 的成本值得實際量測。
2. **seq 範圍與頻率**：不串接 smart_catalog_state.revision，避免 exploration cache 導致 reload loop。請複查以後新增的 join／背景結果是否列入 TABLES 或顯式 bump，以及 files／jobs／source_watch 更新在大型掃描下的頻率。序號可在一個交易中增加多次，不表示使用者操作筆數。
3. **issue reconcile 時機**：為滿足純 GET 不 bump，問題結果改在啟動與既有 smart worker 發布；平時跟隨約 3 秒工作迴圈，worker 忙碌時可能較久。使用 `issues.catalog_seq` checkpoint，避免無變更時反覆全表 reconcile。請複查掃描與匹配期間的問題出現時機及大收藏的背景負載。
4. **目前畫面與草稿**：main.refresh 的 generation 現在傳到清單、標籤、關係與探索頁；手動清單／智慧清單在編輯時保留草稿，關閉編輯後讀新結果；ListExplorer 在背景讀取期間保持元件掛載。建議人工檢查「另一台修改時，本機正在編輯或看深層探索頁」的體驗及 CAS 提示。
5. **未完成的環境驗證**：25 個 socket 測試必須在允許 binding 的環境重跑；本次沒有用網路或權限繞過 sandbox，也沒有實作 PR 5 桌面 transport。未新增 crate、未執行 cargo fmt、未 commit，原有未追蹤的 node_modules 保留。
