@echo off
setlocal EnableExtensions
title LedgerCraft
cd /d "%~dp0"

rem ---------------------------------------------------------------------------
rem  LedgerCraft launcher
rem    start.bat           build (if needed) and open LedgerCraft in its own window
rem    start.bat browser   same, but open in your default browser instead
rem    start.bat test      run the accounting test-suite
rem    start.bat clean     delete build output and rebuild from scratch
rem    start.bat server    build and run the server only (no window), e.g. for testing
rem  Your data lives in %APPDATA%\LedgerCraft (one .lcdb file per organisation).
rem ---------------------------------------------------------------------------

rem Build output goes outside OneDrive so syncing doesn't fight the compiler.
set "CARGO_TARGET_DIR=%LOCALAPPDATA%\LedgerCraft\build\target"
set "EXE=%CARGO_TARGET_DIR%\release\ledger-server.exe"
set "MODE=%~1"

where cargo >nul 2>nul || (
  echo [!] Rust is not installed. Install it from https://rustup.rs then run start.bat again.
  pause & exit /b 1
)
where npm >nul 2>nul || (
  echo [!] Node.js is not installed. Install the LTS version from https://nodejs.org then run start.bat again.
  pause & exit /b 1
)

if /I "%MODE%"=="clean" (
  echo Cleaning build output...
  if exist "%CARGO_TARGET_DIR%" rmdir /s /q "%CARGO_TARGET_DIR%"
  if exist "ui\build" rmdir /s /q "ui\build"
  set "MODE="
)

if /I "%MODE%"=="test" (
  echo Running accounting tests...
  cargo test -p ledger-core
  pushd ui & call npm test --silent & popd
  pause & exit /b
)

echo [1/3] Preparing the interface...
pushd ui
if not exist node_modules (
  call npm install --no-audit --no-fund || (popd & echo [!] npm install failed & pause & exit /b 1)
)
call npm run build --silent || (popd & echo [!] UI build failed & pause & exit /b 1)
popd

echo [2/3] Building LedgerCraft (first build takes a few minutes, later ones are quick)...
cargo build --release -p ledger-server || (echo [!] Build failed & pause & exit /b 1)

rem Stop a previous instance so the new build can start.
taskkill /im ledger-server.exe /f >nul 2>nul

echo [3/3] Starting LedgerCraft...
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
echo LedgerCraft is running at http://127.0.0.1:4180
echo The server runs minimised in the taskbar. Close that window to stop LedgerCraft.
timeout /t 4 >nul
endlocal
