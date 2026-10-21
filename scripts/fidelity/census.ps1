# WP1 direction census (throwaway). Runs every PLAN section 1 direction once with the raw domain CLIs and records
# exit code, wall time, peak working set and stderr warnings. Synthetic inputs only; never corpus/.
#   -Phase prep : build the inputs (Office 16 COM for .rtf/.odt/.dot/.xlsb/.ods/Word-made PDFs; python for big xlsx)
#   -Phase run  : run the census, write $W\results.json
#   -Phase all  : both (default)
# Office rule: only the WINWORD/EXCEL PIDs this script started (found through the window handle) are ever killed.
# Everything lands under E:\office-target\p3\census (not in git).
param([ValidateSet("prep", "run", "all")][string]$Phase = "all",
      [string]$W = "E:\office-target\p3\census",
      [string]$Bin = "E:\office-target\p1",
      [string]$Syn = "E:\office-target\fidelity\synthetic")
$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$DOCS = "$Bin\docs\release\docs-cli.exe"; $SHEETS = "$Bin\sheets\release\sheets-cli.exe"
$SLIDES = "$Bin\slides\release\slides-cli.exe"; $PDF = "$Bin\pdf\release\pdf-cli.exe"
foreach ($b in $DOCS, $SHEETS, $SLIDES, $PDF) { if (-not (Test-Path $b)) { throw "missing binary $b (not building, report it)" } }
$IN = "$W\in"; $OUT = "$W\out"
New-Item -ItemType Directory -Force $IN, $OUT, "$W\helpers" | Out-Null
$env:PYTHONIOENCODING = "utf-8"; $env:WORDCRAFT_REGION = "en-US"   # same paper on every machine; the converter will pin the locale too (F7)

# ---------------------------------------------------------------- Office COM (own instances only)
function New-Office($prog, $proc) {   # starts our own instance; its PID = the one process that was not there before
    $before = @(Get-Process $proc -ErrorAction SilentlyContinue | ForEach-Object Id)
    $app = New-Object -ComObject $prog; Start-Sleep 2
    $new = @(Get-Process $proc -ErrorAction SilentlyContinue | Where-Object { $before -notcontains $_.Id } | ForEach-Object Id)
    if ($new.Count -ne 1) { try { $app.Quit() } catch {}; throw "cannot attribute $proc to this script (new PIDs: $($new -join ',')); not killing anything" }
    return @($app, $new[0])
}
function Stop-Mine($app, $mypid) {
    try { $app.DisplayAlerts = 0 } catch {}
    try { $app.Quit() } catch {}
    try { [void][Runtime.InteropServices.Marshal]::ReleaseComObject($app) } catch {}
    [GC]::Collect(); [GC]::WaitForPendingFinalizers(); Start-Sleep 2
    if ($mypid -gt 0) { Get-Process -Id $mypid -ErrorAction SilentlyContinue | Stop-Process -Force }
}

