@echo off
rem Kingdom Hearts x Elden Ring: starts Elden Ring through me3 with the mod's profile.
rem OFFLINE ONLY (no Easy Anti-Cheat, no online play). Steam must be running.
rem me3 is looked for in: the place remembered in er\me3_path.txt, PATH, the usual install folders;
rem if it is still not found, a window asks you to pick me3.exe once and remembers it.
setlocal
cd /d "%~dp0"
if not exist "%~dp0mods\kh_er_loader.dll" (
  echo The mod is not set up yet: run setup.exe first.
  pause
  exit /b 1
)

set "SAVED=%~dp0me3_path.txt"
set "ME3="
if exist "%SAVED%" set /p ME3=<"%SAVED%"
if defined ME3 if exist "%ME3%" goto run
set "ME3="

for /f "delims=" %%P in ('where me3.exe 2^>nul') do if not defined ME3 set "ME3=%%P"
if defined ME3 goto save
for %%P in ("%LOCALAPPDATA%\Programs\me3\bin\me3.exe" "%LOCALAPPDATA%\me3\bin\me3.exe" "%ProgramFiles%\me3\bin\me3.exe" "%USERPROFILE%\me3\bin\me3.exe") do if not defined ME3 if exist "%%~P" set "ME3=%%~P"
if defined ME3 goto save

echo me3 was not found. A window will open: pick me3.exe.
echo (It is in the "bin" folder of wherever you unpacked or installed me3.)
for /f "usebackq delims=" %%F in (`powershell -NoProfile -STA -ExecutionPolicy Bypass -File "%~dp0..\setup\find_me3.ps1"`) do set "ME3=%%F"
if defined ME3 goto save
echo No me3.exe was picked. Get me3 from https://github.com/garyttierney/me3/releases and run play.bat again.
pause
exit /b 1

:save
2>nul >"%SAVED%" echo %ME3%

:run
rem me3 older than 0.13.0 may ignore the profile's own save file and start the game on your NORMAL save (ER0000.sl2)
set "VER="
for /f "tokens=2" %%V in ('call "%ME3%" --version 2^>nul') do if not defined VER set "VER=%%V"
set "MAJ=" & set "MIN="
if defined VER for /f "tokens=1,2 delims=." %%a in ("%VER%") do (set "MAJ=%%a" & set "MIN=%%b")
if not defined MAJ goto oldme3
if %MAJ% GTR 0 goto versionok
if not defined MIN goto oldme3
if %MIN% GEQ 13 goto versionok
:oldme3
echo.
echo STOPPED: your me3 is too old (version: %VER%). The mod needs me3 0.13.0 or newer.
echo An older me3 can ignore the mod's own save file and open your NORMAL Elden Ring saves.
echo Update me3 from https://github.com/garyttierney/me3/releases and run play.bat again.
echo (me3 used: "%ME3%". To pick a different one, delete er\me3_path.txt.)
pause
exit /b 1
:versionok
rem play.bat never needs to be run as administrator. A game that Windows is told to always run as administrator (or a
rem Steam running as administrator) can't be started by a normal program, so look for that first and help fix it.
set "USERFLAG=" & set "ALLFLAG=" & set "STEAMADMIN="
for /f "usebackq tokens=1,* delims=|" %%a in (`powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0..\setup\launch_check.ps1"`) do (
  if "%%a"=="USERFLAG" (set "USERFLAG=1" & echo   %%b is set to always run as administrator.)
  if "%%a"=="ALLFLAG" (set "ALLFLAG=1" & echo   %%b is set to always run as administrator, for all users.)
  if "%%a"=="STEAMADMIN" set "STEAMADMIN=1"
)
if defined USERFLAG goto userflag
if defined ALLFLAG goto allflag
if defined STEAMADMIN goto steamadmin

echo Starting Elden Ring with me3 %VER%: "%ME3%"
"%ME3%" launch -g eldenring -p "%~dp0profiles\kh-sora.me3"
if not errorlevel 1 exit /b 0
echo.
echo me3 could not start the game (its own message is above).
echo To pick a different me3.exe, delete er\me3_path.txt and run play.bat again.
pause
exit /b 1

:userflag
echo.
echo Windows is told to always run that program as administrator, so the mod can't start the game normally.
choice /c YN /m "Turn that setting off now (only that setting, only for your Windows user)"
if errorlevel 2 goto keepflag
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0..\setup\launch_check.ps1" -Fix
echo Done. If Steam is open, close it fully (Steam menu, Exit) and start it again. Then press a key here.
pause >nul
goto versionok
:keepflag
echo Nothing was changed. To turn it off yourself: right-click the program, Properties, Compatibility, and untick
echo "Run this program as an administrator". Then run play.bat again.
pause
exit /b 1

:allflag
echo.
echo That setting is on for all users, which needs administrator rights to change once:
echo right-click the program, Properties, Compatibility, "Change settings for all users", and untick
echo "Run this program as an administrator". Then run play.bat again.
pause
exit /b 1

:steamadmin
echo.
echo Steam is running as administrator, so the mod can't start the game normally.
echo Close Steam fully (Steam menu, Exit), start it again normally, then run play.bat again.
pause
exit /b 1
