@echo off
setlocal EnableExtensions
title LedgerCraft
cd /d "%~dp0"

rem ---------------------------------------------------------------------------
rem  LedgerCraft launcher
rem    start.bat              build (if needed) and open the NATIVE app (100% Rust, iced)
rem                           in your last layout (QuickBooks Desktop-style the first time)
rem    start.bat desktop      same, opening in the QuickBooks Desktop-style layout
rem    start.bat online       same, opening in the QuickBooks Online-style layout
rem    start.bat xero         same, opening in the Xero-style layout
rem                           (switch any time: View menu / gear menu / Preferences)
rem    start.bat web          the previous web version (full feature set while the native app is built)
rem    start.bat browser      the web version in your default browser
rem    start.bat server       the web version's server only (no window), e.g. for testing
rem    start.bat test         run all tests (accounting engine + native app)
rem    start.bat clean        delete build output and rebuild from scratch
rem  Your data lives in %APPDATA%\LedgerCraft (one .lcdb file per organisation) and is
rem  shared by both versions.
rem ---------------------------------------------------------------------------

rem Build output goes outside OneDrive so syncing doesn't fight the compiler.
set "CARGO_TARGET_DIR=%LOCALAPPDATA%\LedgerCraft\build\target"
set "APP=%CARGO_TARGET_DIR%\release\ledgercraft.exe"
set "EXE=%CARGO_TARGET_DIR%\release\ledger-server.exe"
set "MODE=%~1"

where cargo >nul 2>nul || (
  echo [!] Rust is not installed. Install it from https://rustup.rs then run start.bat again.
  pause & exit /b 1
)

if /I "%MODE%"=="clean" (
  echo Cleaning build output...
  if exist "%CARGO_TARGET_DIR%" rmdir /s /q "%CARGO_TARGET_DIR%"
  if exist "ui\build" rmdir /s /q "ui\build"
  set "MODE="
)

if /I "%MODE%"=="test" (
  echo Running tests...
  cargo test -p ledger-core -p ledger-app
  pause & exit /b
)

if /I "%MODE%"=="web" goto web
if /I "%MODE%"=="browser" goto web
if /I "%MODE%"=="server" goto web

rem ----------------------------- native app ---------------------------------
echo [1/2] Building LedgerCraft (first build takes several minutes, later ones are quick)...
cargo build --release -p ledger-app || (echo [!] Build failed & pause & exit /b 1)
taskkill /im ledgercraft.exe /f >nul 2>nul
echo [2/2] Starting LedgerCraft...
set "SKINARG="
if /I "%MODE%"=="xero" set "SKINARG=--skin xero"
if /I "%MODE%"=="quickbooks" set "SKINARG=--skin quickbooks"
if /I "%MODE%"=="online" set "SKINARG=--skin quickbooks"
if /I "%MODE%"=="desktop" set "SKINARG=--skin desktop"
start "" "%APP%" %SKINARG%
endlocal
exit /b

rem ------------------------------ web version -------------------------------
:web
where npm >nul 2>nul || (
  echo [!] Node.js is needed for the web version. Install the LTS version from https://nodejs.org
  pause & exit /b 1
)
echo [1/3] Preparing the web interface...
pushd ui
if not exist node_modules (
  call npm install --no-audit --no-fund || (popd & echo [!] npm install failed & pause & exit /b 1)
)
call npm run build --silent || (popd & echo [!] UI build failed & pause & exit /b 1)
popd

echo [2/3] Building the server...
cargo build --release -p ledger-server || (echo [!] Build failed & pause & exit /b 1)

rem Stop a previous instance so the new build can start.
taskkill /im ledger-server.exe /f >nul 2>nul

echo [3/3] Starting LedgerCraft (web version)...
if /I "%MODE%"=="server" (
  echo LedgerCraft server starting at http://127.0.0.1:4180 - press Ctrl+C to stop.
  "%EXE%"
  exit /b
)
if /I "%MODE%"=="browser" (
  start "LedgerCraft server" /min "%EXE%" --open
) else (
  start "LedgerCraft server" /min "%EXE%" --app
)
echo.
echo LedgerCraft (web version) is running at http://127.0.0.1:4180
echo The server runs minimised in the taskbar. Close that window to stop it.
timeout /t 4 >nul
endlocal