function Prep {
    foreach ($f in "hrm_order.docx", "hrm_order.doc", "hrm_letter.docx", "hrm_staff.xlsx", "hrm_staff.xls", "hrm_timesheet.xlsx",
                   "hrm_timesheet.xls", "hrm_deck.pptx", "hrm_deck.ppt") { Copy-Item "$Syn\$f" "$IN\$f" -Force }
    $fx = "$repo\domains"
    Copy-Item "$fx\docs\crates\docs-translit\tests\fixtures\uz108-latn.docx", "$fx\docs\crates\docs-translit\tests\fixtures\uz108-cyrl.docx" $IN -Force
    Copy-Item "$fx\sheets\tests\translit-fixtures\xlsx\02-table-rich.xlsx" $IN -Force
    Copy-Item "$fx\slides\tests\translit-fixtures\pptx\01-shapes-notes.pptx", "$fx\slides\tests\translit-fixtures\pptx\02-languages.pptx" $IN -Force

    $T = [IO.File]::ReadAllText("$repo\scripts\fidelity\content.json", [Text.Encoding]::UTF8) | ConvertFrom-Json
    # ---- Word
    $wd, $wdp = New-Office "Word.Application" "WINWORD"; $wd.Visible = $false; $wd.DisplayAlerts = 0
    try {
        $o = $wd.Documents.Open("$IN\hrm_order.docx", $false, $true)   # read-only
        $o.SaveAs2("$IN\order.rtf", 6); $o.Close(0)
        $o = $wd.Documents.Open("$IN\hrm_order.docx", $false, $true); $o.SaveAs2("$IN\order.odt", 23); $o.Close(0)
        $o = $wd.Documents.Open("$IN\hrm_order.docx", $false, $true); $o.SaveAs2("$IN\order.dot", 1); $o.Close(0)
        foreach ($v in @(@("latn", $T.title_lat, $T.place_lat, $T.head_lat, $T.p_lat, $T.list_cyr),
                         @("cyrl", $T.title_cyr, $T.place_cyr, $T.head_cyr, $T.p_cyr, $T.list_cyr))) {
            $d = $wd.Documents.Add()
            $s = $wd.Selection
            $s.Font.Name = "Times New Roman"; $s.Font.Size = 14; $s.Font.Bold = 1; $s.TypeText($v[1]); $s.TypeParagraph()
            $s.Font.Bold = 0; $s.Font.Size = 12; $s.TypeText($v[2]); $s.TypeParagraph(); $s.Font.Bold = 1; $s.TypeText($v[3]); $s.TypeParagraph(); $s.Font.Bold = 0
            for ($k = 0; $k -lt 5; $k++) { foreach ($p in $v[4]) { $s.TypeText($p); $s.TypeParagraph() } }
            $tb = $d.Tables.Add($s.Range, 4, 3); $tb.Borders.Enable = $true
            for ($r = 1; $r -le 4; $r++) { for ($c = 1; $c -le 3; $c++) { $tb.Cell($r, $c).Range.Text = $v[4][($r + $c) % 3].Substring(0, 24) } }
            $d.SaveAs2("$IN\uz_$($v[0]).docx", 16); $d.ExportAsFixedFormat("$IN\uz_$($v[0]).pdf", 17); $d.Close(0)
        }
    } finally { Stop-Mine $wd $wdp }
    # ---- Excel
    $x, $xp = New-Office "Excel.Application" "EXCEL"; $x.Visible = $false; $x.DisplayAlerts = $false
    try {
        foreach ($fmt in @(@("hrm_staff.xlsb", 50), @("hrm_staff.ods", 60))) {
            $b = $x.Workbooks.Open("$IN\hrm_staff.xlsx", 0, $true); $b.SaveAs("$IN\$($fmt[0])", $fmt[1]); $b.Close($false)
        }
    } finally { Stop-Mine $x $xp }
    # ---- big workbooks (scaling for the per-MB timeout term)
    $py = @'
import sys, random
from openpyxl import Workbook
random.seed(1)
for name, rows in (("big5k", 5000), ("big50k", 50000), ("big300k", 300000)):
    wb = Workbook(write_only=True); ws = wb.create_sheet("Data")
    ws.append(["ID", "Name", "Dept", "Salary", "Date", "Note", "Score", "Flag"])
    for i in range(rows):
        ws.append([i, "Xodim %d" % i, "Bo'lim %d" % (i % 40), random.randint(1000000, 9000000), "2026-01-%02d" % (1 + i % 28),
                   "Izoh matni %d" % (i % 977), random.random() * 100, i % 2 == 0])
    wb.save(sys.argv[1] + "\\%s.xlsx" % name)
'@
    $py += "`nfrom PIL import Image`nImage.open(r'$Syn\sample.png').convert('RGB').resize((1240, 1754)).save(sys.argv[1] + '\scan.pdf')`n"
    Set-Content "$W\helpers\bigxlsx.py" $py -Encoding ASCII
    & python "$W\helpers\bigxlsx.py" $IN
    Get-ChildItem $IN | Select-Object Name, Length | Format-Table -AutoSize | Out-String | Write-Host
}

