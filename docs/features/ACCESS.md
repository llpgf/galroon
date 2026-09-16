# Local owner and device access

Status: implemented and tested in the schema v5 Core and frontend source. The current NSIS installer predates this work; desktop pairing client credential storage and remote TLS deployment remain pending.

The local Windows owner is established by the same-SID, executable-validated named-pipe control channel. Its random bearer credential lives for that Core process and is not written to SQLite. Only this identity can initialize or replace the owner password, create pairing codes, list devices, or revoke them. Password setup is atomic and refuses replacing an existing owner through the initial setup route.

Passwords use the existing RustCrypto Argon2 0.5 dependency, with a fresh random salt and PHC verifier. Password replacement invalidates every browser/desktop session and pending pairing code. Password hashing for public sign-in is limited to one concurrent request; persistent attempt counters limit login and pairing redemption to ten requests per minute each. Archive passwords remain unrelated and memory-only.

Browser login issues a collection-specific HttpOnly, SameSite=Strict cookie valid for twelve hours. The response body contains the session ID, role and expiry, without the credential. Browser JavaScript keeps no bearer token in storage. API requests require authentication; browser sessions cannot call mutation routes, irrespective of user-agent or screen size. Logout revokes the session and expires its cookie. Authentication failure returns the browser UI to sign-in on its next poll. API responses prohibit caching.

Desktop pairing uses a random twelve-hex-character code with a five-minute expiry. Redemption is atomic and single-use, returning a random revocable device credential valid for ninety days. SQLite stores only its SHA-256 verifier. A paired desktop can manage catalog/files but cannot administer owner access. The local owner's device list identifies role, last use, expiry and revocation state. A desktop pairing/redemption screen and Windows Credential Manager persistence still need implementation; current acceptance exercises redemption through the HTTP protocol.

The Core currently binds only loopback and rejects non-loopback Host headers. Browser login, pairing redemption and cookie logout check Origin; cross-origin development access is restricted to the existing local development/Tauri origins. The production frontend is served by the Core from its adjacent `web` directory. It receives a restrictive CSP, frame-ancestor prohibition, no-referrer policy and nosniff header. Local HTTP cookies intentionally do not set Secure; enabling remote access requires TLS, Secure cookies and an explicit trusted-origin configuration. No listener or firewall was widened as part of these changes.

Schema v5 adds device session fields, hashed pairing codes and attempt counters. Export and migration snapshots purge session verifiers, owner password settings, pairing codes and attempt counters, with secure-delete/VACUUM. Restoring a backup requires setting up browser access again through the local owner. Existing game files are unaffected.

## API contract

| Route | Access | Result |
| --- | --- | --- |
| `GET /api/access/status` | Public | Owner configured flag |
| `GET /api/access/me` | Authenticated | Role and write/access-management capabilities |
| `POST /api/access/setup` | Local owner | First password setup |
| `POST /api/access/password` | Local owner | Replace password and revoke sessions/codes |
| `POST /api/access/login` | Allowed Origin | Read-only browser cookie |
| `POST /api/access/logout` | Authenticated; Origin for cookies | Revoke current session |
| `POST /api/access/pairing` | Local owner | Named one-use code |
| `POST /api/access/pair` | Allowed Origin | Desktop credential from code |
| `GET /api/access/devices` | Local owner | Session inventory without verifiers |
| `POST /api/access/devices/{id}/revoke` | Local owner | Immediate revocation |

## Evidence and references

Core tests cover single-use/expiry, password replacement, backup sanitization, session restart persistence, rate counters, hostile Origin/Host rejection, cookie flags, 26 denied mutation routes, desktop permissions and immediate revocation. `test-output/ui-access/report.json` records live owner setup, paired-device revocation, production Web login/reload, direct write rejection, mobile layout and Web return to sign-in after revocation. These are local development results, not remote deployment acceptance.

Implementation references: [RustCrypto Argon2 0.5.3](https://docs.rs/argon2/0.5.3/argon2/), [OWASP Session Management](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html), and [OWASP CSRF Prevention](https://cheatsheetseries.owasp.org/cheatsheets/Cross-Site_Request_Forgery_Prevention_Cheat_Sheet.html).
