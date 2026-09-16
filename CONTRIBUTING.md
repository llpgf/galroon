# Development notes

Run commands from the repository root on Windows. Setup and build commands are in [README](README.md). Preserve the lockfiles and review any changes to third-party notices when changing dependencies.

## Focused checks

```powershell
npm run build
npm test -- --run
cargo test -p galroon-core --lib <test_name_filter>
```

Choose checks for the affected behavior. The Core examples under `crates/core/examples/` include fixture generation, API diagnostics and filesystem workflows; inspect their arguments before running them. Generated files belong under ignored `test-output/`. Never use a personal game collection as a disposable test fixture.

Frontend code lives under `src/`, Core logic under `crates/core/src/`, and native Windows integration under `apps/desktop/src-tauri/src/`. Keep related tests near their implementation. [Architecture](docs/development/ARCHITECTURE.md) describes the boundaries.

## Preparing changes

Run `python scripts/check_repository.py` to check the upload candidate, local document links and Tauri resource paths. Review `git status --short` and the actual staged diff before committing. The checker uses tracked and non-ignored paths; it does not stage files, publish anything or guarantee that every possible secret can be detected.

No remote, release publication or license selection is performed by the local preparation scripts. Do not commit databases, game files, sessions, `.env` values, installers or local acceptance archives.
