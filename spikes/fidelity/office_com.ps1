# Microsoft Office 16 COM helper for the fidelity harness.
#   -Mode export  : open each {src} read-only and export a PDF to {out}
#   -Mode inspect : open each {src} read-only, write structural counts JSON to {out}
# -List is a UTF-8 JSON array of {src,out}. -Result gets a JSON array of per-file status.
# Refuses to run if an Office process already exists (the owner may be using Office); only
# processes created by this script are ever killed.
param(
  [Parameter(Mandatory)][ValidateSet("export", "inspect")][string]$Mode,
  [Parameter(Mandatory)][ValidateSet("word", "excel", "ppt")][string]$App,
  [Parameter(Mandatory)][string]$List,
  [Parameter(Mandatory)][string]$Result
)
$ErrorActionPreference = "Stop"
$procName = @{ word = "WINWORD"; excel = "EXCEL"; ppt = "POWERPNT" }[$App]
$busy = Get-Process WINWORD, EXCEL, POWERPNT -ErrorAction SilentlyContinue
if ($busy) { throw "Office process already running (PIDs $($busy.Id -join ',')); refusing to start." }
$items = [IO.File]::ReadAllText($List, [Text.Encoding]::UTF8) | ConvertFrom-Json
if ($items -isnot [array]) { $items = @($items) }
$miss = [Type]::Missing
$results = New-Object System.Collections.ArrayList

function Safe($sb) { try { & $sb } catch { -1 } }

function Inspect-Word($app, $doc) {
  $doc.Bookmarks.ShowHidden = $true
  $bm = @(); foreach ($b in $doc.Bookmarks) { $bm += $b.Name }
  $hf = 0
  foreach ($s in $doc.Sections) { foreach ($i in 1) {
      $hf += Safe { $s.Headers($i).Range.Fields.Count }
      $hf += Safe { $s.Footers($i).Range.Fields.Count } } }
  $hftext = ""
  foreach ($s in $doc.Sections) { $hftext += (Safe { $s.Headers(1).Range.Text }) + "|" + (Safe { $s.Footers(1).Range.Text }) + ";" }
  [ordered]@{
    pages = $doc.ComputeStatistics(2); words = $doc.ComputeStatistics(0)
    paragraphs = $doc.Paragraphs.Count; tables = $doc.Tables.Count
    inline_images = $doc.InlineShapes.Count; floating_shapes = $doc.Shapes.Count
    bookmarks = $bm.Count; bookmark_names = $bm
    fields = $doc.Fields.Count; header_footer_fields = $hf; header_footer_text = $hftext
    comments = $doc.Comments.Count; revisions = $doc.Revisions.Count
    footnotes = $doc.Footnotes.Count; sections = $doc.Sections.Count
    lists_paragraphs = (Safe { $doc.ListParagraphs.Count })
  }
}
function Inspect-Excel($app, $wb) {
  $f = 0; $ch = 0; $cf = 0; $merged = 0; $sheets = @()
  foreach ($ws in $wb.Worksheets) {
    $nf = 0
    try { $nf = $ws.UsedRange.SpecialCells(-4123).Count } catch { $nf = 0 }
    $f += $nf
    $ch += $ws.ChartObjects().Count + (Safe { $ws.Shapes.Count } ) * 0
    $cf += Safe { $ws.Cells.FormatConditions.Count }
    $m = 0; try { $m = ($ws.UsedRange.MergeCells -eq $true) } catch {}
    if ($m) { $merged += 1 }
    $sheets += [ordered]@{ name = $ws.Name; used = $ws.UsedRange.Address($false, $false); formulas = $nf; charts = $ws.ChartObjects().Count }
  }
  [ordered]@{ sheets = $wb.Worksheets.Count; formulas = $f; charts = $ch; cond_formats = $cf
    names = $wb.Names.Count; sheet_detail = $sheets }
}
function Inspect-Ppt($app, $pr) {
  $shapes = 0; $notes = 0; $tables = 0; $pics = 0; $charts = 0; $chars = 0
  foreach ($s in $pr.Slides) {
    foreach ($sh in $s.Shapes) {
      $shapes++
      if ($sh.HasTable -eq -1) { $tables++ }
      if ($sh.Type -eq 13) { $pics++ }
      if ($sh.HasChart -eq -1) { $charts++ }
      if ($sh.HasTextFrame -eq -1) { $chars += $sh.TextFrame.TextRange.Text.Length }
    }
    try { if ($s.NotesPage.Shapes.Placeholders(2).TextFrame.TextRange.Text.Trim().Length -gt 0) { $notes++ } } catch {}
  }
  [ordered]@{ slides = $pr.Slides.Count; shapes = $shapes; tables = $tables; pictures = $pics; charts = $charts
    notes_slides = $notes; text_chars = $chars }
}

$com = @{ word = "Word.Application"; excel = "Excel.Application"; ppt = "PowerPoint.Application" }[$App]
$oa = New-Object -ComObject $com
$mine = Get-Process $procName -ErrorAction SilentlyContinue | ForEach-Object { $_.Id }
try {
  if ($App -ne "ppt") { $oa.Visible = $false }
  if ($App -eq "word") { $oa.DisplayAlerts = 0 }
  if ($App -eq "excel") { $oa.DisplayAlerts = $false; $oa.AskToUpdateLinks = $false }
  foreach ($it in $items) {
    $r = [ordered]@{ src = $it.src; out = $it.out; ok = $false; error = $null }
    $sw = [Diagnostics.Stopwatch]::StartNew()
    $obj = $null
    try {
      switch ($App) {
        "word" { $obj = $oa.Documents.Open($it.src, $false, $true, $false) }
        "excel" { $obj = $oa.Workbooks.Open($it.src, 0, $true) }
        "ppt" { $obj = $oa.Presentations.Open($it.src, -1, 0, 0) }
      }
      if ($Mode -eq "export") {
        switch ($App) {
          "word" { $obj.ExportAsFixedFormat($it.out, 17) }
          "excel" { $obj.ExportAsFixedFormat(0, $it.out) }
          "ppt" { $obj.SaveAs($it.out, 32) }
        }
        $r.ok = Test-Path $it.out
      } else {
        $data = switch ($App) { "word" { Inspect-Word $oa $obj } "excel" { Inspect-Excel $oa $obj } "ppt" { Inspect-Ppt $oa $obj } }
        [IO.File]::WriteAllText($it.out, ($data | ConvertTo-Json -Depth 6), (New-Object Text.UTF8Encoding $false))
        $r.ok = $true
      }
    } catch { $r.error = $_.Exception.Message }
    finally {
      if ($obj -ne $null) { try { if ($App -eq "word") { $obj.Close(0) } elseif ($App -eq "excel") { $obj.Close($false) } else { $obj.Close() } } catch {} }
    }
    $r.ms = $sw.ElapsedMilliseconds
    [void]$results.Add($r)
  }
} finally {
  try { $oa.Quit() } catch {}
  try { [void][Runtime.InteropServices.Marshal]::ReleaseComObject($oa) } catch {}
  [GC]::Collect(); [GC]::WaitForPendingFinalizers(); Start-Sleep 2
  Get-Process $procName -ErrorAction SilentlyContinue | Where-Object { $mine -contains $_.Id } | Stop-Process -Force
  [IO.File]::WriteAllText($Result, (ConvertTo-Json -InputObject @($results) -Depth 4), (New-Object Text.UTF8Encoding $false))
}
