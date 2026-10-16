# Owner password login from LAN browsers

Date: 2026-10-16. Owner decision: a browser that signs in with the owner password
(over loopback or LAN HTTPS) can edit; sessions last 30 days. Paired `web` browsers stay
read-only. Goal: use Galroon from the Mac by opening the NAS Core in a browser.

## Model

- Password login issues a new session role `web_owner` (sessions.role has no CHECK, so
  no migration; pairing codes are unchanged and can never produce `web_owner`).
- Existing Core checks are `role=="web"`, so `web_owner` gets desktop-level permissions
  automatically. Audit every role check in `crates/core/src` (`"web"`, `"desktop"`,
  `"owner"`): `web_owner` behaves like `desktop`, never like `owner`. Owner-only routes
  (`local()`: configure, change password, pairing codes, network, devices) stay
  owner-bearer only.
- `web_owner` responses skip `web_projection` (real paths, no Deny routes) and are not
  read-only.
- Old password-login sessions already stored as `web` stay read-only until they expire
  (12 h); no data migration.
- Lifetime: `web_owner` 30 days (`OWNER_WEB_LIFETIME`), cookie `Max-Age` to match.
  Paired `web` stays 12 h. Changing the owner password still revokes every session.
  Login rate limit unchanged (10/min per IP, existing budgets).

## LAN

- Remove the "password login not allowed over LAN" blocks for `/api/access/login` in
  `access::transport_host` and `access::guard`. Keep the owner-bearer LAN block.
- Login over LAN still needs `origin_allowed` (`https://{host}`) and the peer address,
  as today. Cookie keeps `Secure` on LAN.

## CSRF

In `access::guard`: every non-GET/HEAD request authenticated by cookie (`web` logout and
all `web_owner` writes) must pass `origin_allowed`; missing Origin is rejected.
SameSite=Strict does not separate ports on the same host.

## owner CLI

Add `galroon-core owner change-password` (password on stdin, like `setup`, calling
`setup(db,pw,true)`); wire it in `scripts/nas/owner.sh` (stdin/no-echo handling like
setup) and the usage strings.

## Frontend

- `api.ts` `AccessIdentity.role` adds `'web_owner'`; UI write gating already uses
  `can_write`. Read-only note stays for `role==='web'`.
- Make sure the browser login form is offered on LAN too (check `WebLogin` and any
  transport==='lan' gating); paired-code login stays available.
- Desktop-only UI (`isDesktop`: folder picker, restore, desktop diagnostics) unchanged;
  typed paths work.

## Docs

README (both languages: "本機瀏覽器介面提供唯讀瀏覽" and the English equivalent),
`docs/development/NAS.md`, `docs/development/API_CONTRACT.md`, LAN_CORE_PLAN if it states
LAN password login is blocked. Note the self-signed certificate: check the fingerprint,
then trust it once in the Mac keychain.

## Tests

- LAN password login succeeds with Origin `https://{host}`; wrong/missing Origin 403;
  rate limit still applies.
- Login session: role `web_owner`, `me` → `can_write:true, can_manage_access:false`,
  expires ≈ now+30 d, cookie Max-Age 2592000.
- `web_owner` POST with allowed Origin succeeds; no Origin 403; foreign Origin 403.
- `web_owner` GET of a Project route is unprojected; a Deny route (e.g. `/api/issues`)
  is allowed.
- `web_owner` cannot use owner-only routes.
- Paired `web` session: still read-only and projected; 12 h lifetime.
- Bearer token matching a `web_owner` session row is rejected.
- Owner-bearer over LAN still blocked.
- `owner change-password` revokes sessions and the new password works.
- Update existing tests that asserted LAN login is blocked or that login yields `web`.
- Frontend vitest for any changed login gating.

## Out of scope

Browser restore, browser folder picker, Mac Tauri client, pairing-code changes.
