# Fixture generator 1/2: the extra direction inputs, written by the real MS Office 16 through COM (ASCII-only source).
#   Word  : order.{odt,rtf,html,txt,dot,md,pdf} from the Phase 0 synthetic hrm_order.docx (md is built from the paragraph
#           styles: Word has no md writer)
#   Excel : hrm_staff / hrm_timesheet as .xlsb and .ods; staff.csv = Excel's own CSV (UTF-8) of hrm_staff.xlsx, with this PC's list
#           separator and decimal (";" and "3500000,0" on ru-RU): the D6 input
# Synthetic only, never corpus/. Own Office instances (PID attributed, only those are killed).
param([Parameter(Mandatory)][string]$Dir)
$ErrorActionPreference = "Stop"
. "$PSScriptRoot\com.ps1"
$utf8 = New-Object Text.UTF8Encoding $false

$wd, $wdp = New-Office "Word.Application" "WINWORD"; $wd.Visible = $false; $wd.DisplayAlerts = 0
try {
    $src = "$Dir\hrm_order.docx"
    function Open-Src { $wd.Documents.Open($src, $false, $true) }
    foreach ($f in @(@("order.odt", 23), @("order.rtf", 6), @("order.dot", 1))) { $d = Open-Src; $d.SaveAs2("$Dir\$($f[0])", $f[1]); $d.Close(0) }
    $d = Open-Src; $d.WebOptions.Encoding = 65001; $d.SaveAs2("$Dir\order.html", 10); $d.Close(0)
    $d = Open-Src; $d.TextEncoding = 65001; $d.SaveAs2("$Dir\order.txt", 2); $d.Close(0)
    $d = Open-Src; $d.ExportAsFixedFormat("$Dir\order.pdf", 17); $d.Close(0)
    # md: headings by outline level, list items as "- ", tables as pipe tables
    $d = Open-Src
    $sb = New-Object Text.StringBuilder; $seen = @{}
    foreach ($p in $d.Paragraphs) {
        $r = $p.Range
        if ($r.Information(12)) {
            $t = $r.Tables.Item(1); $k = $t.Range.Start
            if ($seen.ContainsKey($k)) { continue }
            $seen[$k] = 1; $rows = @{}
            foreach ($c in $t.Range.Cells) {
                $txt = ($c.Range.Text -replace "[\r\a\f]", " ").Trim() -replace "\|", "/"
                if (-not $rows.ContainsKey($c.RowIndex)) { $rows[$c.RowIndex] = @{} }
                $rows[$c.RowIndex][$c.ColumnIndex] = $txt
            }
            $nc = ($rows.Values | ForEach-Object { $_.Count } | Measure-Object -Maximum).Maximum
            $first = $true
            foreach ($ri in ($rows.Keys | Sort-Object)) {
                $cells = for ($ci = 1; $ci -le $nc; $ci++) { if ($rows[$ri].ContainsKey($ci)) { $rows[$ri][$ci] } else { "" } }
                [void]$sb.AppendLine("| " + ($cells -join " | ") + " |")
                if ($first) { [void]$sb.AppendLine("|" + ((1..$nc | ForEach-Object { " --- |" }) -join "")); $first = $false }
            }
            [void]$sb.AppendLine(""); continue
        }
        $txt = ($r.Text -replace "[\r\a\f]", " ").Trim()
        if (-not $txt) { continue }
        $lvl = $p.OutlineLevel
        if ($lvl -ge 1 -and $lvl -le 6) { [void]$sb.AppendLine(("#" * $lvl) + " " + $txt) }
        elseif ($r.ListFormat.ListType -ne 0) { [void]$sb.AppendLine("- " + $txt) }
        else { [void]$sb.AppendLine($txt) }
        [void]$sb.AppendLine("")
    }
    $d.Close(0)
    [IO.File]::WriteAllText("$Dir\order.md", $sb.ToString(), $utf8)
} finally { Stop-Mine $wd $wdp }

$x, $xp = New-Office "Excel.Application" "EXCEL"; $x.Visible = $false; $x.DisplayAlerts = $false
try {
    foreach ($n in "hrm_staff", "hrm_timesheet") {
        foreach ($f in @(@("xlsb", 50), @("ods", 60))) {
            $b = $x.Workbooks.Open("$Dir\$n.xlsx", 0, $true); $b.SaveAs("$Dir\$n.$($f[0])", $f[1]); $b.Close($false)
        }
    }
    # xlCSVUTF8 = 62: the separator and the decimal are this PC's (";" and a comma on ru-RU), as a user's own export
    $b = $x.Workbooks.Open("$Dir\hrm_staff.xlsx", 0, $true); $b.SaveAs("$Dir\staff.csv", 62); $b.Close($false)
} finally { Stop-Mine $x $xp }
Get-ChildItem $Dir | Select-Object Name, Length | Format-Table -AutoSize | Out-String | Write-Host
