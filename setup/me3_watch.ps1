# (SHUTDOWN 2026-11-07) Starts Elden Ring through me3 and makes sure me3's launcher never outlives the game.
#
# Why: me3-launcher.exe (me3 0.13.0) talks to Steam as ELDEN RING, so while it runs Steam shows the game as RUNNING.
# When eldenring.exe dies during me3's start-up, me3's launcher waits for it forever, the me3 window never returns, and
# closing that window leaves the launcher behind (seen 2026-11-06: three of them kept Steam saying "running" for a day).
#
# What it does, and nothing else:
#   1. before the start: if no Elden Ring is running, ends me3 launchers left over from earlier runs (if Elden Ring IS
#      running, it says so and starts nothing);
#   2. runs me3 in this window, with the same command line the .bat used before;
#   3. while me3 runs: once the game it started has gone (quit, Alt+F4, crash), me3's launcher gets GraceSec seconds to
#      finish on its own, then it and the me3 command are ended; a launcher with no game NoGameSec seconds after it
#      started is ended too;
#   4. after me3 returns: a launcher of this run still alive with no game is ended.
# Only ever ends me3-launcher.exe processes whose executable is directly in the folder of the me3.exe it was given
# (compared by full path, case-insensitive), and the me3 command it started itself; never anything by name alone, never the game.
# Every process it ends is printed as one line in this window.
# NOT PERSISTENT: it runs only while this window runs and exits with it. No autostart, no scheduled task, no service, no
# registry writes, no files written, no network, no administrator rights.
# Exit code: me3's own; 3 when a stuck launcher had to be ended; 2 when Elden Ring was already running.
#   powershell -NoProfile -ExecutionPolicy Bypass -File me3_watch.ps1 -Me3 <me3.exe> -Profile <profile.me3>
# (-ArgumentLine replaces me3's arguments, and the Game / Launcher names can be changed: used by the tests only.)
param(
    [Parameter(Mandatory)][string]$Me3,
    [string]$Profile = '',
    [string]$ArgumentLine = '',
    [int]$GraceSec = 15,
    [int]$NoGameSec = 90,
    [int]$StaleSec = 30,
    [string]$Game = 'eldenring',
    [string]$Launcher = 'me3-launcher'
)
$ErrorActionPreference = 'Continue'

# the folder of THIS me3 (me3.exe and me3-launcher.exe sit side by side in its bin folder): only launchers whose exe is
# directly in it are ever ended (not in a subfolder, not anywhere else)
$me3Dir = [IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($Me3)).TrimEnd('\')

function Get-Games { @(Get-Process -Name $Game -ErrorAction SilentlyContinue) }
function Get-OurLaunchers {
    @(Get-CimInstance Win32_Process -Filter "Name='$Launcher.exe'" -ErrorAction SilentlyContinue | Where-Object {
        $_.ExecutablePath -and [string]::Equals([IO.Path]::GetDirectoryName($_.ExecutablePath).TrimEnd('\'), $me3Dir, [StringComparison]::OrdinalIgnoreCase)
    })
}
function Get-Age($p) { if ($p.CreationDate) { ((Get-Date) - $p.CreationDate).TotalSeconds } else { 0 } }
function Stop-Launchers($procs, [string]$what) {
    foreach ($p in $procs) {
        try {
            Stop-Process -Id $p.ProcessId -Force -ErrorAction Stop
            Write-Host "  ended $what from $($p.CreationDate) (pid $($p.ProcessId))"
        } catch {
            if (Get-Process -Id $p.ProcessId -ErrorAction SilentlyContinue) { Write-Host "  could not end $what (pid $($p.ProcessId)): $($_.Exception.Message)" }
        }
    }
}

# 1. leftovers of earlier runs
$games = @(Get-Games)
if ($games.Count -gt 0) {
    Write-Host "Elden Ring is already running (pid $(($games | ForEach-Object Id) -join ', ')). Close it first, then start again."
    exit 2
}
$stale = @(Get-OurLaunchers | Where-Object { (Get-Age $_) -ge $StaleSec })
if ($stale.Count -gt 0) {
    Write-Host "me3 launchers left over from earlier runs (they make Steam show Elden Ring as running):"
    Stop-Launchers $stale 'a leftover me3 launcher'
}

# 2. me3, in this window
if (-not $ArgumentLine) { $ArgumentLine = "launch -g eldenring -p `"$Profile`"" }
$start = (Get-Date).AddSeconds(-1)
$cli = Start-Process -FilePath $Me3 -ArgumentList $ArgumentLine -NoNewWindow -PassThru
$null = $cli.Handle # keeps the exit code readable after the process ends (Windows PowerShell 5.1)

# 3. watch
$mine = { @(Get-OurLaunchers | Where-Object { $_.CreationDate -ge $start }) }
$gameSeen = $false
$goneAt = $null
$stuck = $false
while (-not $cli.HasExited) {
    Start-Sleep -Milliseconds 500
    $alive = @(Get-Games | Where-Object { $_.StartTime -ge $start })
    if ($alive.Count -gt 0) { $gameSeen = $true; $goneAt = $null; continue }
    $launchers = @(& $mine) # @(): a single CimInstance has no .Count
    if ($launchers.Count -eq 0) { continue }
    $why = $null
    if ($gameSeen) {
        if ($null -eq $goneAt) { $goneAt = Get-Date }
        if (((Get-Date) - $goneAt).TotalSeconds -ge $GraceSec) { $why = "Elden Ring closed $GraceSec s ago but me3's launcher is still running" }
    } else {
        $oldest = ($launchers | ForEach-Object { Get-Age $_ } | Measure-Object -Maximum).Maximum
        if ($oldest -ge $NoGameSec) { $why = "me3's launcher has run $NoGameSec s without Elden Ring (the game closed or crashed while starting)" }
    }
    if ($why) {
        Write-Host ""
        Write-Host "$why; ending it so Steam no longer shows the game as running:"
        Stop-Launchers $launchers "me3's stuck launcher"
        if (-not $cli.HasExited) {
            Stop-Process -Id $cli.Id -Force -ErrorAction SilentlyContinue
            Write-Host "  ended the stuck me3 command (pid $($cli.Id))"
        }
        $stuck = $true
        break
    }
}

# 4. after me3
if (-not $stuck -and @(Get-Games).Count -eq 0) {
    $left = @(& $mine)
    if ($left.Count -gt 0) {
        Write-Host "me3 returned but its launcher is still running without the game:"
        Stop-Launchers $left "me3's leftover launcher"
    }
}
if ($stuck) { exit 3 }
$cli.WaitForExit()
exit $cli.ExitCode
