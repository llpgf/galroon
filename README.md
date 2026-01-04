# 🎮 Galroon Galgame Manager

<div align="center">

**A modern, portable visual novel game library management system**

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)
[![Python 3.11+](https://img.shields.io/badge/python-3.11+-blue.svg)](https://www.python.org/downloads/)
[![React 19](https://img.shields.io/badge/react-19-61DAFB.svg)](https://react.dev/)
[![Electron](https://img.shields.io/badge/Electron-Latest-9FEAF9.svg)](https://www.electronjs.org/)
[![Status: Development](https://img.shields.io/badge/Status-Development%20Only-orange.svg)](https://github.com/llpgf/galroon)

[Features](#-features) • [Installation](#-installation) • [Development](#-development) • [Contributing](#contributing)

</div>

---

## ⚠️ **DEVELOPMENT BUILD - NOT PRODUCTION READY**

### 🚨 Important Notice

**This is an active development build (v0.1.0). Many features are incomplete or not working.**

**DO NOT use for:**
- ❌ Production use
- ❌ Managing important game libraries
- ❌ Daily game management
- ❌ Expecting stable performance

**Current Status:**
- ✅ **Backend**: 100% complete (API fully functional)
- ⚠️ **Frontend**: 50% complete (many UI components missing)
- ✅ **Launcher**: 100% complete
- ⚠️ **Testing**: 40% coverage
- 📊 **Overall**: 80% complete

### What Works (Phase 1-3)

**Phase 1: Security Hardening** ✅
- API Token authentication
- Zombie process cleanup
- Secure host binding (localhost only)

**Phase 2: Architecture & Performance** ✅
- Alembic database migration system
- Dynamic port allocation
- Frontend virtualization (react-virtuoso)

**Phase 3: Architecture & Optimization** ✅
- Versioned API routing (`/api/v1`)
- WebSocket support for real-time updates
- Image caching system
- LSP server configuration

### What's Missing (Not Implemented)

**Frontend UI Components** 🚧
- Library view (partial)
- Game detail pages
- Metadata editing UI
- Scanner visualization
- Settings UI
- Analytics dashboard
- Tag management interface

**Integration Features** 🚧
- WebSocket client implementation (backend ready, frontend not)
- Image cache integration (backend ready, frontend not)
- Dynamic port handling in UI (backend ready, frontend not)

**Testing** 🚧
- Frontend unit tests
- Integration tests
- E2E tests

---

## 📖 Overview

**Galroon** is a comprehensive visual novel (galgame) library manager designed for enthusiasts who want to organize, manage, and enhance their visual novel collections.

### Planned Capabilities (When Complete)

- **📁 Automatic Library Scanning** - Monitors folders and auto-detects new games
- **🎨 Rich Metadata** - Fetches metadata from VNDB, Bangumi, Steam
- **📦 Portable Mode** - Runs from any folder, zero system footprint
- **🔍 Smart Search** - Advanced search with filters and tags
- **📊 Analytics** - Statistics and knowledge graphs
- **🗑️ Safe Trash** - Delete with undo capability
- **⚡ Fast & Lightweight** - Built for performance

---

## 🏗️ Architecture

### Technology Stack

**Backend:**
- Python 3.11+
- FastAPI (Web Framework)
- SQLite (Database)
- Uvicorn (ASGI Server)

**Frontend:**
- React 19
- TypeScript
- Tailwind CSS
- Zustand (State Management)

**Launcher:**
- Electron (Desktop Wrapper)
- Node.js

### Project Structure

```
Galroon-galgame-manager/
 ├── backend/              # Python FastAPI backend
 │   ├── app/             # Application code
 │   │   ├── api/         # REST API endpoints
 │   │   ├── core/        # Core systems (sentinel, transaction)
 │   │   ├── metadata/    # Metadata providers
 │   │   └── models/      # Data models
 │   ├── tests/           # Backend tests
 │   └── requirements.txt # Python dependencies
 ├── frontend/            # React TypeScript frontend
 │   ├── src/            # Source code
 │   │   ├── api/        # API client
 │   │   ├── components/ # React components
 │   │   ├── hooks/      # Custom hooks
 │   │   └── views/      # Page views
 │   ├── package.json    # Node dependencies
 │   └── vite.config.ts  # Vite config
 ├── launcher/           # Electron desktop app
 │   ├── main.js        # Electron main process
 │   ├── preload.js     # Preload script
 │   └── package.json   # Node dependencies
 ├── tests/             # Integration tests
 ├── docs/              # Documentation
 └── scripts/           # Build scripts
```

---

## 🚀 Installation

### Prerequisites

- **Python** 3.11 or higher
- **Node.js** 18 or higher
- **Git**

### Option 1: Portable Release (Recommended for Testing)

1. Download the latest development build from [Releases](https://github.com/llpgf/galroon/releases)
2. Extract to any folder
3. Run `Galroon.exe` (Windows) or `Galroon` (Linux/Mac)
4. No installation required!

**⚠️ Warning:** This is a development build. Expect bugs and missing features.

### Option 2: Build from Source

#### Clone Repository

```bash
git clone https://github.com/llpgf/galroon.git
cd galroon
```

#### Install Backend Dependencies

```bash
cd backend
pip install -r requirements.txt
```

#### Install Frontend Dependencies

```bash
cd ../frontend
npm install
npm run build
```

#### Build Launcher

```bash
cd ../launcher
npm install
npm run build:portable
```

---

## 🎮 Usage

### Quick Start

1. **Launch the application**
   ```bash
   # From portable release
   ./Galroon.exe

   # From source
   cd launcher && npm start
   ```

2. **Add your library**
   - Go to Settings → Library Roots
   - Add your games folder
   - Click "Scan Library"

3. **Manage your games**
   - View game details
   - Fetch metadata from VNDB
   - Organize with tags
   - Search and filter

### Configuration

Configuration files are stored in:
- **Portable mode:** `<app>/data/config/`
- **Production mode:** `~/.galgame-manager/config/`

Example configuration:

```yaml
library_roots:
  - "D:/Galgames"
  - "E:/Visual Novels"

scan_mode: "realtime"  # realtime | scheduled | manual

metadata:
  primary_provider: "vndb"
  fallback_providers:
    - "bangumi"
    - "steam"

trash:
  max_size_gb: 10
  retention_days: 30
```

---

## 🔧 Development

### Setup Development Environment

```bash
# Backend (with hot reload)
cd backend
uvicorn app.main:app --reload --port 8000

# Frontend (with dev server)
cd frontend
npm run dev

# Launcher (development mode)
cd launcher
npm run dev
```

### Running Tests

```bash
# Backend tests
cd backend
pytest

# Frontend tests
cd frontend
npm test

# Integration tests
cd tests
python test_integration.py
```

### Building for Production

```bash
# Full build (Windows)
./build_portable.bat

# Full build (Linux/Mac)
./build_portable.sh
```

Output: `launcher/release/galroon-Portable-vX.X.X-x64.zip`

---

## 📚 Documentation

- [CHANGELOG.md](CHANGELOG.md) - Version history and release notes
- [VERSION_HISTORY.md](VERSION_HISTORY.md) - Version planning and roadmap
- [AI Review Report](AI_review_report/) - Code review and improvement notes
- [Production Readiness](AI_review_report/Production_Readiness_Todo_Report.md) - Development progress

---

## 🤝 Contributing

We welcome contributions! Please see below for details.

### Development Workflow

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Write/update tests
5. Ensure all tests pass
6. Commit your changes (`git commit -m 'Add amazing feature'`)
7. Push to your branch (`git push origin feature/amazing-feature`)
8. Open a Pull Request

### Code Style

- **Python:** Follow PEP 8, use Black formatter
- **TypeScript:** Follow ESLint rules
- **Commits:** Use Conventional Commits format

### Development Priorities

Current focus areas (v0.1.0):
1. ⏳ Frontend UI components completion
2. ⏳ WebSocket client integration
3. ⏳ Image cache UI integration
4. ⏳ E2E test coverage
5. ⏳ Documentation completion

---

## 📜 License

This project is licensed under **GNU General Public License v3.0**.

See [LICENSE](LICENSE) for full text.

**What this means:**
- ✅ Free to use, study, modify, and distribute
- ⚠️ Modifications must also be GPL v3
- ⚠️ Source code must be provided when distributing
- ✅ Commercial use allowed

For more information, visit https://www.gnu.org/licenses/gpl-3.0.html

---

## 🙏 Acknowledgments

- **[VNDB](https://vndb.org/)** - Visual Novel Database
- **[Bangumi](https://bgm.tv/)** - Chinese ACG database
- **[FastAPI](https://fastapi.tiangolo.com/)** - Modern Python web framework
- **[React](https://react.dev/)** - UI library
- **[Electron](https://www.electronjs.org/)** - Desktop framework

---

## 📞 Support

- **Issues:** [GitHub Issues](https://github.com/llpgf/galroon/issues)
- **Discussions:** [GitHub Discussions](https://github.com/llpgf/galroon/discussions)
- **Wiki:** [Project Wiki](https://github.com/llpgf/galroon/wiki)

---

## 🗺️ Roadmap

### Current Release: v0.1.0 (Development Build)

**Completed (Phase 1-3)**:
- ✅ Phase 1: Security Hardening (API Token, Zombie Process Cleanup)
- ✅ Phase 2: Architecture & Performance (Alembic, Dynamic Port, Virtualization)
- ✅ Phase 3: Architecture & Optimization (API v1, WebSocket, Image Cache)

**Backend Status**: 100% Complete
**Frontend Status**: 50% Complete (UI components pending)

### Upcoming: v0.2.0

- ⏳ Complete frontend UI components
- ⏳ WebSocket client integration
- ⏳ Image cache UI integration
- ⏳ Increase test coverage to 60%

### Future: v1.0.0 (Stable Release)

- ⏳ Frontend 100% completion
- ⏳ Test coverage ≥ 80%
- ⏳ Cloud sync
- ⏳ Multi-language support
- ⏳ Plugin system
- ⏳ Mobile app

---

<div align="center">

**Built with ❤️ by GalroonProject**

[⬆ Back to Top](#-galroon-galgame-manager)

</div>
