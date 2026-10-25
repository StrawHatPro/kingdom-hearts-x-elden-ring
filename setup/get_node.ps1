# Kingdom Hearts x Elden Ring: downloads the pinned portable Node.js (release-tools/node_pin.json: the official win-x64 zip
# of nodejs.org, checked by its SHA-256 from that release's SHASUMS256.txt) into tools\bin\node. Used by setup\setup.bat
# in a source checkout that has no Node 20 or newer; a release download has node.exe bundled and never runs this.
#   powershell -NoProfile -ExecutionPolicy Bypass -File setup\get_node.ps1
# Exit code 0 = tools\bin\node\node.exe is the pinned Node.
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$pin = Get-Content -Raw -LiteralPath (Join-Path $repo 'release-tools\node_pin.json') | ConvertFrom-Json
$bin = Join-Path $repo 'tools\bin'
$dest = Join-Path $bin 'node'
$zip = Join-Path $bin ("node-v{0}-win-x64.zip" -f $pin.version)
$exe = Join-Path $dest 'node.exe'
function Sha256($p) { (Get-FileHash -Algorithm SHA256 -LiteralPath $p).Hash.ToLowerInvariant() }
if ((Test-Path -LiteralPath $exe) -and ((Sha256 $exe) -eq $pin.node_exe_sha256)) { Write-Host "   Node $($pin.version): $exe (already there)"; exit 0 }
New-Item -ItemType Directory -Force -Path $bin | Out-Null
Write-Host "   downloading Node.js $($pin.version) (portable, official): $($pin.zip_url)"
try {
    $ProgressPreference = 'SilentlyContinue'
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    Invoke-WebRequest -UseBasicParsing -Uri $pin.zip_url -OutFile $zip
} catch { Write-Host "   the download failed: $($_.Exception.Message)"; exit 1 }
$got = Sha256 $zip
if ($got -ne $pin.zip_sha256) { Remove-Item -Force -LiteralPath $zip; Write-Host "   the download is not the expected file (SHA-256 $got, expected $($pin.zip_sha256))"; exit 1 }
$tmp = Join-Path $bin 'node_unpack'
if (Test-Path -LiteralPath $tmp) { Remove-Item -Recurse -Force -LiteralPath $tmp }
Expand-Archive -LiteralPath $zip -DestinationPath $tmp -Force
if (Test-Path -LiteralPath $dest) { Remove-Item -Recurse -Force -LiteralPath $dest }
Move-Item -LiteralPath (Join-Path $tmp $pin.zip_folder) -Destination $dest
Remove-Item -Recurse -Force -LiteralPath $tmp
Remove-Item -Force -LiteralPath $zip
if ((Sha256 $exe) -ne $pin.node_exe_sha256) { Write-Host "   node.exe is not the expected file"; exit 1 }
Write-Host "   Node $($pin.version): $exe"
exit 0
