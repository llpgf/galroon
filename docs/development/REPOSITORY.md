# Repository organization

The repository root is the directory containing `Cargo.toml` and `package.json`. Its local folder name can differ from the future GitHub repository name. Existing source, build and installed-app paths were retained during preparation.

## Committed structure

- `src/`, `crates/`, `apps/`, `assets/`, `public/`: application source and source assets.
- `docs/user/`: user guide and support boundaries.
- `docs/development/`: architecture, API contract and this repository guide.
- `docs/features/`: detailed implementation notes, including dated evidence limitations.
- `scripts/`: reusable build and repository checks.
- `third-party/`: retained notices, provenance and license-required source archives.
- `tools/7zip/runtime/`: only `7z.exe`, `7z.dll` and `License.txt` are upload candidates. Downloaded setup programs and unused language/UI files remain local.

## Local-only structure

- `docs/local-history/`: acceptance ledgers, old status/handoff files and `planning/` originals.
- `scripts/local-history/`: historical, output-specific install/package commands and prepared-demo launcher.
- `test-output/`, `output/`, `.playwright-cli/`: generated samples, installers, logs and screenshots.
- `target/`, `dist/`, `node_modules/`: build products and installed dependencies.
- `.galroon/`, catalog databases, device/session JSON, environment files and private keys: local state.

The workspace's sibling `design/` and `sandbox_data/` are outside this repository. They are not implicitly authorized for upload. No original game data was moved or deleted.

## Maintaining the GitHub repository

Run `python scripts/check_repository.py`, inspect `git status --short`, then review the exact staged files. Keep `Cargo.lock` and `package-lock.json`. Required vendored source archives are deliberate exceptions to excluding generated archives; retain their manifests and licenses. Put built installers in a later GitHub Release, not in source history.

The publication repository is https://github.com/llpgf/galroon, with `master` as its default branch. The organized source tree replaces the earlier prototype in a normal commit; earlier versions remain in Git history. A project license for this source tree has not been selected; third-party license terms remain separate. The original acceptance records do not become passing evidence merely because the source tree is ready for Git.
