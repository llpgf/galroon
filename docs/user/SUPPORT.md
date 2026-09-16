# Galroon Windows support and validation boundary

Applies to product 0.0.2, catalog schema 42. This document describes the development build, not completed MVP acceptance.

## System and runtime

| Item | Current requirement or evidence |
|---|---|
| Architecture | Windows x64; Core and Desktop are built for x86_64-pc-windows-msvc. No ARM64/x86 package is supplied. |
| Measured host | Windows build 10.0.26200, Intel i7-14650HX, 16 cores/24 threads, about 64 GiB RAM. This is a test host, not a minimum RAM requirement. |
| Minimum supported Windows version | Not yet established by clean-machine acceptance. Do not infer support for older Windows from a successful current-host build. |
| Desktop rendering | Microsoft Edge WebView2 Runtime. The installer uses Tauri's default WebView2 setup path; this package does not embed a fixed WebView2 runtime. Missing-runtime/offline installation still needs clean-machine verification. |
| Core storage | Active SQLite catalog on a local disk, one Core per catalog. Remote active databases and multiple writers are unsupported. |
| Browser | Local read-only Web; 390px and 1200px Chromium viewport checks recorded. Physical mobile-device and broad browser compatibility remain unverified. |
| Network | Loopback Core access only. Online provider metadata/artwork needs network access; available cached data can be browsed offline. NAS/Mac/LAN/TLS deployment is outside this release scope. |
| Installer | Unsigned development NSIS installer; current-host in-place version upgrade was tested. Clean install, uninstall and minimum-OS acceptance remain open. |

Install WebView2 from [Microsoft's runtime download page](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) if it is missing. Microsoft documents both online bootstrapper and offline standalone choices; their availability does not certify this Galroon package on an untested OS.

## File and archive behavior

| Input or operation | Implemented behavior | Evidence boundary |
|---|---|---|
| Ordinary files and directories | Index in place; selected local copy, reviewed organization/undo, duplicate hashing/isolation/restore. No game execution. | Generated fixture lifecycle and actual Windows lock/journal failure recovery tested. |
| ZIP and 7z | Bundled 7-Zip 26.03 listing, test and extraction into staging; encrypted archives require a password. | Generated actual archives, encrypted 7z, corruption and unsafe entry rejection tested. |
| ISO | 7-Zip extraction; Unicode/Joliet, empty entries and associated cue handling have generated corpus evidence. | Not all disc filesystems/images are certified. MDS/MDF/CUE/BIN catalog classification is not a promise of extraction or mounting. |
| Split 7z/ZIP and RAR volumes | Recognized volume naming, group selection, missing-first/gap/duplicate checks and archive test before multipart publication. | Actual split 7z tested; RAR and numbered ZIP naming coverage does not prove their complete real-archive matrix. |
| Unicode and long paths | Unicode catalog paths and generated NTFS paths beyond 260 UTF-16 units tested through move/undo/copy/extraction. | Not a guarantee for arbitrary UNC shares, filesystem component limits or every path race. |
| Staging failures | No successful publication until verification; validated copy prefix can resume; retry after a recoverable lock/journal error tested. | Actual disk-full and broad disconnected-share fault cases remain open. |

The capability endpoint's `zip`, `7z`, `rar`, `iso` list describes available helper support. It is not a claim that every format variation passed Galroon acceptance. Symlinks/reparse paths, escaping archive names and conflicting destinations can be rejected deliberately. Never treat a partial task or a known filename alone as proof of complete content.

## Data, scale and limits

Catalog backup preserves the catalog and personal metadata, not games/saves, credentials, artwork cache or in-flight device transfers. Restore protects the current catalog, uses an isolated restored directory and disables automatic watching/matching. A catalog restore does not undo disk moves.

100,000 generated works have separate release-Core smart-list timing evidence, including full conversion and concurrent paging. That fixture contains no physical game library. It does not certify all native UI interactions, cold startup, overall RSS or arbitrary real-world collections. Final performance thresholds and independent human matching/reference acceptance remain open.

The English interface retains translation foundations. Pseudo-locale, complete keyboard/accessibility, five independently confirmed real works, full native restore/connection/write interactions and clean-machine tests remain release gates.

## Building and notices

The current build environment uses Node.js 24.12.0, Rust 1.93.1 with MSVC tools, Python 3.11+ for notice generation, and bundled 7-Zip 26.03. These are observed build tools, not independently established minimum compiler versions. Use the lockfiles with `npm ci` and `cargo fetch --locked`; then `npm run desktop:build`. Notice generation uses cached sources and fails for missing notice/source material.

Installed `third-party/` contains the dependency notice inventory and source archive manifest. Its source URLs and checksums identify retained third-party materials. [7-Zip's official download page](https://www.7-zip.org/download.html) also provides its source releases. Upstream license texts remain authoritative.

See [USER_GUIDE.md](USER_GUIDE.md) for operation and recovery steps and [API_CONTRACT.md](../development/API_CONTRACT.md) for the client protocol. Repository acceptance ledgers record individual test evidence; no single package or test count closes all release gates.