# ---------------------------------------------------------------- runner
$results = New-Object System.Collections.ArrayList
function QArg([string]$s) { if ($s -match '[\s"]' -or $s -eq "") { '"' + ($s -replace '(\\*)"', '$1$1\"') + '"' } else { $s } }
function Magic($path) {
    if (-not (Test-Path $path -PathType Leaf)) { return $null }
    $b = [IO.File]::ReadAllBytes($path); if ($b.Length -eq 0) { return "empty" }
    $h = [Text.Encoding]::ASCII.GetString($b, 0, [Math]::Min(8, $b.Length))
    if ($h.StartsWith("PK")) { "zip" } elseif ($h.StartsWith("%PDF")) { "pdf" } elseif ($h.Substring(1).StartsWith("PNG")) { "png" }
    elseif ($h.StartsWith("{\rtf")) { "rtf" } else { "other" }
}
# One process, polled every 20 ms for peak working set. $row = PLAN section 1 row label.
function Step([string]$id, [string]$row, [string]$exe, [string[]]$argv, [string]$out, [int]$timeout = 600, [string]$inFile = "") {
    $psi = New-Object Diagnostics.ProcessStartInfo
    $psi.FileName = $exe; $psi.Arguments = ($argv | ForEach-Object { QArg $_ }) -join " "
    $psi.RedirectStandardOutput = $true; $psi.RedirectStandardError = $true; $psi.UseShellExecute = $false; $psi.CreateNoWindow = $true
    $psi.WorkingDirectory = $W
    if ($out -and (Test-Path $out)) { Remove-Item $out -Recurse -Force }
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $p = [Diagnostics.Process]::Start($psi)
    $so = $p.StandardOutput.ReadToEndAsync(); $se = $p.StandardError.ReadToEndAsync()
    $peak = 0L; $killed = $false
    while (-not $p.WaitForExit(20)) {
        try { $p.Refresh(); if ($p.PeakWorkingSet64 -gt $peak) { $peak = $p.PeakWorkingSet64 } } catch {}
        if ($sw.Elapsed.TotalSeconds -gt $timeout) { $p.Kill(); $killed = $true; $p.WaitForExit() }
    }
    $sw.Stop(); [void]$so.Wait(); [void]$se.Wait()
    $rc = if ($killed) { 124 } else { $p.ExitCode }
    $err = $se.Result.Trim(); $outtxt = $so.Result.Trim()
    $warn = @(($err + "`n" + $outtxt) -split "`r?`n" | Where-Object { $_ -match 'warn|ignored|not imported|lost|unsupported|skipp' } | Select-Object -First 3)
    $magic = $null; $bytes = 0
    if ($out) {
        if (Test-Path $out -PathType Container) { $f = @(Get-ChildItem $out -File); $bytes = ($f | Measure-Object Length -Sum).Sum; $magic = "dir:$($f.Count)" }
        elseif (Test-Path $out) { $bytes = (Get-Item $out).Length; $magic = Magic $out }
    }
    $pages = $null
    if ($out -like "*.pdf" -and (Test-Path $out -PathType Leaf) -and $magic -eq "pdf") { try { $pages = ((& $PDF info $out | Out-String) | ConvertFrom-Json).pages } catch {} }
    $status = if ($rc -ne 0) { "FAIL" } elseif ($out -and ($null -eq $magic -or $magic -eq "empty")) { "FAIL" } else { "PASS" }
    $inMB = if ($inFile -and (Test-Path $inFile)) { [Math]::Round((Get-Item $inFile).Length / 1MB, 3) } else { $null }
    $r = [ordered]@{ id = $id; row = $row; tool = (Split-Path $exe -Leaf); args = $psi.Arguments; rc = $rc; status = $status
        sec = [Math]::Round($sw.Elapsed.TotalSeconds, 3); peakMB = [Math]::Round($peak / 1MB, 1); inMB = $inMB
        outBytes = $bytes; magic = $magic; pages = $pages; stdoutChars = $outtxt.Length; warn = $warn; err1 = (($err -split "`r?`n")[0]) }
    [void]$results.Add([pscustomobject]$r)
    "{0,-26} {1,-4} rc={2,-3} {3,8:N2}s {4,7:N0}MB {5}" -f $id, $status, $rc, $r.sec, $r.peakMB, $r.err1 | Write-Host
}
function ScriptFile($name, $steps) { $f = "$W\helpers\$name.json"; [IO.File]::WriteAllText($f, ($steps | ConvertTo-Json -Depth 6 -Compress), (New-Object Text.UTF8Encoding $false)); return $f }
$J = '{"scope":"document"}'

