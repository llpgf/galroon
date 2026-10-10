# MCP server (AI assistants)

Status: source increment 2026-10-09. `galroon-mcp` is a [Model Context Protocol](https://modelcontextprotocol.io/) server that lets an AI client (Claude Desktop, Claude Code or other MCP clients) read and tidy a Galroon collection through the running Core. It is a separate stdio process; Core, schema and API1 are unchanged.

## Setup

1. Open Galroon. In Settings > Access, set up the owner password if needed, then create a pairing code (for example named `AI assistant`).
2. Redeem it once from a terminal in the Galroon install folder:

   ```powershell
   .\galroon-mcp.exe pair ABCD-1234-EF56
   ```

   The resulting paired-desktop credential is written to `%LOCALAPPDATA%\galroon-mcp\credential.json` (never to stdout, the catalog or backups). `galroon-mcp check` verifies it.
3. Register the server with the MCP client. Claude Desktop (`claude_desktop_config.json`):

   ```json
   {"mcpServers": {"galroon": {"command": "C:\\path\\to\\Galroon\\galroon-mcp.exe"}}}
   ```

   Claude Code: `claude mcp add galroon -- "C:\path\to\Galroon\galroon-mcp.exe"`.

   Codex CLI: `codex mcp add galroon -- "C:\path\to\Galroon\galroon-mcp.exe"`. For a one-off run without editing `~/.codex/config.toml`, pass `-c 'mcp_servers.galroon.command="..."'` (plus `args` / `env`) to `codex exec`, and close stdin (`< /dev/null`) in scripts, or `codex exec` waits for more input.

Galroon must be running. Core listens on a new loopback port each start, so on Windows the server finds it through the existing identity-checked control pipe (same user, executable path verified against `galroon-core.exe` next to `galroon-mcp.exe`). Only the URL and library identity are kept from that exchange; the owner credential is discarded. After Core restarts, a refused connection triggers one rediscovery. `--url http://127.0.0.1:<port>` or `GALROON_URL` overrides discovery (required on non-Windows development hosts).

## Tools

| Tool | Effect |
| --- | --- |
| `galroon_overview`, `search_collection`, `get_work`, `list_resources`, `list_resource_files`, `list_issues`, `list_jobs`, `list_lists`, `get_list`, `list_plans` | Read only. Output is compacted (≤60 items per page, long text and arrays clipped, 100 KB cap). `list_resources` includes Core's auto-match review reason for unmatched resources; `list_resource_files` lists the file names, sizes and availability inside one resource, relative to its source folder, so the assistant can identify a work from names, brands, dates and store IDs before searching VNDB. |
| `search_vndb` | Read only; queries VNDB through Core with optional resource file-name hints. |
| `match_resource` | Binds a resource to an existing work, or creates the work from a VNDB candidate exactly as the match dialog does, then binds. Resource revision required. |
| `update_work` | Status, favorite, notes. Only these fields are sent, because Core merges the body into work overrides. Work revision required. |
| `create_list`, `add_to_list` | Manual lists via the existing request-ID/revision receipts. |
| `preview_organize` | Creates an organize plan (files are hashed, nothing moves). The user approves and executes it in Galroon. |

Not exposed: plan approve/execute, quarantine/restore, undo, merge/split, regroup, root remap, backup/restore, scans, matching start, access management and arbitrary API calls. Every tool is annotated `destructiveHint: false`.

## Boundaries

- The credential has the paired-desktop role: catalog/file management routes are allowed by Core, owner access administration is not. Revoke it any time in Settings > Access; the next call reports that pairing is needed again.
- Every request carries `x-galroon-library` with the paired collection ID. If another collection is open, Core answers 409 and the tool reports the mismatch instead of editing the wrong catalog.
- Only `http://127.0.0.1:<port>` is accepted; no proxies or redirects. Pairing sends a same-origin `Origin` header as Core requires.
- IDs passed to path segments are restricted to `[A-Za-z0-9_:-]`.
- The credential file is not stored in Windows Credential Manager yet (desktop pairings are). It relies on the per-user `%LOCALAPPDATA%` ACL; on Unix it is created `0600`. `GALROON_TOKEN` / `GALROON_MCP_TOKEN_FILE` override it.
- Protocol: newline-delimited JSON-RPC 2.0 on stdio, MCP versions 2025-11-25, 2025-06-18, 2025-03-26 and 2024-11-05; tools capability only; no batching.

## Evidence

`cargo test -p galroon-mcp` (macOS development host) runs the built binary over stdio against a real in-process Core on an ephemeral loopback port with a generated catalog: pairing and `check`, 0600 credential, owner-only route denied (403), reused code leaves the credential intact, revocation message, protocol negotiation and errors, collection search/tag/studio filters, stale-revision rejection, match/update/list flows, organize preview with source files byte-identical and the plan left `ready`, and the 409 collection pin. The same suite passes on Windows 11 (x86_64-pc-windows-msvc, Rust 1.93.1).

Codex acceptance on macOS (2026-10-10, Codex CLI 0.162.1, release `galroon-mcp` against a `ui_core` catalog of the 31-resource / 1,547-file / 122 GB sandbox collection after scan and auto-matching):

- **Identify.** Codex used only MCP tools (`list_resources`, `list_resource_files`, `search_vndb`). It correctly classified the three unmatched resources: a folder of mixed bonus archives as not a work, and two folders, one with a vocal-CD bonus and update archives, as v59911.
- **Match.** `match_resource` created the work with the requested edition and confirmed it through `get_work`.
- **Lists and status.** It created a list and added all six works by two studios, then set a play status.
- **Organize preview.** A preview of a 4.9 GB resource first failed: the MCP client gave up after 60 s while Core kept hashing, which left an orphan plan, and the retry created a second one. Since then, `preview_organize` waits up to 30 minutes and reuses a ready plan that covers the same files and destination. A 12.1 GB preview then completed in about five minutes and created exactly one plan.

A before/after manifest of the source collection was identical, and the managed folder stayed empty.

`tests/windows_live.rs` (ignored by default; needs `GALROON_TEST_CORE`, `GALROON_TEST_SOURCE`, `GALROON_TEST_STATE`) is the native acceptance run. On 2026-10-09 it passed against a release `galroon-core.exe` and 12 real files (3.65 GB, four Japanese-named folders): scan, `pair` and `check` through control-pipe discovery with no `--url`, VNDB search using file-name hints (a folder named only by its store ID matched v57625 as `exact_title`), `match_resource` creating the work from the candidate, `update_work`, an organize preview of 2 files into a managed folder with nothing moved, a clear error while Core was stopped, and the same MCP process reconnecting after Core restarted on a new port. Source file sizes and modification times were unchanged. Installer packaging (`galroon-mcp.exe` resource) has not been rebuilt.
