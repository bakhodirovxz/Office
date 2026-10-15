Add-Type -AssemblyName System.Drawing, System.Windows.Forms
$exe='E:\office-tools\releases\win\gridcraft-0.7.0-windows-x64-portable\gridcraft-0.7.0-windows-x64-portable\gridcraft.exe'
$t0=Get-Date; $p=Start-Process $exe -ArgumentList 'E:\office-target\xlsx-bench\w40.xlsx' -PassThru
$peak=0; $first=$null; $titleSeen=$null; $shot=$false
while(((Get-Date)-$t0).TotalSeconds -lt 240){
  $p.Refresh(); if($p.HasExited){ "EXITED code $($p.ExitCode) after $([int]((Get-Date)-$t0).TotalSeconds)s"; break }
  if($p.PeakWorkingSet64 -gt $peak){$peak=$p.PeakWorkingSet64}
  $title=$p.MainWindowTitle
  if($title -and -not $first){ $first=((Get-Date)-$t0).TotalSeconds; "window first seen at $first s title='$title' responding=$($p.Responding)" }
  if($title -like '*w40*' -and -not $titleSeen){ $titleSeen=((Get-Date)-$t0).TotalSeconds; "title has file name at $titleSeen s: '$title' responding=$($p.Responding) peakMB=$([int]($peak/1MB))" }
  if($titleSeen -and ((Get-Date)-$t0).TotalSeconds -gt $titleSeen+8 -and -not $shot){
     $b=New-Object System.Drawing.Bitmap([System.Windows.Forms.SystemInformation]::VirtualScreen.Width,[System.Windows.Forms.SystemInformation]::VirtualScreen.Height)
     $g=[System.Drawing.Graphics]::FromImage($b); $g.CopyFromScreen(0,0,0,0,$b.Size); $b.Save('C:\Users\uge226\Desktop\office\docs\phase0\xlsx-bench\gui-w40.png'); $shot=$true; "screenshot saved; responding=$($p.Responding)"; break }
  Start-Sleep -Milliseconds 250
}
$p.Refresh(); if($p.PeakWorkingSet64 -gt $peak){$peak=$p.PeakWorkingSet64}
"final peakMB=$([int]($peak/1MB)) responding=$($p.Responding) title='$($p.MainWindowTitle)' total=$([int]((Get-Date)-$t0).TotalSeconds)s"
if(-not $p.HasExited){ $null=$p.CloseMainWindow(); Start-Sleep 3; if(-not $p.HasExited){$p.Kill()} }
