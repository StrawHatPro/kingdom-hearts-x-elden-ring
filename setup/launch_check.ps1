# Kingdom Hearts x Elden Ring: used by er\play.bat before it starts me3, so play.bat never has to be run as administrator.
# A program started normally (like play.bat and me3) cannot start a game that Windows is told to always run as
# administrator, or one that has to talk to a Steam that runs as administrator. This finds those two cases.
#   (no switch)  prints one line per problem: "USERFLAG|<exe>", "ALLFLAG|<exe>", "STEAMADMIN"
#   -Fix         removes "Run this program as an administrator" from eldenring.exe / steam.exe for this Windows user
#                (only that setting; any other compatibility settings are kept)
param([switch]$Fix)
$ErrorActionPreference = 'SilentlyContinue'
$admin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
  [Security.Principal.WindowsBuiltInRole]::Administrator)
if ($admin) { exit 0 }
$layers = 'Software\Microsoft\Windows NT\CurrentVersion\AppCompatFlags\Layers'
$games = '\\(eldenring|steam)\.exe$'
foreach ($hive in @(@{ Root = 'HKCU:'; Tag = 'USERFLAG' }, @{ Root = 'HKLM:'; Tag = 'ALLFLAG' })) {
  $key = Get-Item -LiteralPath "$($hive.Root)\$layers"
  if (-not $key) { continue }
  foreach ($exe in $key.GetValueNames()) {
    if ($exe -notmatch $games) { continue }
    $flags = [string]$key.GetValue($exe)
    if ($flags -notmatch '\bRUNASADMIN\b') { continue }
    if ($Fix -and $hive.Tag -eq 'USERFLAG') {
      $rest = (($flags -split '\s+') | Where-Object { $_ -and $_ -ne 'RUNASADMIN' }) -join ' '
      if ($rest -eq '~' -or $rest -eq '') { Remove-ItemProperty -LiteralPath "HKCU:\$layers" -Name $exe }
      else { Set-ItemProperty -LiteralPath "HKCU:\$layers" -Name $exe -Value $rest }
      continue
    }
    "$($hive.Tag)|$exe"
  }
}
if (-not $Fix) {
  foreach ($p in Get-Process -Name steam) {
    try { $null = $p.Handle } catch { 'STEAMADMIN'; break }
  }
}
