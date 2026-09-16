# Galroon

**Galgame 收藏管理｜A local catalog for visual novel collections**

[繁體中文](#繁體中文) · [English](#english)

---

## 繁體中文

Galroon 是以 **Rust Core + React / TypeScript + Tauri 2** 開發的 Windows 桌面收藏管理工具。它將「作品、版本、資源包、實體檔案」分開管理：同一作品的不同版本集中在一張作品卡片下，共用資源也不需要複製多份檔案。

目前版本為 **0.0.2**，API **1**，收藏資料庫 schema **43**。主要功能已實作；目前仍屬開發版本，完整發佈驗收尚未完成。

### 下載

[下載 Windows x64 安裝包](https://github.com/llpgf/galroon/releases/download/v0.0.2/Galroon_0.0.2_x64-setup.exe) · [v0.0.2 發佈說明與校驗值](https://github.com/llpgf/galroon/releases/tag/v0.0.2)

此版本標示為 **Pre-release（預發佈）**。歷史 `v0.5.0` 屬於先前原型；請使用上述連結取得目前版本。

### 主要功能

- **掃描與匹配**：索引來源資料夾、監視檔案變更、匹配 VNDB 資料，保留手動修改與關聯修正。
- **收藏與探索**：瀏覽作品、人物、角色與公司，使用收藏狀態、標籤、手動清單及智慧清單管理收藏。
- **版本與資源**：管理不同版本、偏好版本、附加內容與跨作品共用的資源。
- **檔案操作**：預覽並確認整理、重複檔案隔離、復原、複製與封存檔解壓操作。
- **背景任務**：暫停、繼續、檢視任務歷史與分類摘要，並備份／還原收藏資料庫。
- **Core 與前端分離**：Windows 桌面程式可編輯，本機瀏覽器介面提供唯讀瀏覽。

### 使用範圍

- 本版以 **Windows x64** 為目標。遊戲安裝／啟動、NAS 上執行 Core、macOS 與遠端 TLS 部署不在本版範圍內。
- 活躍的 SQLite 收藏資料庫應放在本機磁碟，不應放在 SMB 共用資料夾或由兩個 Core 同時開啟。
- 掃描只建立索引；移動、隔離等實際檔案操作需要預覽與確認。
- 收藏備份包含目錄資料與手動修改，不包含遊戲檔案、存檔或登入憑證；還原收藏也不會撤銷磁碟上的檔案移動。

### 從原始碼執行

準備 Node.js / npm、Rust MSVC 工具鏈、Visual Studio C++ Build Tools、Python 3.11+ 與 WebView2。以下指令均在 repository 根目錄執行。

```powershell
git clone https://github.com/llpgf/galroon.git
cd galroon
npm ci
cargo fetch --locked
rustup component add rust-docs
```

開發模式需開啟兩個終端機：

```powershell
# 終端機 1：前端
npm run dev
```

```powershell
# 終端機 2：桌面程式與本機 Core
npm run desktop
```

產生 Windows 桌面程式與 NSIS 安裝包：

```powershell
npm run desktop:build
```

安裝包輸出到 `target/release/bundle/nsis/`。建置會產生前端與第三方聲明，並打包 Core、所需的 7-Zip 執行元件及操作文件。原始碼 repository 不包含已建置的安裝包。

### 文件與開發

- [文件索引](docs/README.md)
- [操作指南](docs/user/USER_GUIDE.md)・[支援範圍](docs/user/SUPPORT.md)
- [架構](docs/development/ARCHITECTURE.md)・[本機 API 契約](docs/development/API_CONTRACT.md)
- [開發說明](CONTRIBUTING.md)・[目錄結構](docs/development/REPOSITORY.md)

詳細技術文件目前主要為英文。建置檔、遊戲資料、資料庫、登入狀態、本機驗收紀錄及歷史打包腳本均排除於 Git。

---

## English

Galroon is a **Windows desktop catalog for visual novel collections**, built with a Rust Core, React / TypeScript and Tauri 2. It treats works, editions, resource packages and physical files as separate entities: multiple editions share one work card, and shared resources do not require duplicate files.

Current version: **0.0.2**, API **1**, catalog schema **43**. Core features are implemented; full release acceptance is still in progress. This is a development build.

### Download

[Download the Windows x64 installer](https://github.com/llpgf/galroon/releases/download/v0.0.2/Galroon_0.0.2_x64-setup.exe) · [v0.0.2 release notes and checksums](https://github.com/llpgf/galroon/releases/tag/v0.0.2)

This is a **Pre-release**. The historical `v0.5.0` belongs to the earlier prototype; use the links above for the current version.

### Features

- **Scanning and matching:** index source folders, watch file changes, match VNDB metadata and preserve manual metadata and relationship corrections.
- **Collection and discovery:** browse works, people, characters and studios; organize reading states, tags, manual lists and smart lists.
- **Editions and resources:** manage editions, preferences, extras and resources shared across works.
- **File operations:** preview and approve organization, duplicate isolation, restore, copy and archive extraction.
- **Background jobs:** pause and resume tasks, inspect history and entity summaries, and back up or restore the catalog.
- **Separate Core and clients:** edit through the Windows desktop application and browse read-only through a local browser.

### Scope

- This version targets **Windows x64**. Game installation/launching, a NAS-hosted Core, macOS and remote TLS deployment are outside this release scope.
- Keep the active SQLite catalog on a local disk, not an SMB share, and do not open it with two Cores.
- Scanning indexes files in place. Physical file operations such as moving or isolation require a reviewed confirmation.
- Catalog backups include catalog data and manual edits, not game files, saves or login credentials. Restoring a catalog does not undo filesystem moves.

### Build from source

Install Node.js / npm, Rust with the MSVC toolchain, Visual Studio C++ Build Tools, Python 3.11+ and WebView2. Run commands from the repository root.

```powershell
git clone https://github.com/llpgf/galroon.git
cd galroon
npm ci
cargo fetch --locked
rustup component add rust-docs
```

For development, use two terminals:

```powershell
# Terminal 1: frontend
npm run dev
```

```powershell
# Terminal 2: desktop application and local Core
npm run desktop
```

Build the Windows application and NSIS installer:

```powershell
npm run desktop:build
```

The installer is written to `target/release/bundle/nsis/`. The build generates frontend assets and third-party notices, then bundles Core, the required 7-Zip runtime and user/API documentation. Built installers are not included in source history.

### Documentation and development

- [Documentation index](docs/README.md)
- [User guide](docs/user/USER_GUIDE.md) · [Support boundaries](docs/user/SUPPORT.md)
- [Architecture](docs/development/ARCHITECTURE.md) · [Local API contract](docs/development/API_CONTRACT.md)
- [Development notes](CONTRIBUTING.md) · [Repository organization](docs/development/REPOSITORY.md)

Build output, game data, catalogs, login state, local acceptance records and historical packaging scripts are excluded from Git.

---

## 目錄結構 / Repository layout

```text
.
├── apps/desktop/src-tauri/   Windows desktop shell and packaging
├── crates/core/             Rust Core, catalog, jobs and APIs
├── src/                     React frontend and colocated tests
├── public/                  Public static assets
├── assets/                  Source artwork
├── docs/
│   ├── user/                User guide and support boundaries
│   ├── development/         Architecture, API and repository notes
│   └── features/            Detailed feature notes
├── scripts/                 Build, notice and repository helpers
├── third-party/             Notices, provenance and source archives
├── tools/7zip/              Required runtime and its license
├── Cargo.toml / Cargo.lock  Rust workspace and locked dependencies
└── package*.json            Frontend commands and locked dependencies
```

## 授權與第三方元件 / Licensing and third-party components

此份 Galroon 原始碼尚未選定專案授權；公開原始碼不代表另行授予開源授權。第三方元件仍遵循各自的授權，請保留其聲明、來源封存檔與雜湊紀錄。

A project license has not yet been selected for this Galroon source tree. Public source availability does not itself grant an open-source license. Third-party components retain their own licenses; preserve their notices, source archives and provenance.

See [third-party notices and sources](third-party/README.md) and the [7-Zip license](tools/7zip/runtime/License.txt).
