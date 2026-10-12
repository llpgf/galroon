# AI match suggestions

Core and MCP record suggestions; the review UI lets a person confirm them. Suggestions do not change automatic matching or ranking.

An assistant investigates an unmatched resource with `list_resources`, `list_resource_files` and `search_vndb`, then calls `suggest_match` with its current resource revision, one or more VNDB work IDs, query and rationale. This creates a proposal only: no work, edition, binding or manual history is created. Use this tool for review cases instead of `match_resource` (which still binds directly).

Core fetches the actual works by ID through the same VNDB provider cache and 1.6-second request spacing as matching. Each work must have its own title, alttitle, multilingual title/Latin title or alias equal, after `matching::normalized`, to the query or a prepared resource/member title. Preparation shares automatch's catalog input, including name cleaning and bounded member hints. Release titles alone cannot verify a proposal. Brand or date evidence must corroborate through the existing matching predicate, unless the input contains neither. This predicate accepts an agreeing brand **or** date. All proposed works must pass for the suggestion to be verified. Per-work verification reasons are stored; rationale is supplied by the assistant and is not proof.

A person can confirm either kind individually. Batch confirmation accepts verified suggestions only and reports success or failure for each ID. Each confirmation rechecks the resource revision and open state inside one transaction, creates missing works through the manual dialog's work-creation helper, and uses its manual binding/history helper. A stale confirmation returns HTTP 409 with no catalog changes. Failures, including history-write failures, roll back the whole suggestion's confirmation.

For several works, the first is the resource's physical primary, and every reference has role `main`. Each is a complete work contained in the resource, so `extra` would misrepresent the relationship. No edition is invented: `release_id` stays null. The resource and its files remain single records, referenced by each work. Existing VNDB works are reused, including the existing manual path's merge resolution. The `resource_history` entry and `manual.bind` event record a person's action; existing bound-resource checks keep rescans and rematches from replacing it.

Schema 47 stores proposals in `match_suggestions`. Only one open suggestion may exist per resource; a replacement dismisses the preceding open proposal. Any resource revision change makes open suggestions stale, including regrouping and manual binding. A rescan increments the revision when existing file size/mtime changes or a file becomes missing; new members already increment it. An unchanged rescan retains suggestions. Confirmed and dismissed history is retained.

Owners on loopback and paired desktops on loopback/LAN may create, confirm and dismiss. Web sessions can only read, through the common path-redacting projection. Source attribution derives from the authenticated session (`mcp:<paired device name>` or `owner:owner`); it is not caller-supplied. Normal collection/device identity pins apply.

See [API contract](../development/API_CONTRACT.md#ai-match-suggestions-schema-47-phase-3a) for request and response shapes and [MCP](MCP.md) for assistant setup.

In Organize and Activity issues, open suggestions show an AI suggestion badge marked verified or unverified, all proposed work titles and VNDB IDs, and an expandable rationale. Confirm uses the reviewed resource revision. A changed suggestion shows a conflict and reloads the list for review; it is never retried automatically. Dismiss removes the proposal without binding.

Confirm all verified includes open verified suggestions across all pages, independent of the visible resource filter. Requests are limited to 60 items each and the UI retains each result, including failures, before refreshing. Web sessions have no suggestion write controls. Resources bound to multiple distinct works show “Collection” / 「合集」 on each work page. Work assets already count physical resources and files once, so no count or backend change is needed.

`tests/e2e/us5-suggestions.spec.ts` starts from the intake snapshot and covers confirmation, rescan persistence, verified-only batches, stale conflicts, dismissal and shared resources. The two-work story temporarily groups two correctly identified golden resources through the catalog API, then restores their boundaries before golden grading. It never edits the source files or golden expectations.

Phase 3b separately admits deterministically proven pure compilations automatically (US-5.2 acceptance 1 and 5); see [matching](MATCHING.md#pure-compilations--algorithm-14-us-52). It shares `editions::set_bindings_in`, the binding/history primitive underlying `manual_bindings_in`, but records an automatic match attempt. AI suggestions still require a person to confirm.
