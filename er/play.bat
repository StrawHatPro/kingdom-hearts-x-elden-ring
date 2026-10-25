@echo off
rem Kingdom Hearts x Elden Ring: optional shortcut. OFFLINE ONLY (no Easy Anti-Cheat, no online play).
rem Starts Elden Ring through me3 with the mod's profile. Steam must be running.
setlocal
cd /d "%~dp0"
set "ME3=me3"
where me3 >nul 2>nul || (
  if exist "%LOCALAPPDATA%\Programs\me3\bin\me3.exe" set "ME3=%LOCALAPPDATA%\Programs\me3\bin\me3.exe"
  if exist "%LOCALAPPDATA%\me3\bin\me3.exe" set "ME3=%LOCALAPPDATA%\me3\bin\me3.exe"
  if exist "%ProgramFiles%\me3\bin\me3.exe" set "ME3=%ProgramFiles%\me3\bin\me3.exe"
)
if not exist "%~dp0mods\kh_er_loader.dll" (echo The mod is not built yet: run setup\setup.bat first. & pause & exit /b 1)
"%ME3%" launch -g eldenring -p "%~dp0profiles\kh-sora.me3"
if errorlevel 1 (echo me3 could not start the game. Is me3 installed? https://github.com/garyttierney/me3 & pause)
