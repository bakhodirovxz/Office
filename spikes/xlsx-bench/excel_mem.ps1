# Steady-state memory of Microsoft Excel (COM, invisible), the reference for gui_mem.py.
# Mirrors excel_ref.ps1 (Visible/DisplayAlerts/AskToUpdateLinks/ScreenUpdating off, read-only open),
# but samples the counters the way gui_mem.py does, with the same sampler (memsample.py):
#   open -> sample (peak of the open) -> wait $Settle s -> sample (steady)
#   CalculateFull -> sample -> wait $Settle s -> sample (steady after recalc)
# Only the EXCEL process this script started is ever stopped: it is identified by diffing the
# EXCEL process list before and after creating the COM object. Before each run the script waits
# until no WINWORD/EXCEL/POWERPNT process runs (another agent drives Office over COM) and at
# least $NeedMB MiB of RAM are available.
# Appends JSON lines to E:/office-target/gridcraft-spike/r2/mem_results.jsonl.
param([string[]]$Files = @('w40.xlsx'), [string]$Label = 'excel', [int]$Settle = 10, [int]$NeedMB = 3072, [switch]$Visible)

# Capped as quiet.sh caps it: other agents keep 1-2 GB in use (the RAM guard is quiet.sh's job).
$NeedMB = [Math]::Min($NeedMB, 3500)
$dir = 'E:\office-target\xlsx-bench'
$log = 'E:\office-target\gridcraft-spike\r2\mem_results.jsonl'
New-Item -ItemType Directory -Force (Split-Path $log) | Out-Null
$py = 'E:\office-tools\venv-bench\Scripts\python.exe'
$sampler = Join-Path $PSScriptRoot 'memsample.py'

function Sample([int]$procId, [string]$tag) { (& $py -I $sampler $procId $tag) | ConvertFrom-Json }
function FreeMB { [int]((Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory / 1KB) }
function Heavy { @(Get-Process rustc, cargo, EXCEL, WINWORD, POWERPNT, gridcraft, gridcraft-cli -ErrorAction SilentlyContinue | ForEach-Object { $_.ProcessName } | Sort-Object -Unique) }

foreach ($f in $Files) {
    $waited = 0
    while ((Get-Process WINWORD, EXCEL, POWERPNT -ErrorAction SilentlyContinue) -or ((FreeMB) -lt $NeedMB)) {
        Start-Sleep -Seconds 15; $waited += 15
    }
    $before = @(Get-Process EXCEL -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
    $rec = [ordered]@{ label = $Label; app = 'excel'; visible = [bool]$Visible; file = $f; waited_s = $waited; free_before_mb = (FreeMB); others = (Heavy) }
    $xl = New-Object -ComObject Excel.Application
    $mine = @(Get-Process EXCEL -ErrorAction SilentlyContinue | Where-Object { $before -notcontains $_.Id })
    if ($mine.Count -ne 1) {
        $rec.ok = $false; $rec.error = "expected one new EXCEL process, found $($mine.Count)"
        $xl.Quit(); [void][Runtime.InteropServices.Marshal]::ReleaseComObject($xl)
        ($rec | ConvertTo-Json -Compress -Depth 4) | Add-Content -Encoding utf8 $log
        continue
    }
    $proc = $mine[0]
    $rec.exe = $proc.Path
    try {
        $xl.Visible = [bool]$Visible
        $xl.DisplayAlerts = $false
        $xl.AskToUpdateLinks = $false
        $xl.ScreenUpdating = [bool]$Visible
        $rec.version = $xl.Version
        $rec.base = Sample $proc.Id 'base'
        $sw = [Diagnostics.Stopwatch]::StartNew()
        $wb = $xl.Workbooks.Open((Join-Path $dir $f), 0, $true)
        $rec.open_s = [math]::Round($sw.Elapsed.TotalSeconds, 2)
        $rec.sheets = $wb.Worksheets.Count
        $rec.calc_state = $xl.CalculationState
        $rec.open_peak = Sample $proc.Id 'open'
        Start-Sleep -Seconds $Settle
        $rec.steady = Sample $proc.Id 'steady'
        $sw.Restart()
        $xl.CalculateFull()
        $rec.recalc_s = [math]::Round($sw.Elapsed.TotalSeconds, 2)
        $rec.recalc_peak = Sample $proc.Id 'recalc'
        Start-Sleep -Seconds $Settle
        $rec.steady_recalc = Sample $proc.Id 'steady_recalc'
        $wb.Close($false)
        $rec.ok = $true
    } catch {
        $rec.ok = $false
        $rec.error = $_.Exception.Message
    } finally {
        try { $xl.Quit() } catch {}
        [void][Runtime.InteropServices.Marshal]::ReleaseComObject($xl)
        [GC]::Collect(); [GC]::WaitForPendingFinalizers()
        Start-Sleep -Seconds 3
        $proc.Refresh()
        if (-not $proc.HasExited) { Stop-Process -Id $proc.Id -Force }
    }
    ($rec | ConvertTo-Json -Compress -Depth 4) | Add-Content -Encoding utf8 $log
    $rec | ConvertTo-Json -Compress -Depth 4
}
