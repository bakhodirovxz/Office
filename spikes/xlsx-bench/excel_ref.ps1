# Reference measurement: open (and save-as) the synthetic workbooks in the locally installed
# Microsoft Excel via COM, recording wall time and peak working set of the EXCEL process.
# Read-only on inputs; outputs go to E:/office-target/xlsx-bench/excel/. Appends JSON lines to
# E:/office-target/xlsx-bench/excel_results.jsonl.
param([string[]]$Files = @('w10.xlsx','w40.xlsx','w80.xlsx','w120.xlsx'))

$dir = 'E:\office-target\xlsx-bench'
$out = Join-Path $dir 'excel'
New-Item -ItemType Directory -Force $out | Out-Null
$log = Join-Path $dir 'excel_results.jsonl'

foreach ($f in $Files) {
    $xl = New-Object -ComObject Excel.Application
    $xl.Visible = $false
    $xl.DisplayAlerts = $false
    $xl.AskToUpdateLinks = $false
    $xl.ScreenUpdating = $false
    $proc = Get-Process EXCEL | Sort-Object StartTime -Descending | Select-Object -First 1
    $rec = [ordered]@{ app = 'Excel ' + $xl.Version + ' (' + $(if ([Environment]::Is64BitProcess -and $proc.Path -notlike '*x86*') { '64-bit' } else { '32-bit' }) + ')'; file = $f }
    try {
        $sw = [Diagnostics.Stopwatch]::StartNew()
        $wb = $xl.Workbooks.Open((Join-Path $dir $f), 0, $true)
        $xl.CalculateFull()
        $rec.open_calc_s = [math]::Round($sw.Elapsed.TotalSeconds, 2)
        $rec.sheets = $wb.Worksheets.Count
        $proc.Refresh(); $rec.peak_mb_after_open = [math]::Round($proc.PeakWorkingSet64 / 1MB)
        $sw.Restart()
        $wb.SaveAs((Join-Path $out ($f -replace '\.xlsx$', '_excel.xlsx')), 51)
        $rec.save_s = [math]::Round($sw.Elapsed.TotalSeconds, 2)
        $wb.Close($false)
        $rec.ok = $true
    } catch {
        $rec.ok = $false
        $rec.error = $_.Exception.Message
    } finally {
        $proc.Refresh(); $rec.peak_mb = [math]::Round($proc.PeakWorkingSet64 / 1MB)
        $xl.Quit()
        [void][Runtime.InteropServices.Marshal]::ReleaseComObject($xl)
        [GC]::Collect(); [GC]::WaitForPendingFinalizers()
        Start-Sleep -Seconds 3
        if (-not $proc.HasExited) { Stop-Process -Id $proc.Id -Force }
    }
    ($rec | ConvertTo-Json -Compress) | Add-Content -Encoding utf8 $log
    $rec | ConvertTo-Json -Compress
}
