# Changelog

All notable changes to Galroon will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

---

## [0.1.0] - 2026-01-04 (Phase 2: Architecture & Performance)

### Added
- **Database Migration System (Alembic)**
  - Initialized Alembic migration system
  - Created `backend/app/alembic` directory structure
  - Marked current SQLite schema as base version (001_initial_schema)
  - Added automatic migration execution to build scripts (build_portable.sh, build_portable.bat)

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

### Technical
- Backend completion: 100%
- Frontend completion: 45%
- Launcher completion: 90%
- Documentation: 60%
- Testing: 20%
- Overall: 73%

### Notes
- This is a development release (v1.x.x)
- API stability is not guaranteed
- Features may change or be removed in future versions
- Documentation is still incomplete
- Not recommended for production use

---

## [Unreleased]

### Planned
- Complete frontend implementation (target: v0.6.0)
- Increase test coverage (target: ≥80% for v1.0.0)
- Complete documentation
- Performance optimizations
- Additional features and improvements

---

## Version Reference

[1.1.0]: https://github.com/llpgf/galroon/releases/tag/v1.1.0
[1.0.0]: https://github.com/llpgf/galroon/releases/tag/v1.0.0

---

**Note:** For detailed version history and release criteria, see [VERSION_HISTORY.md](VERSION_HISTORY.md)
