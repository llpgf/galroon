@echo off
REM ===============================================
REM Vnite Portable Build Script
REM Phase 25.0: The Green Release
REM ===============================================

echo.
echo ========================================
echo VNITE PORTABLE BUILD SCRIPT
echo ========================================
echo.

REM Check if Python is installed
python --version >nul 2>&1
if %ERRORLEVEL% neq 0 (
    echo [ERROR] Python is not installed or not in PATH
    pause
    exit /b 1
)

REM Check if Node.js is installed
node --version >nul 2>&1
if %ERRORLEVEL% neq 0 (
    echo [ERROR] Node.js is not installed or not in PATH
    pause
    exit /b 1
)

REM ===============================================
REM STEP 1: Install Python dependencies
REM ===============================================
echo.
echo [1/5] Installing Python dependencies...
cd /d "%~dp0backend"
pip install pyinstaller -q
if %ERRORLEVEL% neq 0 (
    echo [ERROR] Failed to install PyInstaller
    pause
    exit /b 1
)
echo PyInstaller installed successfully

REM ===============================================
REM STEP 2: Freeze Backend
REM ===============================================
echo.
echo [2/5] Freezing backend with PyInstaller...
pyinstaller build_backend.spec --noconfirm --clean
if %ERRORLEVEL% neq 0 (
    echo [ERROR] Failed to freeze backend
    pause
    exit /b 1
)
echo Backend frozen successfully

REM ===============================================
REM STEP 3: Build Frontend
REM ===============================================
echo.
echo [3/5] Building frontend...
cd /d "%~dp0frontend"
call npm run build
if %ERRORLEVEL% neq 0 (
    echo [ERROR] Failed to build frontend
    pause
    exit /b 1
)
echo Frontend built successfully

REM ===============================================
REM STEP 4: Install Electron dependencies
REM ===============================================
echo.
echo [4/5] Installing Electron dependencies...
cd /d "%~dp0launcher"
call npm install
if %ERRORLEVEL% neq 0 (
    echo [ERROR] Failed to install Electron dependencies
    pause
    exit /b 1
)
echo Electron dependencies installed successfully

REM ===============================================
REM STEP 5: Build Portable Package
REM ===============================================
echo.
echo [5/5] Building portable package...
call npm run build:portable
if %ERRORLEVEL% neq 0 (
    echo [ERROR] Failed to build portable package
    pause
    exit /b 1
)
echo Portable package built successfully

REM ===============================================
REM BUILD COMPLETE
REM ===============================================
echo.
echo ========================================
echo BUILD COMPLETE!
echo ========================================
echo.
echo Output files:
echo   - launcher/release/Vnite-1.0.0-x64.zip (ZIP archive)
echo   - launcher/release/Vnite-Portable-1.0.0-x64.exe (Portable EXE)
echo.
echo Distribution:
echo   1. Extract ZIP to any folder
echo   2. Run Vnite.exe
echo   3. No installation required!
echo.
pause
