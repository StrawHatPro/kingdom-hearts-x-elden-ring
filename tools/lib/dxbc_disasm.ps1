param([string]$Dir)
Add-Type -TypeDefinition @"
using System; using System.Runtime.InteropServices;
public static class D3D {
  [DllImport("d3dcompiler_47.dll")] public static extern int D3DDisassemble(byte[] src, IntPtr size, uint flags, string comments, out IntPtr blob);
  [ComImport, Guid("8BA5FB08-5195-40e2-AC58-0D989C3A0102"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
  public interface ID3DBlob { [PreserveSig] IntPtr GetBufferPointer(); [PreserveSig] IntPtr GetBufferSize(); }
  public static string Dis(byte[] b) { IntPtr p; int hr = D3DDisassemble(b, (IntPtr)b.Length, 0, null, out p); if (hr != 0) return "HRESULT " + hr.ToString("X"); ID3DBlob blob = (ID3DBlob)Marshal.GetObjectForIUnknown(p); string s = Marshal.PtrToStringAnsi(blob.GetBufferPointer()); Marshal.Release(p); return s; }
}
"@
Get-ChildItem -Path $Dir -Filter *.bin | ForEach-Object {
  $asm = $_.FullName -replace '\.bin$', '.asm'
  if (-not (Test-Path $asm)) { [IO.File]::WriteAllText($asm, [D3D]::Dis([IO.File]::ReadAllBytes($_.FullName))) }
}
