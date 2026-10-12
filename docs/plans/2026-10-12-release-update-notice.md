# Release update notice (phase 1: notify only)

Owner decisions (2026-10-12): follow Roon's model, where Core checks for updates and every client shows the result. Automatic checks are on by default and can be turned off in Settings. Only full releases count; pre-releases and drafts are ignored. Galroon only notifies and links to the release page. Automatic download and install (Tauri updater, signed releases) is phase 2 and is out of scope here.

## Core

New module `crates/core/src/updates.rs`.

- Source: `GET {api}/repos/llpgf/galroon/releases/latest`, where `api` defaults to `https://api.github.com` and `GALROON_UPDATE_API` overrides it (tests point it at a local axum fixture, in the same way as `GALROON_VNDB_API`). GitHub's `latest` endpoint already excludes drafts and pre-releases. Send `User-Agent: Galroon/<version>` and `Accept: application/vnd.github+json`, use a 20 s timeout, and send `If-None-Match` when an ETag is stored; 304 means unchanged. A 404 means there is no full release yet, which is a successful check with no update.
- Versions: parse `tag_name` with an optional leading `v` using the `semver` crate (already in Cargo.lock; add it to core's Cargo.toml). An unparseable tag, or one with a semver pre-release part, is never offered. An update is available only when latest > `CARGO_PKG_VERSION`.
- Trust: accept `html_url` only when it starts with `https://github.com/llpgf/galroon/releases/`; otherwise use `https://github.com/llpgf/galroon/releases`. Return `body` as plain text, truncated to 20,000 characters. Clients render it as text and never as HTML or markdown HTML.
- Device-local state in `device_dir/update-check.json`: `{version:1, enabled:bool, last_checked:unix_seconds|null, last_error:string|null, etag:string|null, latest:{version, name, url, notes, published_at}|null}`. Write it atomically with a temp file and rename, using `plans::validate_chain` and the symlink refusal from `network.rs`. A missing file means enabled with nothing checked. A damaged file is treated as missing and logged with `diagnostics::record`; it never stops Core from starting. This is not catalog data and is not included in backups.
- Worker: Services gains an update worker (start and request_stop, like the other workers). When enabled, it checks 30 s after start and then every 24 h. It skips the startup check when `last_checked` is less than 24 h old. Failures are stored in `last_error` and never retried more often than once per hour. A stop request ends the worker promptly, without waiting out a sleep.
- Install kind: `GALROON_INSTALL_KIND` env (`docker` | unset). The Dockerfile sets `ENV GALROON_INSTALL_KIND=docker`. Otherwise use `desktop` on Windows and `standalone` on other systems.

### API

- `GET /api/updates` (any authenticated role, `WebAccess::Unchanged`): `{current, install_kind, enabled, checking, last_checked, last_error, update_available, latest}`. `latest` is present only when it is newer than `current`.
- `POST /api/updates/check` (owner and desktop; web is already refused for non-GET): runs one check now, even when automatic checks are disabled, and returns the same body. Concurrent calls share one in-flight check, with no double request. Limit it to one GitHub request per 60 s; within that window, return the stored state.
- `POST /api/updates/settings` `{enabled:bool}` with `deny_unknown_fields` (owner and desktop): persists the setting and wakes the worker. Enabling with no recent check triggers a check.
- Add both routes to `WEB_ROUTES` and the route tables. `docs/development/API_CONTRACT.md` documents all three.

### Tests (Core, Mac: `cargo test -p galroon-core --lib --bins --tests`)

All against a local fixture server, with no network access:
- A newer full release is reported, and an equal or older one is not.
- 404 gives no update and no error. 304 reuses the stored `latest`. A 500 error or timeout is stored in `last_error` and keeps the previous `latest`.
- `v0.6.0-beta.1` and garbage tags are ignored. A foreign `html_url` is replaced.
- Disabled: the worker makes no request, but a manual check still does. The setting persists across `initialize`.
- A web session can GET `/api/updates` and gets 403 on both POSTs. A damaged `update-check.json` does not stop `initialize` or the server.

## Frontend

- `src/updates.ts`: API types and calls, plus `skippedVersion` per client in localStorage (`galroon.updates.skipped`, wrapped in try/catch) and a pure `shouldShowUpdate(state, skipped)` function. Add vitest tests.
- `UpdateBanner`: shown below `ConnectionBanner` when `update_available` and the version is not skipped. It shows "Galroon {version} is available", the release name, collapsible plain-text notes (`white-space: pre-wrap`), an **Open download page** button and a **Skip this version** button. Use a plain `<a target="_blank" rel="noreferrer">`, matching the existing VNDB links in WorkExperience.tsx. Add no new Tauri plugin. When `install_kind` is `docker`, the hint says to update the Core container image instead of downloading the installer.
- Version mismatch: when the client's `APP_VERSION` differs from `context.core.version`, show a small notice: "This app (x) and Core (y) are different versions; update both." It is independent of GitHub.
- Settings: a new `about` category with Info icon, shown to every role. It lists Core and client versions, the last check time or "never", the last error and the current state. Owner and desktop also get an **Automatically check for updates** toggle and a **Check now** button. Web is read-only.
- Fetch `/api/updates` on load and then every 6 h (the client only reads Core's state, so it never contacts GitHub).
- Add every string to the existing locales (`src/locales/zh-TW.ts` and the English defaults in `i18n.ts`). Keep the existing UI style and the 390 px layout without horizontal overflow.

## Docs

Add `docs/features/UPDATES.md`, covering behaviour, privacy (Core contacts api.github.com once a day while enabled and sends only the User-Agent; the IP is visible to GitHub) and how to turn it off. Add a short user-guide section in `docs/user/USER_GUIDE.md`.

## Out of scope

Automatic download or install, a pre-release channel, and update checks from clients directly.
