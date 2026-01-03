@echo off
setlocal enabledelayedexpansion

echo ========================================
echo VNITE MANUAL BUILD SCRIPT
echo ========================================
echo.

cd /d "%~dp0launcher"

REM ===============================================
REM STEP 1: Create output directory
REM ===============================================
echo [1/4] Creating output directory...
if not exist "release\win-unpacked" mkdir "release\win-unpacked"

REM ===============================================
REM STEP 2: Copy Electron runtime
REM ===============================================
echo [2/4] Copying Electron runtime...
if not exist "node_modules\electron\dist\" (
    echo ERROR: Electron not found. Run: npm install
    pause
    exit /b 1
)

xcopy /E /I /Y "node_modules\electron\dist\*" "release\win-unpacked\"

REM ===============================================
REM STEP 3: Copy app files
REM ===============================================
echo [3/4] Copying application files...
copy /Y "main.js" "release\win-unpacked\resources\app\"
copy /Y "preload.js" "release\win-unpacked\resources\app\"
copy /Y "package.json" "release\win-unpacked\resources\app\"

REM ===============================================
REM STEP 4: Copy backend and frontend
REM ===============================================
echo [4/4] Copying backend and frontend...
if not exist "..\backend\dist\backend" (
    echo ERROR: Backend not frozen. Run PyInstaller first
    pause
    exit /b 1
)

if not exist "..\frontend\dist" (
    echo ERROR: Frontend not built. Run: npm run build
    pause
    exit /b 1
)

xcopy /E /I /Y "..\backend\dist\backend" "release\win-unpacked\resources\backend\"
xcopy /E /I /Y "..\frontend\dist" "release\win-unpacked\resources\frontend\"

echo.
echo ========================================
echo BUILD COMPLETE!
echo ========================================
echo.
echo Output: release\win-unpacked\
echo   - Vnite.exe (ready to run)
echo.
echo To create ZIP:
echo   1. Navigate to release\win-unpacked
echo   2. Select all files
echo   3. Send to Compressed (zipped) folder
echo.
pause
