@echo off
:: 切換到目前檔案所在的目錄 (確保不會因為路徑問題找不到檔案)
cd /d "%~dp0"

echo ==========================================
echo       正在執行 Galroon 代碼快照...
echo ==========================================
echo.

:: 檢查是否有 Python
python --version >nul 2>&1
if %errorlevel% neq 0 (
    echo [錯誤] 找不到 Python，請確認已安裝並加入 PATH 環境變數。
    pause
    exit /b
)

:: 執行快照腳本
python export_code.py

echo.
echo ==========================================
if exist project_snapshot.txt (
    echo [成功] 快照已生成：project_snapshot.txt
) else (
    echo [失敗] 未能生成檔案，請檢查上方錯誤訊息。
)
echo ==========================================
echo.
echo 按任意鍵關閉視窗...
pause >nul