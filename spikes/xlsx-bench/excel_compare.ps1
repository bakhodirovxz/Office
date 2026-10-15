# Opens two workbooks read-only in the locally installed Excel (COM, invisible) and compares, sheet by
# sheet, every cell's formula as Excel reads it (Range.Formula over the used range) and every value
# after a full recalculation (Range.Value2). Used to check that gridcraft's shared-formula output
# means, to Excel, exactly what the unpatched build's per-cell output means. Writes nothing; prints
# one JSON line.
param([Parameter(Mandatory = $true)][string]$A, [Parameter(Mandatory = $true)][string]$B)

$xl = New-Object -ComObject Excel.Application
$xl.Visible = $false
$xl.DisplayAlerts = $false
$xl.AskToUpdateLinks = $false
$xl.ScreenUpdating = $false
$proc = Get-Process EXCEL | Sort-Object StartTime -Descending | Select-Object -First 1
$res = [ordered]@{ a = $A; b = $B; excel = $xl.Version }
try {
    $wa = $xl.Workbooks.Open($A, 0, $true)
    $wb = $xl.Workbooks.Open($B, 0, $true)
    $xl.CalculateFull()
    $res.a_name = $wa.Name; $res.b_name = $wb.Name
    $sheets = 0; $cells = 0; $formulas = 0; $fdiff = 0; $vdiff = 0; $first = @()
    for ($i = 1; $i -le $wa.Worksheets.Count; $i++) {
        $sa = $wa.Worksheets.Item($i); $sb = $wb.Worksheets.Item($i)
        $sheets++
        $addr = $sa.UsedRange.Address($false, $false)
        if ($sb.UsedRange.Address($false, $false) -ne $addr) { $fdiff++; $first += "$($sa.Name): used range $addr vs $($sb.UsedRange.Address($false, $false))"; continue }
        $fa = $sa.Range($addr).Formula; $fb = $sb.Range($addr).Formula
        $va = $sa.Range($addr).Value2; $vb = $sb.Range($addr).Value2
        if ($fa -isnot [Array]) { $fa = @($fa); $fb = @($fb); $va = @($va); $vb = @($vb) }
        $ea = $fa.GetEnumerator(); $eb = $fb.GetEnumerator(); $xa = $va.GetEnumerator(); $xb = $vb.GetEnumerator()
        while ($ea.MoveNext() -and $eb.MoveNext() -and $xa.MoveNext() -and $xb.MoveNext()) {
            $cells++
            $p = [string]$ea.Current; $q = [string]$eb.Current
            if ($p.StartsWith('=')) { $formulas++ }
            if ($p -ne $q) { $fdiff++; if ($first.Count -lt 5) { $first += "$($sa.Name): formula '$p' vs '$q'" } }
            if ([string]$xa.Current -ne [string]$xb.Current) { $vdiff++; if ($first.Count -lt 5) { $first += "$($sa.Name): value '$($xa.Current)' vs '$($xb.Current)'" } }
        }
    }
    $res.sheets = $sheets; $res.cells = $cells; $res.formulas = $formulas
    $res.formula_diffs = $fdiff; $res.value_diffs = $vdiff; $res.first_diffs = $first
    $wa.Close($false); $wb.Close($false)
    $res.ok = ($fdiff -eq 0 -and $vdiff -eq 0)
} catch {
    $res.ok = $false
    $res.error = $_.Exception.Message
} finally {
    $proc.Refresh(); $res.excel_peak_mb = [math]::Round($proc.PeakWorkingSet64 / 1MB)
    $xl.Quit()
    [void][Runtime.InteropServices.Marshal]::ReleaseComObject($xl)
    [GC]::Collect(); [GC]::WaitForPendingFinalizers()
    Start-Sleep -Seconds 3
    if (-not $proc.HasExited) { Stop-Process -Id $proc.Id -Force }
}
$res | ConvertTo-Json -Compress
