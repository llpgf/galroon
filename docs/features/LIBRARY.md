# Collection browsing

The collection shows one card per work. Shared editions or resources do not duplicate that card. Search matches normalized title, original title, aliases, studio and tags; multiple words must all match.

Studio/brand, release year, edition language, tag and resource availability filters combine with search and reading state. Empty metadata is explicitly selectable as **Not specified**. Studio labels are currently treated as whole values: a provider's comma-separated developer label is not split into separate company identities.

Resource availability is based on indexed file states and the source status at the latest refresh. A work may match several states when it has multiple resources. A resource is available only when its source is online, it has indexed files and none are missing or unverified. An offline source never makes a resource available. This is a catalog check, not a fresh integrity or full-hash guarantee.

**Title display** switches between the corrected display title and original title, with a fallback when one is empty. The preference is stored on the frontend device and does not alter metadata. Sorting supports title and release year. Cards are rendered in pages of 60; entering a work and returning preserves filters and the current page. Changing search, filters, title display or sort starts at page one.

The Core currently returns complete work/resource/edition lists; pagination limits rendered cards, not API transfer size. People/related-work navigation, server pagination and very large work-count acceptance remain unfinished.

## Verification

- `npm test`: six collection-model tests plus three API session tests.
- `test-output/ui-library/report.json`: live browser assertions for pagination, exact combined results, all availability states, unknown metadata, alias/fullwidth search, original title persistence and Back behavior. Desktop and 390px screenshots were inspected.
- `test-output/ui-library/scale-report.json`: 6,600 generated tiny files, 120 resources and 121 works. Three metadata-only scans took 14,285 / 17,097 / 13,760 ms. Twenty warm API samples per route had p95 below 41 ms. This does not measure cold caches, peak memory, real-game throughput or identification accuracy.

All fixture file changes occurred inside `test-output/ui-library`. The supplied game originals were not modified.
