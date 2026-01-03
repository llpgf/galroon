# 🎮 Galroon Galgame Manager

<div align="center">

**A modern, portable visual novel game library management system**

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)
[![Python 3.11+](https://img.shields.io/badge/python-3.11+-blue.svg)](https://www.python.org/downloads/)
[![React 19](https://img.shields.io/badge/react-19-61DAFB.svg)](https://react.dev/)
[![Electron](https://img.shields.io/badge/Electron-Latest-9FEAF9.svg)](https://www.electronjs.org/)

[Features](#-features) • [Installation](#-installation) • [Usage](#-usage) • [Development](#-development) • [Contributing](#-contributing)

</div>

---

## 📖 Overview

**Vnite** is a comprehensive visual novel (galgame) library manager designed for enthusiasts who want to organize, manage, and enhance their visual novel collections.

### Key Capabilities

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
vnite-galgame-manager/
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

### Option 1: Portable Release (Recommended)

1. Download the latest release from [Releases](https://github.com/your-username/vnite-galgame-manager/releases)
2. Extract to any folder
3. Run `Vnite.exe` (Windows) or `Vnite` (Linux/Mac)
4. No installation required!

### Option 2: Build from Source

#### Clone Repository

```bash
git clone https://github.com/your-username/vnite-galgame-manager.git
cd vnite-galgame-manager
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
   ./Vnite.exe

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

Output: `launcher/release/Vnite-Portable-vX.X.X-x64.zip`

---

## 📚 Documentation

- [Architecture Guide](docs/ARCHITECTURE.md)
- [API Documentation](docs/API.md)
- [Contributing Guide](docs/CONTRIBUTING.md)
- [Metadata Sources](docs/METADATA_SOURCES.md)

---

## 🤝 Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for details.

### Development Workflow

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Write/update tests
5. Ensure all tests pass
6. Commit your changes (`git commit -m 'Add amazing feature'`)
7. Push to the branch (`git push origin feature/amazing-feature`)
8. Open a Pull Request

### Code Style

- **Python:** Follow PEP 8, use Black formatter
- **TypeScript:** Follow ESLint rules
- **Commits:** Use Conventional Commits format

---

## 📜 License

This project is licensed under the **GNU General Public License v3.0**.

See [LICENSE](LICENSE) for the full text.

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

- **Issues:** [GitHub Issues](https://github.com/your-username/vnite-galgame-manager/issues)
- **Discussions:** [GitHub Discussions](https://github.com/your-username/vnite-galgame-manager/discussions)
- **Wiki:** [Project Wiki](https://github.com/your-username/vnite-galgame-manager/wiki)

---

## 🗺️ Roadmap

### Current Release: v1.0.0
- ✅ Portable mode
- ✅ Metadata fetching (VNDB, Bangumi, Steam)
- ✅ File system monitoring
- ✅ Safe trash with undo
- ✅ Advanced search

### Upcoming: v1.1.0
- ⏳ Cloud sync
- ⏳ Mobile app
- ⏳ Multi-language support
- ⏳ Plugin system

---

<div align="center">

**Built with ❤️ by the Vnite Project**

[⬆ Back to Top](#-vnite-galgame-manager)

</div>
 
"## AI Review Enabled" 
