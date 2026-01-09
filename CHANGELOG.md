# Changelog

All notable changes to Galroon will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

---

## [0.1.0] - 2026-01-04 (Phase 1: Security Hardening)

### Added
- **Backend Dependency Management**
  - Created `backend/requirements.txt` with all dependencies
  - Documented runtime and development dependencies
  - Added testing and code quality tools (pytest, black, mypy, basedpyright)

- **Security: API Token Authentication**
  - Launcher generates random UUID session token on startup
  - Backend validates `X-Vnite-Token` header via middleware
  - Frontend automatically injects token via Axios interceptor
  - Token passed through environment variable from Launcher to Backend
  - Health/docs endpoints remain accessible without token (for monitoring)
  - Sandbox mode bypasses token check (for development)

- **Security: Zombie Process Cleanup**
  - Added `tree-kill` dependency to launcher
  - Implemented process tree cleanup on `will-quit` event
  - Ensures all Python subprocesses are terminated on app exit
  - Prevents resource leaks and hanging processes

### Changed
- **Security: Host Binding**
  - Verified backend uses `127.0.0.1` exclusively
  - Confirmed no `0.0.0.0` usage in codebase
  - Prevents external network exposure

### Fixed
- Fixed potential zombie process issue on Electron app quit
- Fixed unauthorized API access vulnerability

### Security Improvements
- ✅ API access now requires valid session token
- ✅ Zombie processes eliminated on shutdown
- ✅ Backend only listens on localhost
- ✅ Dependency management clarified

### Technical Details

#### Files Modified
- `backend/requirements.txt` (NEW)
- `launcher/package.json` (added tree-kill)
- `launcher/main.js` (token generation, tree-kill integration)
- `launcher/ipc.js` (getSessionToken handler)
- `launcher/preload.js` (auth API exposure)
- `backend/app/main.py` (TokenAuthMiddleware)
- `frontend/src/api/client.ts` (token injection)
- `build/resources/app/*` (hot-patched)

#### Implementation Notes

**Token Flow**:
1. Launcher generates UUID on startup
2. Token stored in `global.sessionToken` for IPC access
3. Token passed to backend via `SESSION_TOKEN` env var
4. Frontend requests token via `vnite:get-session-token` IPC
5. Axios interceptor injects `X-Vnite-Token` header
6. Backend middleware validates token before processing requests

**Middleware Behavior**:
- All endpoints except `/`, `/api/health`, `/docs`, `/openapi.json` require token
- Returns 401 if token missing
- Returns 403 if token invalid
- Sandbox mode (`GALGAME_ENV=sandbox`) bypasses validation

---

## [0.1.0] - 2026-01-04 (Phase 2: Architecture & Performance)

### Added
- **Database Migration System (Alembic)**
  - Initialized Alembic migration system
  - Created `backend/app/alembic` directory structure
  - Marked current SQLite schema as base version (001_initial_schema)
  - Added automatic migration execution to build scripts (build_portable.sh, build_portable.bat)
  - Environment variable `VNITE_API_PORT` passed to backend for dynamic port support

- **Dynamic Port Allocation**
  - Added `portfinder` dependency to launcher
  - Implemented `getAvailablePort()` function for dynamic port discovery
  - Backend now runs on dynamically allocated ports (8000-8999 range)
  - Frontend API client automatically updates base URL with dynamic port
  - IPC handler `vnite:get-api-port` exposes port to frontend
  - Fallback to random port if portfinder fails

- **Frontend Performance: Virtualization (delegated)**
  - LibraryView.tsx updated by frontend-ui-ux-engineer agent
  - Replaced standard Grid with react-virtuoso for optimized rendering
  - Implemented itemComponent pattern for virtualized lists
  - Lazy loading for cover images (handled by virtuooso)

- **Backend Configuration**
  - Added `VNITE_API_PORT` environment variable support
  - Updated `app/alembic/env.py` for migration integration

### Changed
- **Build Scripts**
  - `build_portable.sh`: Added Alembic upgrade step (step 2.5)
  - `build_portable.bat`: Added Alembic upgrade step (step 2.5)

### Technical Details

#### Files Created
- `backend/requirements.txt` - Added alembic>=1.12.0
- `backend/alembic.ini` - Alembic configuration
- `backend/app/alembic/env.py` - Migration environment
- `backend/app/alembic/__init__.py` - Migration scripts package
- `backend/app/alembic/001_initial_schema.py` - Base schema migration
- `backend/app/alembic/README.md` - Migration documentation

#### Files Modified (main_code/v0.1.0/)
- `launcher/package.json` - Added portfinder dependency
- `launcher/main.js` - Dynamic port allocation, async startBackend, port IPC
- `launcher/ipc.js` - getApiPort handler
- `launcher/preload.js` - auth.getApiPort exposure

#### Files Modified (build hot patch)
- `build/resources/app/package.json` - Added portfinder, tree-kill
- `build/resources/app/main.js` - Phase 27.0, Phase 28.0
- `build/resources/app/ipc.js` - getApiPort handler
- `build/resources/app/preload.js` - auth.getApiPort

