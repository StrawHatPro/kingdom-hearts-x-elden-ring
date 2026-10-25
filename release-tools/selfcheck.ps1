# The offline check of a released kh_er.dll: loads er\mods\kh_er.dll in this PowerShell process (not in the game), calls its
# kh_er_selfcheck export, which finds the pack (er\mods\kh-er.cfg or build\sora_pack) and reads every generated table made
# from KH3 data (pack\gen, made by the setup), and prints er\mods\kh-er-selfcheck.txt. Exit code 0 = every table read.
# Also checks that kh_er_loader.dll is the loader for Elden Ring 2.7.1.0 and 2.7.2.0 (its own version text).
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File release-tools\selfcheck.ps1 [-Mods <folder of the DLLs>]
param([string]$Mods = (Join-Path (Split-Path -Parent $PSScriptRoot) 'er\mods'))
$ErrorActionPreference = 'Stop'
$dll = Join-Path $Mods 'kh_er.dll'
$loader = Join-Path $Mods 'kh_er_loader.dll'
if (-not (Test-Path $dll)) { Write-Host "no $dll"; exit 2 }
$ok = $true
if (Test-Path $loader) {
    $text = [System.Text.Encoding]::ASCII.GetString([System.IO.File]::ReadAllBytes($loader))
    if ($text.Contains('2.7.1.0 or 2.7.2.0') -and -not $text.Contains('2.7.1.0 (app 1.17.1) only')) { Write-Host 'loader: built for Elden Ring 2.7.1.0 and 2.7.2.0' }
    else { Write-Host 'loader: NOT the 2.7.1.0 / 2.7.2.0 loader'; $ok = $false }
} else { Write-Host "no $loader"; $ok = $false }
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class KhErSelfCheck {
    [DllImport("kernel32", SetLastError = true, CharSet = CharSet.Unicode)] public static extern IntPtr LoadLibraryW(string path);
    [DllImport("kernel32", SetLastError = true, CharSet = CharSet.Ansi)] public static extern IntPtr GetProcAddress(IntPtr module, string name);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] public delegate uint Check([MarshalAs(UnmanagedType.LPWStr)] string dir, UIntPtr len);
    public static uint Run(string dll, string dir) {
        IntPtr m = LoadLibraryW(dll);
        if (m == IntPtr.Zero) throw new Exception("LoadLibrary failed: " + Marshal.GetLastWin32Error());
        IntPtr f = GetProcAddress(m, "kh_er_selfcheck");
        if (f == IntPtr.Zero) throw new Exception("kh_er_selfcheck is not exported: not a release build of kh_er.dll");
        Check c = (Check)Marshal.GetDelegateForFunctionPointer(f, typeof(Check));
        return c(dir, (UIntPtr)dir.Length);
    }
}
'@
$dir = (Resolve-Path $Mods).Path
$n = [KhErSelfCheck]::Run((Resolve-Path $dll).Path, $dir)
$report = Join-Path $Mods 'kh-er-selfcheck.txt'
if (Test-Path $report) { Get-Content $report | ForEach-Object { Write-Host "  $_" } }
if ($n -eq 0) { Write-Host 'kh_er.dll: the generated tables were NOT read'; $ok = $false } else { Write-Host "kh_er.dll: $n table group(s) read" }
if ($ok) { exit 0 } else { exit 1 }
