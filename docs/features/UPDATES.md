# Release update notices

Core checks for new full Galroon releases. Every authenticated client can read its cached result; clients never contact GitHub for checks. Drafts, prereleases, invalid version tags and versions equal to or older than Core are not offered. This phase only notifies and opens the release page: it does not download or install anything automatically.

Automatic checks are enabled by default. Core waits 30 seconds after startup, skips a recent check, and then checks every 24 hours. Failed checks retain the previous release and show the last error; retries wait at least one hour. Concurrent manual checks share one request, and manual requests are limited to once per 60 seconds (once per hour after a failure). A 404 means there is no full release yet, not an error. GitHub ETags avoid downloading unchanged release details.

In **Settings → About**, all roles can see Core and client versions, update status, the last check time and the last error. Owner and paired desktop sessions can turn **Automatically check for updates** off or on and use **Check now**, even when automatic checks are off. Re-enabling checks wakes Core and checks when its cached result is no longer recent. Browser sessions are read-only.

An available release appears above the main view with its name and collapsible plain-text notes. **Open download page** opens the project's GitHub release page. **Skip this version** hides that version on this client; newer releases can still appear, and Settings keeps showing the available release. A separate notice appears when the app and Core versions differ, even without internet access. Update both components as appropriate. Docker users should update the Core container image instead of downloading a desktop installer.

## Privacy and storage

While automatic checks are enabled, Core contacts `api.github.com` once a day. The request identifies Galroon and its Core version through `User-Agent: Galroon/<version>`; GitHub also sees the Core computer's public IP address. Core sends no collection data, file paths, device IDs, passwords or access tokens. The request also includes the GitHub JSON `Accept` header and, when cached, `If-None-Match`. Manual checks and retries after failures can make additional requests under the limits above. Disable **Automatically check for updates** to stop automatic requests.

Core stores preferences, the ETag, release details and check results in the device-local `update-check.json`, separately from catalog data. Catalog backups do not contain this file. Missing or damaged state defaults to automatic checks being enabled; damaged state is logged and never prevents Core startup. The per-client skipped version is kept in localStorage as `galroon.updates.skipped` when storage is available.

## Deployment

`GALROON_UPDATE_API` overrides the API base (default `https://api.github.com`); Core requests `/repos/llpgf/galroon/releases/latest` with a 20-second timeout. Tests pass a local axum fixture base explicitly and do not change process environment or contact GitHub. `GALROON_INSTALL_KIND=docker` is set by the Dockerfile. Otherwise Core reports `desktop` on Windows and `standalone` on other systems.

Only links beginning with `https://github.com/llpgf/galroon/releases/` are accepted from the API; other links fall back to the project's releases page. Notes are limited to 20,000 characters and rendered as text, never HTML or markdown HTML. Signed releases, an automatic installer and a prerelease channel belong to a later phase.