function Run-Census {
    $i = $IN; $o = $OUT
    # ---- 1.1 Docs
    foreach ($e in "odt", "rtf", "html", "md", "txt") {
        Step "docs.docx2$e" "1.1 docx -> odt/rtf/html/md/txt" $DOCS @("convert", "$i\hrm_order.docx", "$o\order.$e") "$o\order.$e" -inFile "$i\hrm_order.docx" }
    foreach ($e in "odt", "rtf") {   # Word-made inputs (the realistic ones)
        Step "docs.w$e`2docx" "1.1 odt/rtf/html/md/txt -> docx" $DOCS @("convert", "$i\order.$e", "$o\from_w$e.docx") "$o\from_w$e.docx" -inFile "$i\order.$e" }
    foreach ($e in "html", "md", "txt") {
        Step "docs.$e`2docx" "1.1 odt/rtf/html/md/txt -> docx" $DOCS @("convert", "$o\order.$e", "$o\from_$e.docx") "$o\from_$e.docx" -inFile "$o\order.$e" }
    Step "docs.doc2docx" "1.1 .doc/.dot -> docx" $DOCS @("convert", "$i\hrm_order.doc", "$o\from_doc.docx") "$o\from_doc.docx" -inFile "$i\hrm_order.doc"
    Step "docs.dot2docx" "1.1 .doc/.dot -> docx" $DOCS @("convert", "$i\order.dot", "$o\from_dot.docx") "$o\from_dot.docx" -inFile "$i\order.dot"
    Step "docs.letter.doc2docx" "1.1 .doc/.dot -> docx" $DOCS @("convert", "$Syn\hrm_letter.doc", "$o\from_letter_doc.docx") "$o\from_letter_doc.docx" -inFile "$Syn\hrm_letter.doc"
    foreach ($s in @(@("docx", "$i\hrm_order.docx"), @("doc", "$i\hrm_order.doc"), @("odt", "$i\order.odt"), @("rtf", "$i\order.rtf"), @("md", "$o\order.md"))) {
        Step "docs.$($s[0])2pdf" "1.1 docx/doc/odt/rtf/... -> pdf" $DOCS @("convert", $s[1], "$o\order_$($s[0]).pdf") "$o\order_$($s[0]).pdf" -inFile $s[1] }
    Step "docs.uz108latn2pdf" "1.1 docx/doc/odt/rtf/... -> pdf" $DOCS @("convert", "$i\uz108-latn.docx", "$o\uz108.pdf") "$o\uz108.pdf" -inFile "$i\uz108-latn.docx"
    # docx -> png: wordcraft's own page 1, each page via render, and the pdf route
    Step "docs.docx2png.convert" "1.1 docx -> png" $DOCS @("convert", "$i\hrm_order.docx", "$o\order_c.png") "$o\order_c.png" -inFile "$i\hrm_order.docx"
    for ($n = 1; $n -le 4; $n++) { Step "docs.docx2png.render.p$n" "1.1 docx -> png" $DOCS @("render", "$i\hrm_order.docx", "$o\order_r$n.png", "--page", "$n") "$o\order_r$n.png" -inFile "$i\hrm_order.docx" }
    $steps = @(@{ tool = "doc_open"; args = @{ path = "out/order_docx.pdf" } }) + (1..4 | ForEach-Object { @{ tool = "page_render"; args = @{ doc = 1; page = $_; dpi = 144 }; out = "out/order_pdfroute_p$_.png" } })
    Step "docs.docx2png.pdfroute" "1.1 docx -> png" $PDF @("run", "--script", (ScriptFile "pngroute" $steps), "--root", $W) "$o\order_pdfroute_p4.png"
    # translit (Latin -> Cyrillic)
    foreach ($e in "docx", "odt", "rtf") {
        Step "docs.translit.uz108.$e" "1.1 docx (+translit) -> docx/odt/rtf" $DOCS @("run", "--file", "$i\uz108-latn.docx", "--cmd", "tools.translitCyrl=$J", "--save", "$o\uz108_cyr.$e") "$o\uz108_cyr.$e" -inFile "$i\uz108-latn.docx" }
    Step "docs.translit.uz_latn" "1.1 docx (+translit) -> docx/odt/rtf" $DOCS @("run", "--file", "$i\uz_latn.docx", "--cmd", "tools.translitCyrl=$J", "--save", "$o\uz_latn_cyr.docx") "$o\uz_latn_cyr.docx" -inFile "$i\uz_latn.docx"

    # ---- 1.2 Sheets
    foreach ($s in @(@("xls", "hrm_staff.xls"), @("xlsb", "hrm_staff.xlsb"), @("ods", "hrm_staff.ods"), @("timesheet.xls", "hrm_timesheet.xls"))) {
        Step "sheets.$($s[0])2xlsx" "1.2 xls/xlsb/ods -> xlsx" $SHEETS @("convert", "$i\$($s[1])", "$o\from_$($s[0]).xlsx") "$o\from_$($s[0]).xlsx" -inFile "$i\$($s[1])" }
    foreach ($s in @(@("xlsx", "hrm_staff.xlsx"), @("timesheet", "hrm_timesheet.xlsx"), @("xls", "hrm_staff.xls"), @("tablerich", "02-table-rich.xlsx"))) {
        Step "sheets.$($s[0])2pdf" "1.2 xlsx (any input) -> pdf" $SHEETS @("run", "--in", "$i\$($s[1])", "--cmd", ('file.exportPdf={"path":"' + ("$o\sheet_$($s[0]).pdf" -replace '\\', '/') + '","sheets":"all"}')) "$o\sheet_$($s[0]).pdf" -inFile "$i\$($s[1])" }
    foreach ($e in "csv", "tsv", "json", "html") {
        Step "sheets.xlsx2$e" "1.2 xlsx -> csv" $SHEETS @("convert", "$i\hrm_staff.xlsx", "$o\staff.$e") "$o\staff.$e" -inFile "$i\hrm_staff.xlsx" }
    Step "sheets.xlsx2csv.sheet" "1.2 xlsx -> csv" $SHEETS @("convert", "$i\hrm_timesheet.xlsx", "$o\ts_sheet1.csv", "--sheet", "Tabel") "$o\ts_sheet1.csv" -inFile "$i\hrm_timesheet.xlsx"
    Step "sheets.translit.xlsx" "1.2 xlsx -> xlsx (+translit) / pdf" $SHEETS @("run", "--in", "$i\hrm_staff.xlsx", "--cmd", 'tools.translitCyrl={"scope":"workbook"}', "--out", "$o\staff_cyr.xlsx") "$o\staff_cyr.xlsx" -inFile "$i\hrm_staff.xlsx"
    Step "sheets.translit.tablerich" "1.2 xlsx -> xlsx (+translit) / pdf" $SHEETS @("run", "--in", "$i\02-table-rich.xlsx", "--cmd", 'tools.translitCyrl={"scope":"workbook"}', "--out", "$o\tr_cyr.xlsx") "$o\tr_cyr.xlsx" -inFile "$i\02-table-rich.xlsx"
    Step "sheets.translit.pdf" "1.2 xlsx -> xlsx (+translit) / pdf" $SHEETS @("run", "--in", "$i\hrm_staff.xlsx", "--cmd", 'tools.translitCyrl={"scope":"workbook"}', "--cmd", ('file.exportPdf={"path":"' + ("$o\staff_cyr.pdf" -replace '\\', '/') + '","sheets":"all"}')) "$o\staff_cyr.pdf" -inFile "$i\hrm_staff.xlsx"
    foreach ($b in "big5k", "big50k", "big300k") {
        Step "sheets.$b.xlsx2xlsx" "1.2 large workbooks" $SHEETS @("convert", "$i\$b.xlsx", "$o\$b.out.xlsx") "$o\$b.out.xlsx" -inFile "$i\$b.xlsx"
        Step "sheets.$b.xlsx2csv" "1.2 large workbooks" $SHEETS @("convert", "$i\$b.xlsx", "$o\$b.csv") "$o\$b.csv" -inFile "$i\$b.xlsx" }
    Step "sheets.big5k.xlsx2pdf" "1.2 large workbooks" $SHEETS @("run", "--in", "$i\big5k.xlsx", "--cmd", ('file.exportPdf={"path":"' + ("$o\big5k.pdf" -replace '\\', '/') + '","sheets":"all"}')) "$o\big5k.pdf" -inFile "$i\big5k.xlsx"

    # ---- 1.3 Slides
    Step "slides.pptx2pdf" "1.3 pptx -> pdf" $SLIDES @("convert", "$i\hrm_deck.pptx", "$o\deck.pdf") "$o\deck.pdf" -inFile "$i\hrm_deck.pptx"
    foreach ($f in "01-shapes-notes", "02-languages") { Step "slides.$f.2pdf" "1.3 pptx -> pdf" $SLIDES @("convert", "$i\$f.pptx", "$o\$f.pdf") "$o\$f.pdf" -inFile "$i\$f.pptx" }
    Step "slides.pptx2png" "1.3 pptx -> png" $SLIDES @("render", "$i\hrm_deck.pptx", "--all", "$o\deck_png") "$o\deck_png" -inFile "$i\hrm_deck.pptx"
    Step "slides.translit.pptx" "1.3 pptx (+translit) -> pptx/pdf" $SLIDES @("run", "--in", "$i\hrm_deck.pptx", "--cmd", 'tools.translitCyrl={"scope":"deck"}', "--save", "$o\deck_cyr.pptx") "$o\deck_cyr.pptx" -inFile "$i\hrm_deck.pptx"
    Step "slides.translit.pdf" "1.3 pptx (+translit) -> pptx/pdf" $SLIDES @("run", "--in", "$i\02-languages.pptx", "--cmd", 'tools.translitCyrl={"scope":"deck"}', "--save", "$o\lang_cyr.pdf") "$o\lang_cyr.pdf" -inFile "$i\02-languages.pptx"
    foreach ($e in "pptx", "pdf") { Step "slides.ppt2$e" "1.3 .ppt -> pptx/pdf/png" $SLIDES @("convert", "$i\hrm_deck.ppt", "$o\fromppt.$e") "$o\fromppt.$e" -inFile "$i\hrm_deck.ppt" }
    Step "slides.ppt2png" "1.3 .ppt -> pptx/pdf/png" $SLIDES @("render", "$i\hrm_deck.ppt", "--all", "$o\ppt_png") "$o\ppt_png" -inFile "$i\hrm_deck.ppt"

    # ---- 1.4 PDF (inputs: Word-made PDFs, the 108-page pdf, the 4-page order pdf)
    Step "pdf.combine" "1.4 combine" $PDF @("combine", "$i\uz_latn.pdf", "$i\uz_cyrl.pdf", "$o\order_docx.pdf", "--out", "$o\combined.pdf") "$o\combined.pdf"
    Step "pdf.split.every" "1.4 split" $PDF @("split", "$o\uz108.pdf", "--every", "10", "--out-dir", "$o\split") "$o\split" -inFile "$o\uz108.pdf"
    Step "pdf.split.before" "1.4 split" $PDF @("split", "$o\order_docx.pdf", "--before", "3", "--out-dir", "$o\split2") "$o\split2" -inFile "$o\order_docx.pdf"
    Step "pdf.extract" "1.4 extract pages" $PDF @("extract", "$o\uz108.pdf", "--pages", "1,3,5-8", "--out", "$o\extract.pdf") "$o\extract.pdf" -inFile "$o\uz108.pdf"
    foreach ($s in @(@("latn", "$i\uz_latn.pdf"), @("cyrl", "$i\uz_cyrl.pdf"), @("uz108", "$o\uz108.pdf"))) {
        Step "pdf.text.$($s[0])" "1.4 text" $PDF @("text", $s[1]) "" -inFile $s[1] }
    Step "pdf.text.scan" "1.4 text" $PDF @("text", "$i\scan.pdf") "" -inFile "$i\scan.pdf"   # image-only pdf: an empty result must become a converter warning
    Step "pdf.render.p1" "1.4 pdf -> png" $PDF @("render", "$i\uz_cyrl.pdf", "--page", "1", "--dpi", "144", "--out", "$o\cyrl_p1.png") "$o\cyrl_p1.png" -inFile "$i\uz_cyrl.pdf"
    $steps = @(@{ tool = "doc_open"; args = @{ path = "out/uz108.pdf" } }) + (1..108 | ForEach-Object { @{ tool = "page_render"; args = @{ doc = 1; page = $_; dpi = 96 }; out = ("out/uz108_png/p{0:000}.png" -f $_) } })
    New-Item -ItemType Directory -Force "$o\uz108_png" | Out-Null
    Step "pdf.render.script108" "1.4 pdf -> png" $PDF @("run", "--script", (ScriptFile "render108" $steps), "--root", $W) "$o\uz108_png" -inFile "$o\uz108.pdf"
    # pdf -> docx native (F3), 2 Word-made PDFs, then (+translit) chain on the Latin result
    foreach ($v in "latn", "cyrl") {
        $steps = @(@{ tool = "doc_open"; args = @{ path = "in/uz_$v.pdf" } }; @{ tool = "doc_export_office"; args = @{ doc = 1; path = "out/uz_${v}_native.docx" } })
        Step "pdf.native2docx.$v" "1.4 pdf -> docx" $PDF @("run", "--script", (ScriptFile "native_$v" $steps), "--root", $W) "$o\uz_${v}_native.docx" -inFile "$i\uz_$v.pdf" }
    Step "pdf.native2docx.translit" "1.4 pdf -> docx (+translit)" $DOCS @("run", "--file", "$o\uz_latn_native.docx", "--cmd", "tools.translitCyrl=$J", "--save", "$o\uz_latn_native_cyr.docx") "$o\uz_latn_native_cyr.docx" -inFile "$o\uz_latn_native.docx"
    foreach ($e in "pptx", "html", "rtf") {
        $steps = @(@{ tool = "doc_open"; args = @{ path = "in/uz_cyrl.pdf" } }; @{ tool = "doc_export_office"; args = @{ doc = 1; path = "out/uz_cyrl_native.$e" } })
        Step "pdf.native2$e" "1.4 pdf -> docx (other targets, F3)" $PDF @("run", "--script", (ScriptFile "native_$e" $steps), "--root", $W) "$o\uz_cyrl_native.$e" -inFile "$i\uz_cyrl.pdf" }
}

