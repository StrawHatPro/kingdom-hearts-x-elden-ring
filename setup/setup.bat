@echo off
rem Kingdom Hearts x Elden Ring: the first-run setup. In a release download, double-click setup.exe in the mod's folder;
rem this file runs it too. Options are passed on, e.g.
rem   setup.bat --kh3 "D:\Games\KINGDOM HEARTS III" --er "D:\SteamLibrary\steamapps\common\ELDEN RING"
rem When a game is not found, a folder picker opens.
rem Node.js is never needed beforehand:
rem   0. setup.exe (a release download: Node.js built in, as a single executable application) runs the setup;
rem   1. else tools\bin\node\node.exe (the portable Node a source checkout downloaded before, see 3);
rem   2. else a Node.js 20 or newer on PATH (a source checkout);
rem   3. else the pinned portable Node.js (release-tools\node_pin.json, from nodejs.org, checked by its SHA-256) is downloaded
rem      into tools\bin\node (setup\get_node.ps1); not with --no-download.
rem The npm packages come bundled in a release download; a source checkout installs the locked versions once (npm ci).
setlocal EnableExtensions
cd /d "%~dp0.."
set "KH_SETUP_PICKER=1"
set "KH_SETUP_NO_PAUSE=1"
if not exist "setup.exe" goto no_setup_exe
"%CD%\setup.exe" %*
set "CODE=%ERRORLEVEL%"
pause
exit /b %CODE%
:no_setup_exe
set "NODE="
set "NPM="
set "NODL="
set "NODE_MAJOR="
for %%a in (%*) do if /i "%%~a"=="--no-download" set "NODL=1"
if exist "tools\bin\node\node.exe" set "NODE=%CD%\tools\bin\node\node.exe"
if defined NODE goto have_node
for /f "delims=" %%v in ('node -p "process.versions.node.split('.')[0]" 2^>nul') do set "NODE_MAJOR=%%v"
if not defined NODE_MAJOR goto no_path_node
if %NODE_MAJOR% GEQ 20 set "NODE=node"
:no_path_node
if defined NODE set "NPM=npm"
if defined NODE goto have_node
if defined NODL (
  echo No Node.js 20 or newer was found, and --no-download was given: run without --no-download, or install Node.js 20+.
  pause
  exit /b 1
)
echo Getting the portable Node.js the mod uses, into tools\bin\node ...
powershell -NoProfile -ExecutionPolicy Bypass -File "setup\get_node.ps1"
if errorlevel 1 (
  echo Could not get Node.js: check the connection, or install Node.js 20+ and run this again.
  pause
  exit /b 1
)
set "NODE=%CD%\tools\bin\node\node.exe"
:have_node
if exist "node_modules\gltf-validator\package.json" if exist "node_modules\iced-x86\package.json" if exist "node_modules\ajv\package.json" if exist "node_modules\@wasm-audio-decoders\ogg-vorbis\package.json" goto have_modules
if defined NODL (
  echo The npm packages are missing and --no-download was given: use a release download, or run without --no-download.
  pause
  exit /b 1
)
if not defined NPM if exist "tools\bin\node\npm.cmd" set "NPM=%CD%\tools\bin\node\npm.cmd"
if not defined NPM (
  echo The npm packages are missing and there is no npm: use a release download, or delete tools\bin\node and run this again.
  pause
  exit /b 1
)
echo Installing the locked npm packages (npm ci) ...
call "%NPM%" ci --omit=dev --no-fund --no-audit
if errorlevel 1 (
  echo npm ci failed.
  pause
  exit /b 1
)
:have_modules
"%NODE%" setup\setup.js %*
set "CODE=%ERRORLEVEL%"
pause
exit /b %CODE%
