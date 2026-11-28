# Release updates (built-in signed updates & notices)

Core checks for new full Galroon releases. Every authenticated client can read its cached result; clients never contact GitHub for background checks. Drafts, prereleases, invalid version tags and versions equal to or older than Core are not offered.

Automatic checks are enabled by default. Core waits 30 seconds after startup, skips a recent check, and then checks every 24 hours. Failed checks retain the previous release and show the last error; retries wait at least one hour. Concurrent manual checks share one request, and manual requests are limited to once per 60 seconds (once per hour after a failure). A 404 means there is no full release yet, not an error. GitHub ETags avoid downloading unchanged release details.

## Built-in coordinated updates (Roon-style)

When running the Docker launcher container (`docker/entrypoint.sh`, `GALROON_LAUNCHER=1`) and/or the signed Windows Tauri desktop app, writable roles (owner, password-authenticated owner web, and paired desktop) can trigger one-click in-app updates from the update banner or **Settings → About**:

1. **NAS Docker Core + Windows Desktop**: Desktop first triggers `POST /api/updates/apply` on NAS Core. Core streams `galroon-nas-x86_64.tar.gz` and its `.sig` from the trusted GitHub Release origin, verifies SHA-256, Ed25519 Minisign signature, and strict SemVer monotonicity (`new > current`), safely extracts `galroon-core` and `web/` (rejecting symlinks and path traversal), stages them under `/data/runtime/releases/<version>/`, writes `/data/runtime/pending`, gracefully pauses/drains jobs and checkpoints SQLite WAL, and exits with code `42`.
2. **Automatic rollback**: `docker/entrypoint.sh` starts the staged release. Once Core initializes its database, listeners, and control socket, it marks `/data/runtime/healthy`, promotes `current`, and prunes older releases (retaining `current` and `previous`). If a newly applied version exits before becoming healthy, `entrypoint.sh` automatically rolls back to the previous healthy release (or the container's built-in `/opt/galroon/galroon-core`) and records the rollback reason for the UI.
3. **Desktop update**: Once NAS Core reports the new version on `/api/health` / `/api/updates` (or immediately in local Windows mode), the Windows desktop app uses `tauri-plugin-updater` to download and verify the signed NSIS installer. In local Windows mode, it downloads the installer first, then stops the local `galroon-core.exe` cleanly via `prepare_desktop_update` (waiting for `core.lock` release) before running the passive installer and relaunching.
4. **Web UI sessions**: After NAS Core restarts with the new version, browser tabs automatically reload to pick up the updated static `web/` bundle served alongside the new `galroon-core` binary.
5. **Fallback**: The **Open download page** link always remains available, and older Docker containers without `entrypoint.sh` report `can_self_update: false` and show the container-image hint.

## Signing & release automation

Both Windows installers (`latest.json` + `*-setup.exe.sig`) and NAS bundles (`nas-latest.json` + `galroon-nas-x86_64.tar.gz.sig`) are signed with a single Minisign (Ed25519) keypair held only in GitHub Actions Secrets (`TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`). The matching public key is embedded in `apps/desktop/src-tauri/tauri.conf.json` and `crates/core/src/updates.rs`. The `.github/workflows/release.yml` workflow builds both targets on `v*` tags, verifies signature/keypair consistency before publishing, and uploads all signed artifacts to the GitHub Release.

## Privacy and storage

While automatic checks are enabled, Core contacts `api.github.com` once a day (`User-Agent: Galroon/<version>`, `Accept: application/vnd.github+json`, optional `If-None-Match`). Core sends no collection data, file paths, device IDs, passwords or access tokens. Disable **Automatically check for updates** to stop automatic requests.

Core stores preferences, the ETag, release details and check results in device-local `update-check.json`, separately from catalog data. Catalog backups do not contain this file. The per-client skipped version is kept in localStorage as `galroon.updates.skipped` when storage is available.

## Deployment overrides

- `GALROON_UPDATE_API`: overrides the release check API base (default `https://api.github.com`).
- `GALROON_UPDATE_DOWNLOAD_BASE`: overrides the release asset download base (default `https://github.com/llpgf/galroon/releases/download`).
- `GALROON_INSTALL_KIND=docker`, `GALROON_LAUNCHER=1`, `GALROON_RUNTIME_DIR=/data/runtime`, `GALROON_WEB_DIR`: configured automatically by `docker/Dockerfile` and `docker/entrypoint.sh`.