# ---------------------------------------------------------------- analysis helpers (python: numpy + pillow, like scripts/fidelity)
$an = @'
import sys, json, re, difflib, subprocess
import numpy as np
from PIL import Image, ImageFilter
def diff(a, b):
    A = Image.open(a).convert("L"); B = Image.open(b).convert("L"); same = A.size == B.size
    if not same: B = B.resize(A.size)
    x = np.asarray(A.filter(ImageFilter.GaussianBlur(1)), dtype=np.int16); y = np.asarray(B.filter(ImageFilter.GaussianBlur(1)), dtype=np.int16)
    return {"pct": round(float((np.abs(x - y) > 48).mean() * 100), 3), "sizeA": A.size, "sizeB": (Image.open(b).size), "sameSize": same}
def words(t): return re.findall(r"\w+", t.lower())
def sim(a, b): return round(difflib.SequenceMatcher(None, words(a), words(b), autojunk=False).ratio(), 4)
def run(*c): return subprocess.run(c, capture_output=True, text=True, encoding="utf-8").stdout
cmd = sys.argv[1]
if cmd == "png":   # F4: wordcraft render (scale 2) vs pdf route (144 dpi)
    W = sys.argv[2]; out = []
    for n in range(1, 5):
        out.append({"page": n, **diff(f"{W}/out/order_r{n}.png", f"{W}/out/order_pdfroute_p{n}.png")})
    import hashlib
    h = lambda f: hashlib.sha256(open(f, "rb").read()).digest()
    for r in out: r["identical_bytes"] = h(f"{W}/out/order_r{r['page']}.png") == h(f"{W}/out/order_pdfroute_p{r['page']}.png")
    cv = diff(f"{W}/out/order_r1.png", f"{W}/out/order_c.png"); print(json.dumps({"pages": out, "convert_vs_render_p1": cv}))
