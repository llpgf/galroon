@echo off
title Galroon v0.5.0 — Dev Launcher
echo ============================================
echo   Galroon v0.5.0 Dev Launcher
echo ============================================
echo.

cd /d "%~dp0"

:: Check node_modules
if not exist "node_modules" (
    echo [1/3] Installing npm dependencies...
    call npm install
    if errorlevel 1 (
        echo ERROR: npm install failed
        pause
        exit /b 1
    )
) else (
    echo [1/3] node_modules OK
)

echo [2/3] Starting Vite dev server + Tauri backend...
echo.
echo    Frontend: http://localhost:1420
echo    App window will open automatically
echo.
echo    Press Ctrl+C to stop
echo ============================================
echo.

call npx tauri dev

pause
