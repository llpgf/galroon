# US-4.1 (W layer of docs/acceptance/USER_STORIES.md) on the Windows test laptop:
# silent install -> Start menu entry -> first launch with a fresh GALROON_DATA -> tests/windows (add source, covers,
# 0 false matches, native watcher). Run from a checkout after `npm ci` and `npm run desktop:build`.
# SSH sessions cannot show windows, so the app is launched in the signed-in user's desktop through a one-off
# scheduled task, with WebView2 remote debugging on 127.0.0.1 so Playwright can drive the real window.
param(
  [string]$Installer = '',
  [string]$Source = 'D:\galroon-mcp-sandbox\source',
  [string]$DataRoot = 'D:\galroon-e2e',
  [int]$CdpPort = 9222,
  [string]$InstallDir = (Join-Path $env:LOCALAPPDATA 'Galroon')
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$app = Join-Path $InstallDir 'galroon-desktop.exe'
$expected = (Get-Content -Raw -LiteralPath (Join-Path $repo 'apps\desktop\src-tauri\tauri.conf.json') | ConvertFrom-Json).version
$task = 'GalroonAcceptance'

if (-not $Installer) {
  $Installer = Get-ChildItem -Recurse -LiteralPath (Join-Path $repo 'target\release\bundle') -Filter '*-setup.exe' | Sort-Object LastWriteTime | Select-Object -Last 1 -ExpandProperty FullName
}
if (-not (Test-Path -LiteralPath $Installer)) { throw "Installer not found: $Installer" }
# An installer from elsewhere could be an older build with the same version; test this checkout's own bundle.
$bundle = (Resolve-Path -LiteralPath (Join-Path $repo 'target\release\bundle')).Path
if (-not (Resolve-Path -LiteralPath $Installer).Path.StartsWith($bundle, [StringComparison]::OrdinalIgnoreCase)) { throw "Installer must come from this checkout's build ($bundle): $Installer" }
if (-not (Test-Path -LiteralPath $Source)) { throw "Test source not found: $Source" }

# Only this test's processes run on the laptop; an open app would lock the install.
Get-Process galroon-desktop, galroon-core -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 2

"== 1. Silent install: $Installer -> $InstallDir (expect $expected)"
# /D pins the folder: without it NSIS reuses whatever location an earlier install recorded in the registry.
$p = Start-Process -FilePath $Installer -ArgumentList '/S', "/D=$InstallDir" -Wait -PassThru
if ($p.ExitCode -ne 0) { throw "Installer exit code $($p.ExitCode)" }
$version = (Get-Item -LiteralPath $app).VersionInfo.ProductVersion
"installed: $version at $app"
if ($version -ne $expected) { throw "Installed app is $version, expected ${expected}: the installer did not replace $app" }
$entry = Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*' -ErrorAction SilentlyContinue | Where-Object DisplayName -eq 'Galroon' | Select-Object -First 1
if (-not $entry -or $entry.DisplayVersion -ne $expected -or $entry.InstallLocation.Trim('"') -ne $InstallDir) { throw "Uninstall entry is $($entry.DisplayVersion) at $($entry.InstallLocation), expected $expected at $InstallDir" }
"uninstall entry: $($entry.DisplayVersion) at $($entry.InstallLocation)"
# Same-version builds exist (all of 0.0.2), so also prove the installed binaries are this checkout's build.
# Core and MCP must be byte-identical to target\release. The bundler rewrites a few bytes of
# target\release\galroon-desktop.exe after packaging it, so that one is compared by size only.
foreach ($name in 'galroon-core.exe', 'galroon-mcp.exe', 'galroon-desktop.exe') {
  $built = Join-Path $repo "target\release\$name"
  $installed = Join-Path $InstallDir $name
  if (-not (Test-Path -LiteralPath $built)) { throw "Build output missing: $built (run npm run desktop:build in this checkout)" }
  if ($name -eq 'galroon-desktop.exe') {
    if ((Get-Item -LiteralPath $built).Length -ne (Get-Item -LiteralPath $installed).Length) { throw "Installed $name differs in size from this checkout's build" }
    "size $name $((Get-Item -LiteralPath $installed).Length) (matches target\release)"
    continue
  }
  $a = (Get-FileHash -LiteralPath $built -Algorithm SHA256).Hash; $b = (Get-FileHash -LiteralPath $installed -Algorithm SHA256).Hash
  if ($a -ne $b) { throw "Installed $name ($b) is not this checkout's build ($a)" }
  "sha256 $name $b (matches target\release)"
}
$shortcut = Get-ChildItem -Recurse -Path "$env:APPDATA\Microsoft\Windows\Start Menu\Programs", "$env:ProgramData\Microsoft\Windows\Start Menu\Programs" -Filter 'Galroon*.lnk' -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $shortcut) { throw 'No Galroon entry in the Start menu' }
$target = (New-Object -ComObject WScript.Shell).CreateShortcut($shortcut.FullName).TargetPath
if ($target -ne $app) { throw "Start menu entry opens $target, not $app" }
"start menu: $($shortcut.FullName) -> $target"

"== 2. First launch with a fresh data folder"
$data = Join-Path $DataRoot ('data-' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
New-Item -ItemType Directory -Path $data | Out-Null
$launch = Join-Path $DataRoot 'launch-acceptance.cmd'
Set-Content -LiteralPath $launch -Encoding ASCII -Value @(
  '@echo off',
  "set GALROON_DATA=$data",
  "set WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=$CdpPort --remote-debugging-address=127.0.0.1",
  "start `"`" `"$app`""
)
# Over SSH USERDOMAIN reads WORKGROUP, which cannot be mapped to an account; whoami gives COMPUTER\user.
$user = (whoami)
Unregister-ScheduledTask -TaskName $task -Confirm:$false -ErrorAction SilentlyContinue
Register-ScheduledTask -TaskName $task -Action (New-ScheduledTaskAction -Execute 'cmd.exe' -Argument "/c `"$launch`"") -Principal (New-ScheduledTaskPrincipal -UserId $user -LogonType Interactive) | Out-Null
try {
  Start-ScheduledTask -TaskName $task
  $deadline = (Get-Date).AddSeconds(90)
  while ($true) {
    try { Invoke-RestMethod "http://127.0.0.1:$CdpPort/json/version" -TimeoutSec 2 | Out-Null; break } catch {}
    if ((Get-Date) -gt $deadline) { throw "The app did not open its debugging port $CdpPort (is a user signed in to the desktop?)" }
    Start-Sleep -Seconds 2
  }
  "app window is up; data: $data"

  "== 3-4. Source, covers, 0 false matches, watcher"
  $env:GALROON_CDP = "http://127.0.0.1:$CdpPort"
  $env:GALROON_WIN_SOURCE = $Source
  $env:GALROON_EXPECT_VERSION = $expected
  Push-Location -LiteralPath $repo
  try { npx playwright test -c playwright.windows.config.ts; $code = $LASTEXITCODE } finally { Pop-Location }
} finally {
  Unregister-ScheduledTask -TaskName $task -Confirm:$false -ErrorAction SilentlyContinue
  Get-Process galroon-desktop, galroon-core -ErrorAction SilentlyContinue | Stop-Process -Force
}
if ($code -ne 0) { throw "Windows acceptance failed (Playwright exit $code)" }
'US-4.1 PASSED'
