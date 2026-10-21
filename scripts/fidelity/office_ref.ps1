# Office 16 COM reference conversions for the direction harness (ASCII-only source).
# -List : UTF-8 JSON array of {src, out, fmt}; fmt = pdf | docx | odt | rtf | html | txt | xlsx | csv | pptx | png
#         Word opens docx/doc/dot/odt/rtf/html/txt/md/PDF (PDF reflow), Excel xls/xlsx/xlsb/ods/csv, PowerPoint ppt/pptx.
#         csv is Excel's UTF-8 CSV with Local:=False (en-US separators, apples to apples with the Craft CSV).
#         png (PowerPoint) writes slides into the directory {out}.
# -Result : JSON array of {src, out, fmt, ok, error, ms}. Opening a file ALSO proves "Office can reopen it".
# -Progress : optional; one compact JSON line per finished item is appended at once, so the caller keeps them when it has to kill a hung Office.
# Own Office instance per app (PID attributed, written to $env:FID_PIDFILE for the caller); only that PID is killed.
param([Parameter(Mandatory)][ValidateSet("word", "excel", "ppt")][string]$App,
      [Parameter(Mandatory)][string]$List, [Parameter(Mandatory)][string]$Result, [string]$Progress = "")
$ErrorActionPreference = "Stop"
. "$PSScriptRoot\com.ps1"
$items = [IO.File]::ReadAllText($List, [Text.Encoding]::UTF8) | ConvertFrom-Json
if ($items -isnot [array]) { $items = @($items) }
$miss = [Type]::Missing
$proc = @{ word = "WINWORD"; excel = "EXCEL"; ppt = "POWERPNT" }[$App]
$prog = @{ word = "Word.Application"; excel = "Excel.Application"; ppt = "PowerPoint.Application" }[$App]
$results = New-Object System.Collections.ArrayList
$oa, $mypid = New-Office $prog $proc
# Word opening a PDF asks "Word will now convert your PDF" and hangs a headless run; the documented switch is a per-user
# option. Set it only for this run and put the previous state back afterwards.
$rk = "HKCU:\Software\Microsoft\Office.0\Word\Options"; $rkOld = $null; $rkSet = $false
if ($App -eq "word" -and ($items | Where-Object { $_.src -like "*.pdf" })) {
    if (-not (Test-Path $rk)) { New-Item $rk -Force | Out-Null }
    $rkOld = (Get-ItemProperty $rk -Name DisableConvertPdfWarning -ErrorAction SilentlyContinue).DisableConvertPdfWarning
    Set-ItemProperty $rk -Name DisableConvertPdfWarning -Value 1 -Type DWord; $rkSet = $true
}
try {
    if ($App -ne "ppt") { $oa.Visible = $false }
    if ($App -eq "word") { $oa.DisplayAlerts = 0 }
    if ($App -eq "excel") { $oa.DisplayAlerts = $false; $oa.AskToUpdateLinks = $false }
    foreach ($it in $items) {
        $r = [ordered]@{ src = $it.src; out = $it.out; fmt = $it.fmt; ok = $false; error = $null; ms = 0 }
        $sw = [Diagnostics.Stopwatch]::StartNew(); $obj = $null
        try {
            $ext = [IO.Path]::GetExtension($it.src).ToLower()
            if (Test-Path $it.out) { Remove-Item $it.out -Recurse -Force }
            switch ($App) {
                "word" {
                    if ($ext -eq ".txt" -or $ext -eq ".md") { $obj = $oa.Documents.Open($it.src, $false, $true, $false, $miss, $miss, $miss, $miss, $miss, $miss, 65001) }
                    else { $obj = $oa.Documents.Open($it.src, $false, $true, $false) }
                    switch ($it.fmt) {
                        "pdf" { $obj.ExportAsFixedFormat($it.out, 17) }
                        "docx" { $obj.SaveAs2($it.out, 16) }
                        "odt" { $obj.SaveAs2($it.out, 23) }
                        "rtf" { $obj.SaveAs2($it.out, 6) }
                        "html" { $obj.WebOptions.Encoding = 65001; $obj.SaveAs2($it.out, 10) }
                        "txt" { $obj.TextEncoding = 65001; $obj.SaveAs2($it.out, 2) }
                        default { throw "word cannot write $($it.fmt)" }
                    }
                }
                "excel" {
                    $obj = $oa.Workbooks.Open($it.src, 0, $true)
                    switch ($it.fmt) {
                        "pdf" { $obj.ExportAsFixedFormat(0, $it.out) }
                        "xlsx" { $obj.SaveAs($it.out, 51) }
                        "csv" { $obj.SaveAs($it.out, 62, $miss, $miss, $miss, $miss, $miss, $miss, $miss, $miss, $miss, $false) }
                        default { throw "excel cannot write $($it.fmt)" }
                    }
                }
                "ppt" {
                    $obj = $oa.Presentations.Open($it.src, -1, 0, 0)
                    switch ($it.fmt) {
                        "pdf" { $obj.SaveAs($it.out, 32) }
                        "pptx" { $obj.SaveAs($it.out, 24) }
                        "png" { New-Item -ItemType Directory -Force $it.out | Out-Null; $obj.Export($it.out, "PNG", 1920, 1080) }
                        default { throw "powerpoint cannot write $($it.fmt)" }
                    }
                }
            }
            $r.ok = Test-Path $it.out
        } catch { $r.error = $_.Exception.Message }
        finally {
            if ($obj -ne $null) { try { if ($App -eq "word") { $obj.Close(0) } elseif ($App -eq "excel") { $obj.Close($false) } else { $obj.Close() } } catch {} }
        }
        $r.ms = $sw.ElapsedMilliseconds
        [void]$results.Add($r)
    }
} finally {
    Stop-Mine $oa $mypid
    if ($rkSet) { if ($null -eq $rkOld) { Remove-ItemProperty $rk -Name DisableConvertPdfWarning -ErrorAction SilentlyContinue } else { Set-ItemProperty $rk -Name DisableConvertPdfWarning -Value $rkOld -Type DWord } }
    [IO.File]::WriteAllText($Result, (ConvertTo-Json -InputObject @($results) -Depth 4), (New-Object Text.UTF8Encoding $false))
}
