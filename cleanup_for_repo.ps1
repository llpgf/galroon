# V0.3.0 Repo Cleanup Script
# Run this in PowerShell from C:\Users\Ben\Desktop\galroon\main_code\v0.3.0

Write-Host "=== V0.3.0 Repo Cleanup ===" -ForegroundColor Cyan

# 1. Delete runtime data (databases, logs)
Write-Host "Removing runtime data..." -ForegroundColor Yellow
Remove-Item -Recurse -Force "backend\data" -ErrorAction SilentlyContinue
Remove-Item -Recurse -Force "backend\sandbox_data" -ErrorAction SilentlyContinue
Remove-Item -Force "backend\backend.log" -ErrorAction SilentlyContinue

# 2. Delete Python cache
Write-Host "Removing Python cache..." -ForegroundColor Yellow
Get-ChildItem -Recurse -Directory -Filter "__pycache__" | Remove-Item -Recurse -Force
Remove-Item -Recurse -Force ".pytest_cache" -ErrorAction SilentlyContinue

# 3. Delete node_modules and dist (if you want clean repo)
Write-Host "Removing node_modules and dist..." -ForegroundColor Yellow
Remove-Item -Recurse -Force "frontend\node_modules" -ErrorAction SilentlyContinue
Remove-Item -Recurse -Force "frontend\dist" -ErrorAction SilentlyContinue
Remove-Item -Recurse -Force "launcher\node_modules" -ErrorAction SilentlyContinue

# 4. Delete project_snapshot.txt (large generated file)
Write-Host "Removing project snapshot..." -ForegroundColor Yellow
Remove-Item -Force "project_snapshot.txt" -ErrorAction SilentlyContinue

# 5. Verify .gitignore coverage
Write-Host ""
Write-Host "=== Checking git status ===" -ForegroundColor Cyan
git status --porcelain

Write-Host ""
Write-Host "=== Cleanup Complete ===" -ForegroundColor Green
Write-Host "Now run: git add -A && git status" -ForegroundColor White
