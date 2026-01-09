# Onboarding

This project has a FastAPI backend and a Vite React frontend.

## Requirements
- Python 3.10
- Node.js 18+ and npm
- SQLite (bundled with Python)

## Quick start
1) Backend setup
- cd backend
- python -m venv .venv
- .venv\Scripts\Activate.ps1
- pip install -r requirements.txt
- Copy backend/.env.example to backend/.env and adjust values
- python -m uvicorn app.main:app --reload --host 127.0.0.1 --port 8000

2) Frontend setup
- cd frontend
- npm install
- npm run dev

## Tests
- Backend: python -m pytest backend/tests
- Frontend: npm run build (in frontend)

## Common issues
- Missing DB view error: run `alembic upgrade head` from backend/
- 401/403 errors: set SESSION_TOKEN in backend/.env and send X-Vnite-Token header
- Port mismatch: frontend reads dynamic port from backend if available, default 8000

## Where to look
- Backend entry: backend/app/main.py
- Frontend entry: frontend/src/main.tsx and frontend/src/App.tsx
- API client: frontend/src/api/client.ts
