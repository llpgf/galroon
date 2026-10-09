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

Galroon must be running. Core listens on a new loopback port each start, so on Windows the server finds it through the existing identity-checked control pipe (same user, executable path verified against `galroon-core.exe` next to `galroon-mcp.exe`). Only the URL and library identity are kept from that exchange; the owner credential is discarded. After Core restarts, a refused connection triggers one rediscovery. `--url http://127.0.0.1:<port>` or `GALROON_URL` overrides discovery (required on non-Windows development hosts).

## Tools

| Tool | Effect |
| --- | --- |
| `galroon_overview`, `search_collection`, `get_work`, `list_resources`, `list_issues`, `list_jobs`, `list_lists`, `get_list`, `list_plans` | Read only. Output is compacted (≤60 works per page, long text and arrays clipped, 100 KB cap). |
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

`cargo test -p galroon-mcp` (macOS development host) runs the built binary over stdio against a real in-process Core on an ephemeral loopback port with a generated catalog: pairing and `check`, 0600 credential, owner-only route denied (403), reused code leaves the credential intact, revocation message, protocol negotiation and errors, collection search/tag/studio filters, stale-revision rejection, match/update/list flows, organize preview with source files byte-identical and the plan left `ready`, and the 409 collection pin. The Windows discovery path compiles only on Windows and has not yet been exercised; installer packaging (`galroon-mcp.exe` resource) has not been rebuilt.
