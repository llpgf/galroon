# Architecture

## Backend (FastAPI)
- Entry and lifecycle: backend/app/main.py
- Routers: backend/app/api/*
- v1 routers: backend/app/api/v1/* (mounted under /api/v1)
- Legacy routers: backend/app/api/legacy.py (pre-v1 endpoints kept for compatibility)
- Core services: backend/app/core/*
- Metadata system: backend/app/metadata/*
- Background services: backend/app/services/*
- Models: backend/app/models/*

### App state
The app stores shared runtime objects in app.state during lifespan startup, including:
- library_root, config_dir
- journal_manager
- sentinel (scanner)
- batch_manager
- database

## Frontend (Vite + React)
- Entry: frontend/src/main.tsx
- Routes and layout: frontend/src/App.tsx
- Pages: frontend/src/pages/*
- Reusable UI: frontend/src/components/*
- API client: frontend/src/api/*
- Global styles and tokens: frontend/src/styles/globals.css

### UI consistency
Global CSS variables live in globals.css. Components should use CSS variables instead of hard-coded colors.

## Data locations
- Database: backend/data/library.db
- Backups: backend/data/backups
- Config: backend/.config or path from env overrides
