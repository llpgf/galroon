# Galroon

Galroon is a desktop galgame library manager built with Tauri 2, Rust, React, and SQLite. It scans local folders, classifies bundled assets, enriches works from VNDB, Bangumi, and DLsite, and gives you a poster-first library with review, merge, collection, and metadata provenance tools.

## Current baseline

- Poster-first library view with canonical work grouping
- Background scan, enrichment, backup, and update-check jobs
- Bangumi OAuth support for R18/private metadata access
- Multi-source enrichment and field-level source preference
- Characters, creators, collections, workshop, and review workflows
- AI gateway support through LiteLLM or any OpenAI-compatible endpoint
- Scheduled workspace backups and official Tauri updater integration

## Stack

- Frontend: React 19, TypeScript, Vite
- Desktop shell: Tauri 2
- Backend: Rust, Tokio, SQLx, SQLite
- Metadata sources: VNDB, Bangumi, DLsite
- AI gateway: LiteLLM recommended, OpenAI-compatible supported

## Repository layout

```text
public/        Static assets
src/           React frontend
src-tauri/     Tauri app, Rust backend, migrations, updater config
```

## Local development

Requirements:

- Node.js 20+
- Rust stable
- Windows is the current primary dev target

Install and run:

```bash
npm install
npm run tauri dev
```

Build:

```bash
npm run build
cd src-tauri
cargo check
cargo test
```

## AI gateway

Galroon stores AI settings in the workspace, not in the repo. The recommended setup is LiteLLM so one OpenAI-compatible endpoint can front OpenAI, Anthropic, Gemini, OpenRouter, and local models.

Supported presets in Settings:

- LiteLLM
- Generic OpenAI-compatible
- OpenAI
- OpenRouter
- Ollama

Translated text is cached in the workspace database so repeated translation does not keep spending tokens.

## Bangumi auth

Bangumi support includes browser-based OAuth for authenticated and R18/private-visible entries. Tokens are stored in the workspace config and are never meant to be committed.

## Auto update

Galroon uses the official Tauri updater.

To publish signed updater packages from GitHub Actions, set these repository secrets:

- `TAURI_SIGNING_PRIVATE_KEY`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` if your key is password protected

The updater public key is already embedded in the app config. Release artifacts and updater manifests are expected to be uploaded through GitHub Releases.

## Release workflow

This repo includes an official Tauri GitHub Actions release workflow. Tagging a release such as `v0.5.0` should build and upload signed updater artifacts.

## Notes

- Workspace data, thumbnails, logs, databases, sandbox content, and signing keys are intentionally ignored.
- This repo tracks the current Tauri-based codebase, not the older Python/Electron generations.

## License

GPL-3.0. See `LICENSE`.
