# Shared COM helpers (dot-source). Only the Office PID this script started is ever killed; other agents use COM too.
function New-Office($prog, $proc) {
    $before = @(Get-Process $proc -ErrorAction SilentlyContinue | ForEach-Object Id)
    $app = New-Object -ComObject $prog; Start-Sleep 2
    $new = @(Get-Process $proc -ErrorAction SilentlyContinue | Where-Object { $before -notcontains $_.Id } | ForEach-Object Id)
    if ($new.Count -ne 1) { try { $app.Quit() } catch {}; throw "cannot attribute $proc to this script (new PIDs: $($new -join ',')); not killing anything" }
    if ($env:FID_PIDFILE) { Set-Content $env:FID_PIDFILE $new[0] }   # lets the python caller kill exactly this PID on a timeout
    return @($app, $new[0])
}
function Stop-Mine($app, $mypid) {
    try { $app.DisplayAlerts = 0 } catch {}
    try { $app.Quit() } catch {}
    try { [void][Runtime.InteropServices.Marshal]::ReleaseComObject($app) } catch {}
    [GC]::Collect(); [GC]::WaitForPendingFinalizers(); Start-Sleep 2
    if ($mypid -gt 0) { Get-Process -Id $mypid -ErrorAction SilentlyContinue | Stop-Process -Force }
}