if cmd == "native":  # F3: structure + text of the native pdf->docx
    W, docs, pdf, v = sys.argv[2:6]; src = f"{W}/in/uz_{v}.pdf"; dx = f"{W}/out/uz_{v}_native.docx"
    st = run(pdf, "text", src); dt = run(docs, "text", dx); info = json.loads(run(docs, "info", dx)); orig = json.loads(run(docs, "info", f"{W}/in/uz_{v}.docx"))
    cyr = lambda s: len(re.findall(r"[\u0400-\u04FF]", s))
    run(docs, "convert", dx, f"{W}/out/uz_{v}_native_rt.pdf")
    rt = json.loads(run(pdf, "info", f"{W}/out/uz_{v}_native_rt.pdf")); op = json.loads(run(pdf, "info", src))
    subprocess.run([pdf, "render", src, "--page", "1", "--dpi", "96", "--out", f"{W}/out/n_{v}_a.png"], capture_output=True)
    subprocess.run([pdf, "render", f"{W}/out/uz_{v}_native_rt.pdf", "--page", "1", "--dpi", "96", "--out", f"{W}/out/n_{v}_b.png"], capture_output=True)
    print(json.dumps({"variant": v, "text_similarity": sim(st, dt), "cyr_chars_pdf": cyr(st), "cyr_chars_docx": cyr(dt),
        "paragraphs_docx": info.get("paragraphs"), "paragraphs_original_docx": orig.get("paragraphs"), "pages_docx_roundtrip": rt.get("pages"),
        "pages_source_pdf": op.get("pages"), "tables_hint_original": None, "page1_pixel_diff": diff(f"{W}/out/n_{v}_a.png", f"{W}/out/n_{v}_b.png"),
        "docx_text_lines": [l for l in dt.splitlines() if l.strip()][:3], "pdf_text_nonblank_lines": len([l for l in st.splitlines() if l.strip()]),
        "docx_nonblank_lines": len([l for l in dt.splitlines() if l.strip()])}, ensure_ascii=False))
'@
Set-Content "$W\helpers\analyse.py" $an -Encoding UTF8

if ($Phase -in "prep", "all") { Prep }
if ($Phase -in "run", "all") {
    Run-Census
    $results | ConvertTo-Json -Depth 5 | Set-Content "$W\results.json" -Encoding UTF8
    "--- F4 (pdf route vs docs-cli render, per page)"
    & python "$W\helpers\analyse.py" png $W | Tee-Object "$W\f4.json"
    "--- F3 (pdfcraft native pdf -> docx)"
    foreach ($v in "latn", "cyrl") { & python "$W\helpers\analyse.py" native $W $DOCS $PDF $v | Tee-Object "$W\f3_$v.json" }
    $p = @($results | Where-Object status -eq "PASS").Count
    "census: {0} directions, {1} PASS, {2} FAIL; results in {3}\results.json" -f $results.Count, $p, ($results.Count - $p), $W
}
