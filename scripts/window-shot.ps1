# Launch a desktop exe, wait, print its window title and save a PrintWindow capture of its window (WP7 evidence).
#   powershell -File scripts/window-shot.ps1 -Exe <path> -Out <png> [-Wait 12] [-Args "..."]
param([Parameter(Mandatory)][string]$Exe, [Parameter(Mandatory)][string]$Out, [int]$Wait = 12, [string]$ExeArgs = "", [string]$Click = "")
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class WinShot {
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, UIntPtr e);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
}
"@
$p = if ($ExeArgs) { Start-Process -FilePath $Exe -ArgumentList $ExeArgs -PassThru } else { Start-Process -FilePath $Exe -PassThru }
Start-Sleep -Seconds $Wait
$p.Refresh()
"exe=$Exe title=[$($p.MainWindowTitle)] responding=$($p.Responding)"
$h = $p.MainWindowHandle
if ($h -ne [IntPtr]::Zero) {
  $r = New-Object WinShot+RECT
  [void][WinShot]::GetWindowRect($h, [ref]$r)
  $w = $r.Right - $r.Left; $ht = $r.Bottom - $r.Top
  # -Click "x,y;x,y": window-relative clicks (an interactive desktop is needed), one second apart.
  foreach ($c in ($Click -split ';' | Where-Object { $_ })) {
    $xy = $c -split ','
    [void][WinShot]::SetForegroundWindow($h)
    [void][WinShot]::SetCursorPos($r.Left + [int]$xy[0], $r.Top + [int]$xy[1])
    Start-Sleep -Milliseconds 300
    [WinShot]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero); [WinShot]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Seconds 2
  }
  if ($w -gt 0 -and $ht -gt 0) {
    $bmp = New-Object System.Drawing.Bitmap $w, $ht
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $dc = $g.GetHdc()
    [void][WinShot]::PrintWindow($h, $dc, 2)
    $g.ReleaseHdc($dc); $g.Dispose()
    $bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
    "saved $Out ${w}x${ht}"
  }
}
Stop-Process -Id $p.Id -Force
