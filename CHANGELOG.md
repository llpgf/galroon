# Changelog

All notable changes to Galroon will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.1.0] - 2026-01-04

### Added
- Initial development release of Galroon
- Core backend functionality (FastAPI, SQLite database)
- Basic frontend application (React 19, TypeScript, Tailwind CSS)
- Electron launcher for cross-platform deployment
- Automatic game scanning and monitoring
- Metadata management from VNDB, Bangumi, and Steam
- Portable mode support (zero system dependencies)
- Safe deletion system with trash can and undo support
- Advanced search with tags, filters, and sorting
- Analytics dashboard with statistics and knowledge graph

### Changed
- **BREAKING:** Restructured project directory from `Claude Code\` to `galroon\`
  - `main_code/` - Version controlled clean source code
  - `debugs/` - Development workspace
  - `build/` - Production builds for manual testing
  - `review/` - Clean source code for GitHub review
  - `record/` - Work reports and memory
  - `AI_review_report/` - AI review reports
  - `reference/` - Reference documentation
- Updated all script paths to new directory structure
- Implemented semantic versioning (SemVer) starting with v0.1.0

### Fixed
- Backend crash: `AttributeError: 'Config' object has no attribute 'library_root'`
- Frontend white screen issue (path configuration)
- Portable mode compatibility issues
- PyInstaller compilation and deployment

### Technical
- Backend completion: 100%
- Frontend completion: 45%
- Launcher completion: 90%
- Documentation: 60%
- Testing: 20%
- Overall: 73%

### Notes
- This is a development release (v0.x.x)
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

[0.1.0]: https://github.com/llpgf/galroon/releases/tag/v0.1.0

---

**Note:** For detailed version history and release criteria, see [VERSION_HISTORY.md](VERSION_HISTORY.md)
