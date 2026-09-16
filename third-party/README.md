# Third-party components

`THIRD_PARTY_NOTICES.txt` contains the preserved license and notice texts for the locked Windows Cargo dependency graph, npm packages not marked development-only, the bundled fonts, and 7-Zip. Build dependencies and optional nested source notices are included conservatively. Their inclusion does not mean every listed implementation is linked into the executable.

`inventory.json` records versions, declared licenses, source locations and notice hashes. Files missing from published crate archives are recovered at the upstream commit recorded by that crate; `upstream/` retains those texts and their provenance. The selectors crate declares MPL-2.0 in its source header but omits the full text; its entry preserves the header and the standard MPL text supplied by the locked cssparser crate.

`sources/` contains unmodified source archives for the MPL-2.0 crates and 7-Zip 26.03, with URLs and SHA-256 hashes in `sources/manifest.json`. A `.crate` file is a gzip-compressed tar archive. These sources remain governed by their upstream licenses. You may unpack and modify them under those licenses. The archive includes its build files. Galroon uses unmodified upstream 7-Zip executable/DLL files as a separate helper; it does not incorporate a modified 7-Zip build.

`supplemental/` preserves the full 7-Zip LGPL and unRAR texts from its source distribution, and SQLite's source notice. The unRAR restriction is included in the upstream notice; this package does not grant rights beyond the upstream terms.

To regenerate from the project checkout, use Python 3.11+, installed npm dependencies and the fetched Cargo registry:

```powershell
python scripts/generate_notices.py
```

For a new dependency whose published crate omits license files, `--fetch-missing` retrieves public upstream license files at its recorded commit. Review and retain the new provenance. If a source archive is missing or differs from the lockfile checksum, generation fails; update the source package and manifest before packaging. Normal builds use the retained upstream files without network requests from this script.

Official upstream references: [7-Zip downloads and source](https://www.7-zip.org/download.html), [Mozilla Public License 2.0](https://www.mozilla.org/en-US/MPL/2.0/). Microsoft WebView2 is a separately installed Microsoft runtime, not a component relicensed by this notice file.

`rust/<version>/COPYRIGHT-library.html` and its referenced license texts are copied from the matching Rust toolchain documentation. The notice inventory includes these separately from Cargo packages. `supplemental/NSIS-3.11-COPYING.txt` preserves the installer engine notices, including the upstream LZMA exception. Regeneration requires the matching `rust-docs` component (`rustup component add rust-docs` if absent).
