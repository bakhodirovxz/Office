# Smoke-run the MSVC desktop exes (PLAN WP13): start each with a synthetic file, wait for a window, close it, kill only our own pid.
#   powershell -File scripts/msvc-smoke.ps1 [-Root E:/office-target/p1/msvc] [-Inputs E:/office-target/p3/census/in] [-Wait 15]
param([string]$Root = 'E:/office-target/p1/msvc', [string]$Inputs = 'E:/office-target/p3/census/in', [int]$Wait = 15)
$apps = @(@('docs', 'docs.exe', 'uz_latn.docx'), @('sheets', 'sheets.exe', '02-table-rich.xlsx'), @('slides', 'slides.exe', '01-shapes-notes.pptx'), @('pdf', 'pdf.exe', 'uz_latn.pdf'))
$rc = 0
foreach ($a in $apps) {
  $exe = "$Root/$($a[0])/x86_64-pc-windows-msvc/release/$($a[1])"
  if (-not (Test-Path $exe)) { $exe = (Get-ChildItem "$Root/$($a[0])/x86_64-pc-windows-msvc/release/*.exe" | Where-Object Name -notmatch 'cli|build' | Select-Object -First 1).FullName }
  $args = @(); if ($a[2]) { $args = @("$Inputs/$($a[2])") }
  $t0 = Get-Date
  $p = if ($args) { Start-Process $exe -ArgumentList $args -PassThru } else { Start-Process $exe -PassThru }
  $title = ''
  for ($i = 0; $i -lt $Wait * 2; $i++) { Start-Sleep -Milliseconds 500; $p.Refresh(); if ($p.HasExited) { break }; if ($p.MainWindowTitle) { $title = $p.MainWindowTitle; break } }
  $ms = [int]((Get-Date) - $t0).TotalMilliseconds
  $alive = -not $p.HasExited
  if ($alive) { [void]$p.CloseMainWindow(); if (-not $p.WaitForExit(8000)) { Stop-Process -Id $p.Id -Force; $how = 'killed' } else { $how = 'closed' } } else { $how = "exited($($p.ExitCode))" }
  $ok = $alive -and $title
  if (-not $ok) { $rc = 1 }
  "{0,-7} {1} alive={2} title=[{3}] ms_to_window={4} end={5} {6}" -f $a[0], (Split-Path $exe -Leaf), $alive, $title, $ms, $how, $(if ($ok) { 'OK' } else { 'FAIL' })
}
exit $rc
