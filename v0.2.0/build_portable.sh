#!/bin/bash
# ===============================================
# Vnite Portable Build Script (Linux/macOS)
# Phase 25.0: The Green Release
# ===============================================

set -e  # Exit on error

echo ""
echo "========================================"
echo "VNITE PORTABLE BUILD SCRIPT"
echo "========================================"
echo ""

# Check if Python is installed
if ! command -v python3 &> /dev/null; then
    echo "[ERROR] Python 3 is not installed"
    exit 1
fi

# Check if Node.js is installed
if ! command -v node &> /dev/null; then
    echo "[ERROR] Node.js is not installed"
    exit 1
fi

# Get script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# ===============================================
# STEP 1: Install Python dependencies
# ===============================================
echo ""
echo "[1/5] Installing Python dependencies..."
cd "$SCRIPT_DIR/backend"
pip3 install pyinstaller -q
echo "PyInstaller installed successfully"

# ===============================================
# STEP 2: Freeze Backend
# ===============================================
echo ""
echo "[2/6] Freezing backend with PyInstaller..."
cd "$SCRIPT_DIR/backend"
pyinstaller build_backend.spec --noconfirm --clean
echo "Backend frozen successfully"

# ===============================================
# STEP 2.5: Run Database Migrations
# ===============================================
echo ""
echo "[2.5/6] Running database migrations..."
cd "$SCRIPT_DIR/backend"
python3 -c "from app.alembic import command; command.upgrade('head')"
echo "Database migrations completed"

# ===============================================
# STEP 3: Build Frontend
# ===============================================
echo ""
echo "[3/5] Building frontend..."
cd "$SCRIPT_DIR/frontend"
npm run build
echo "Frontend built successfully"

# ===============================================
# STEP 4: Install Electron dependencies
# ===============================================
echo ""
echo "[4/5] Installing Electron dependencies..."
cd "$SCRIPT_DIR/launcher"
npm install
echo "Electron dependencies installed successfully"

# ===============================================
# STEP 5: Build Portable Package
# ===============================================
echo ""
echo "[5/5] Building portable package..."
npm run build:portable
echo "Portable package built successfully"

# ===============================================
# BUILD COMPLETE
# ===============================================
echo ""
echo "========================================"
echo "BUILD COMPLETE!"
echo "========================================"
echo ""
echo "Output files:"
echo "  - launcher/release/Vnite-1.0.0-x64.zip (ZIP archive)"
echo "  - launcher/release/Vnite-Portable-1.0.0-x64.exe (Portable EXE)"
echo ""
echo "Distribution:"
echo "  1. Extract ZIP to any folder"
echo "  2. Run Vnite"
echo "  3. No installation required!"
echo ""
