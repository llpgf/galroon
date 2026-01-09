# Galroon Galgame Manager

**A modern, portable visual novel (galgame) library management system.**

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)
[![Python 3.10+](https://img.shields.io/badge/python-3.10+-blue.svg)](https://www.python.org/downloads/)
[![React 19](https://img.shields.io/badge/react-19-61DAFB.svg)](https://react.dev/)
[![Electron](https://img.shields.io/badge/Electron-Latest-9FEAF9.svg)](https://www.electronjs.org/)
[![Status: Development](https://img.shields.io/badge/Status-Development%20Only-orange.svg)](https://github.com/llpgf/galroon)

---

## Development build notice

This is an active development build (**v0.3.0**). It is not production ready.

Do not use for:
- production use
- managing important libraries
- daily game management
- expecting stable performance

---

## Current status

- Backend: feature-complete for current roadmap (API and services stable)
- Frontend: in progress (core library UI is functional; remaining pages in progress)
- Launcher: feature-complete
- Testing: partial coverage (backend tests exist, frontend tests limited)

---

## What changed in v0.3.0

- Backend legacy endpoints moved out of `main.py` into a dedicated legacy router.
- Legacy endpoints now share read-only dependency logic.
- Frontend routing structure standardized with `src/pages` for page components.
- API client initialization fixed to wait for session token and dynamic port.
- UI tokens consolidated into a single global stylesheet.
- Manual chunk splitting and lazy loading for heavy dashboards.
- New onboarding and architecture docs for faster handoff.

---

## Features (planned and in progress)

- Automatic library scanning (manual, scheduled, realtime)
- Rich metadata (VNDB, others in progress)
- Portable mode (launcher supported)
- Smart search and filtering
- Analytics and knowledge graphs (in progress)
- Safe trash management

---

## Quick start (dev)

Backend:
```
cd backend
python -m venv .venv
.venv\Scripts\Activate.ps1
pip install -r requirements.txt
copy .env.example .env
python -m uvicorn app.main:app --reload --host 127.0.0.1 --port 8000
```

Frontend:
```
cd frontend
npm install
npm run dev
```

Tests:
```
python -m pytest backend/tests
```

---

## Project structure

```
backend/                 # FastAPI backend
  app/
    api/                 # REST API endpoints
    core/                # core systems (sentinel, transaction)
    metadata/            # metadata providers
    models/              # data models
  tests/
frontend/                # React frontend
  src/
    api/                 # API client
    components/          # reusable UI components
    pages/               # route-level pages
    styles/              # global styles/tokens
launcher/                # Electron desktop wrapper
docs/                    # documentation
tests/                   # integration tests
```

---

## Documentation

- `docs/ONBOARDING.md`
- `docs/ARCHITECTURE.md`
- `docs/API_MAP.md`
- `CHANGELOG.md`

---

## Contributing

1. Fork the repo
2. Create a feature branch
3. Make changes and add tests where possible
4. Open a pull request

Code style:
- Python: follow PEP 8
- TypeScript: follow ESLint rules

---

## License

GPL v3. See `LICENSE`.
