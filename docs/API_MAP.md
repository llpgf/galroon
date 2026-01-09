# API Map

This project keeps both legacy endpoints and v1 endpoints.

## Legacy endpoints (/api/*)
Defined in backend/app/api/legacy.py
- /api/scan/*
- /api/library/*
- /api/trash/*
- /api/metadata/* (batch, field lock/status, play status, apply, game metadata)

## v1 endpoints (/api/v1/*)
Defined in backend/app/api/v1/* and mounted in backend/app/api/__init__.py
- /api/v1/organizer/*
- /api/v1/canonical/*
- /api/v1/tags/*
- /api/v1/auth/*
- /api/v1/sync/*
- /api/v1/vndb/*
- /api/v1/graph/*

## Other routers (/api/*)
Defined in backend/app/api/*.py
- /api/organizer/*
- /api/curator/*
- /api/analytics/*
- /api/search/*
- /api/connectors/*
- /api/utilities/*
- /api/history/*
- /api/settings/*
- /api/system/*
- /api/scheduler/*
- /api/backup/*
- /api/update/*
- /api/games/*
- /api/scanner/*

## Frontend mapping
- The API client lives in frontend/src/api/client.ts
- Utility helpers are in frontend/src/api/utilityApi.ts

When adding new endpoints, update both the backend router and the client mapping.