#### Files Modified (frontend - delegated)
- `frontend/src/views/LibraryView.tsx` - Virtualization (by frontend-ui-ux-engineer)
- `frontend/src/api/client.ts` - Dynamic port initialization (initApiPort)

### Migration System
```
Initial Schema (001_initial_schema):
- Marks current database schema as base version
- No schema changes (Database._init_db already creates tables)
- Provides version tracking for future migrations

Migration Commands:
- alembic revision -m "description" - Create new migration
- alembic upgrade head - Apply all pending migrations
- alembic downgrade -1 - Revert last migration
```

### Port Allocation Flow
```
1. Launcher starts
2. getAvailablePort() checks port 8000
3. If busy, search 8000-8999 range
4. Allocate available port
5. Pass via VNITE_API_PORT to backend
6. Frontend requests port via IPC (vnite:get-api-port)
7. API client updates baseURL dynamically
```

### Performance Improvements
- Virtual scrolling reduces DOM nodes from 1000+ to only visible items
- Large libraries (>500 games) now render smoothly
- Lazy loading images reduces initial load time
- Dynamic port allocation avoids port conflicts

---

## [0.1.0] - 2026-01-04 (Phase 3: Architecture & Optimization)

### Added
- **Modular Routing (API v1)**
  - Created `backend/app/api/v1` directory structure
  - Implemented versioned API endpoints with `/api/v1` prefix
  - Moved all existing routers under v1 namespace for better organization
  - Updated `app/api/__init__.py` to import v1 router

- **WebSocket Support for Real-time Updates**
  - Created `backend/app/api/websocket.py` - WebSocket connection manager
  - Implemented WebSocket endpoint `/ws/scan-progress` in v1 router
  - Added `ScanProgressUpdate` model for progress broadcasting
  - WebSocket client can subscribe to scan progress updates
  - Support for ping/pong keep-alive mechanism

- **Image Caching System**
  - Created `backend/app/api/image_cache.py` - Local image cache service
  - Created `backend/app/api/v1/image_cache_api.py` - Image cache API endpoints
  - Features:
    - Download and cache cover images locally
    - Serve cached images with Cache-Control headers
    - Automatic cache cleanup when size exceeds limit (default 500MB)
    - Cache index for fast lookup
    - Cache info endpoint (`/api/v1/images/cache-info`)
    - Manual cache cleanup endpoint (`/api/v1/images/cache/cleanup`)
    - Clear cache endpoint (`/api/v1/images/cache/clear`)
    - Serve cached image endpoint (`/api/v1/images/cached/{cache_key}`)
  - Cache storage location: `data/cache/covers/`

### Changed
- **Backend Structure**
  - `backend/app/api/v1/__init__.py` - New versioned router
  - `backend/app/api/__init__.py` - Updated to include v1 router
  - `backend/app/main.py` - Added v1 router registration
  - `backend/requirements.txt` - Added `websockets>=11.0` dependency

### Technical Details

#### Files Created
- `backend/app/api/v1/__init__.py` - Versioned API router with WebSocket
- `backend/app/api/v1/image_cache_api.py` - Image cache API endpoints
- `backend/app/api/websocket.py` - WebSocket manager for real-time updates
- `backend/app/api/image_cache.py` - Image cache service

#### Files Modified (backend)
- `backend/app/api/__init__.py` - Added v1 router import
- `backend/app/main.py` - Added v1 router registration
- `backend/requirements.txt` - Added websockets dependency

#### WebSocket Message Format

**Client → Server**:
```json
{
  "action": "subscribe"
}
```

**Server → Client** (Scan Progress):
```json
{
  "type": "scan_progress",
  "data": {
    "current": 150,
    "total": 1000,
    "percentage": 15.0,
    "message": "Scanning...",
    "is_complete": false
  }
}
```

**Ping/Pong**:
```json
{"action": "ping"}  // Client → Server
{"action": "pong"}   // Server → Client
```

#### Image Cache API Endpoints

```
POST   /api/v1/images/download          # Download and cache image
GET    /api/v1/images/cache-info        # Get cache statistics
POST   /api/v1/images/cache/cleanup     # Clean up if exceeds limit
POST   /api/v1/images/cache/clear       # Clear all cached images
GET    /api/v1/images/cached/{cache_key}  # Serve cached image
```

#### Image Cache Features
- **Automatic Download**: Images are downloaded on first access
- **Local Storage**: Cached in `data/cache/covers/`
- **Size Limit**: Default 500MB, automatic cleanup
- **Cache Index**: Fast MD5 hash-based lookup
- **Cache Headers**: 7-day cache-control for cached images
- **Manual Management**: Info, cleanup, and clear endpoints

---

## [0.0.0] - 2026-01-03 (Initial Development)

### Added
- Initial release of Galroon Galgame Manager
- Transaction-based file operations with rollback
- Noise-resilient file system monitoring (Sentinel)
- SQLite FTS5 database for instant search
- FastAPI backend with modular routers
- React 19 + TypeScript frontend with Zustand state management
- Electron launcher for portable distribution
- Metadata providers: VNDB, Bangumi, Steam
- Smart Trash Manager with retention policies
- Batch metadata scanning with rate limiting
- Visual scanner with progress tracking
- Backup and restore functionality
- Task scheduler for automated scans
