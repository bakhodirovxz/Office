# Drives the native "Open" file dialog that a process opened (no keystrokes to the foreground window, so it works on any desktop state).
#   powershell -File scripts/plugins/uia-file-dialog.ps1 -ProcessId <pid> -Path <file> [-Cancel] [-Wait 30]
# Finds the dialog (a top-level window of that process of class #32770, by EnumWindows). Cancel closes it (WM_CLOSE, as the X does). Otherwise the path goes into the
# "File name" box (a legacy ComboBoxEx32, control id 1148, which UI Automation does not expose with a value) with WM_SETTEXT and the Open button (id 1) gets BM_CLICK.
# One JSON line: { found, title, action, dialogs_left }. Exit 0 when done, 2 when no dialog, 3 when no file name box.
param([Parameter(Mandatory)][int]$ProcessId, [string]$Path = "", [switch]$Cancel, [int]$Wait = 30)
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Add-Type @"
using System; using System.Collections.Generic; using System.Runtime.InteropServices; using System.Text;
public class Win {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc p, IntPtr l);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr h, int id);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr FindWindowEx(IntPtr p, IntPtr a, string cls, string w);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, string l);
  [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  public static List<IntPtr> Dialogs(uint pid) {
    var r = new List<IntPtr>();
    EnumWindows((h, l) => { uint p; GetWindowThreadProcessId(h, out p); var c = new StringBuilder(64); GetClassName(h, c, 64);
      if (p == pid && c.ToString() == "#32770" && IsWindowVisible(h)) r.Add(h); return true; }, IntPtr.Zero);
    return r;
  }
  public static string Title(IntPtr h) { var s = new StringBuilder(256); GetWindowText(h, s, 256); return s.ToString(); }
}
"@
$hd = [IntPtr]::Zero
for ($i = 0; $i -lt $Wait -and $hd -eq [IntPtr]::Zero; $i++) {
  $d = [Win]::Dialogs([uint32]$ProcessId)
  if ($d.Count -gt 0) { $hd = $d[0] } else { Start-Sleep -Seconds 1 }
}
if ($hd -eq [IntPtr]::Zero) { '{"found":false}'; exit 2 }
$title = [Win]::Title($hd)
if ($Cancel) {
  [void][Win]::PostMessage($hd, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero); $action = 'cancel (WM_CLOSE)'
} else {
  $cb = [Win]::GetDlgItem($hd, 1148)
  $inner = [Win]::FindWindowEx($cb, [IntPtr]::Zero, 'ComboBox', $null)
  $he = [Win]::FindWindowEx($inner, [IntPtr]::Zero, 'Edit', $null)
  if ($he -eq [IntPtr]::Zero) { [Console]::Error.WriteLine("no Edit in the file name box (dialog $hd, combo $cb, inner $inner)"); exit 3 }
  [void][Win]::SendMessage($he, 0x000C, [IntPtr]::Zero, $Path)   # WM_SETTEXT
  Start-Sleep -Milliseconds 500
  [void][Win]::SendMessage([Win]::GetDlgItem($hd, 1), 0x00F5, [IntPtr]::Zero, [IntPtr]::Zero); $action = 'open (path set, Open clicked)'   # BM_CLICK
}
Start-Sleep -Milliseconds 800
$left = @([Win]::Dialogs([uint32]$ProcessId))
(@{ found = $true; title = $title; action = $action; dialogs_left = $left.Count } | ConvertTo-Json -Compress)
exit 0
