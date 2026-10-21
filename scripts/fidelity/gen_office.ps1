# Generates the SYNTHETIC fidelity test set with Microsoft Office 16 via COM.
# usage: powershell -NoProfile -File gen_office.ps1 -Out E:\office-target\fidelity\synthetic
# Synthetic content only (content.json). Creates its own Office instances, quits them afterwards.
param([string]$Out = "E:\office-target\fidelity\synthetic", [string]$Only = "word,excel,ppt")
$ErrorActionPreference = "Stop"
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$T = [IO.File]::ReadAllText("$here\content.json", [Text.Encoding]::UTF8) | ConvertFrom-Json
New-Item -ItemType Directory -Force $Out | Out-Null
$miss = [Type]::Missing

$busy = Get-Process WINWORD, EXCEL, POWERPNT -ErrorAction SilentlyContinue
if ($busy) { throw "Office is running (PIDs $($busy.Id -join ',')); not touching it." }

function Stop-Mine($app, $name) {
  try { $app.Quit() } catch {}
  try { [void][Runtime.InteropServices.Marshal]::ReleaseComObject($app) } catch {}
  [GC]::Collect(); [GC]::WaitForPendingFinalizers()
  Start-Sleep 2
  Get-Process $name -ErrorAction SilentlyContinue | Stop-Process -Force   # only ours: none existed at start
}

# ---- PNG
Add-Type -AssemblyName System.Drawing
$png = "$Out\sample.png"
$bmp = New-Object Drawing.Bitmap 320, 180
$g = [Drawing.Graphics]::FromImage($bmp)
$g.Clear([Drawing.Color]::FromArgb(30, 90, 160))
$g.FillEllipse([Drawing.Brushes]::Gold, 40, 30, 120, 120)
$g.FillRectangle([Drawing.Brushes]::White, 180, 50, 110, 80)
$g.DrawString("HRM", (New-Object Drawing.Font "Arial", 20), [Drawing.Brushes]::Black, 195, 70)
$g.Dispose(); $bmp.Save($png, [Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()

# =================== WORD ===================
function AddP($doc, $text, $font, $size, $bold, $align) {
  $r = $doc.Range($doc.Content.End - 1, $doc.Content.End - 1)
  $r.Text = $text
  $r.Font.Name = $font; $r.Font.Size = $size; $r.Font.Bold = $bold
  $r.ParagraphFormat.Alignment = $align
  $r.InsertParagraphAfter()
  return $r
}
function MakeHeaderFooter($doc) {
  $sec = $doc.Sections(1)
  $h = $sec.Headers(1).Range; $h.Text = "HRM sinov / test - "; $h.Font.Name = "Calibri"; $h.Font.Size = 9
  $e = $sec.Headers(1).Range; $e.Collapse(0); [void]$doc.Fields.Add($e, 33)
  $f = $sec.Footers(1).Range; $f.Text = "Page "; $f.Font.Name = "Calibri"; $f.Font.Size = 9
  $e = $sec.Footers(1).Range; $e.Collapse(0); [void]$doc.Fields.Add($e, 33)
  $e = $sec.Footers(1).Range; $e.InsertAfter(" of "); $e = $sec.Footers(1).Range; $e.Collapse(0); [void]$doc.Fields.Add($e, 26)
  $sec.Footers(1).Range.ParagraphFormat.Alignment = 1
}
function AddTable($doc, $T) {
  $r = $doc.Range($doc.Content.End - 1, $doc.Content.End - 1)
  $tb2 = $doc.Tables.Add($r, 7, 4)
  $tb2.Borders.Enable = $true
  $tb2.Range.Font.Name = "Times New Roman"; $tb2.Range.Font.Size = 12
  for ($c = 1; $c -le 4; $c++) { $tb2.Cell(1, $c).Range.Text = $T.th[$c - 1]; $tb2.Cell(1, $c).Range.Font.Bold = $true }
  for ($i = 0; $i -lt 5; $i++) {
    $tb2.Cell($i + 2, 1).Range.Text = [string]($i + 1)
    $tb2.Cell($i + 2, 2).Range.Text = $T.names[$i]
    $tb2.Cell($i + 2, 3).Range.Text = $T.pos[$i]
    $tb2.Cell($i + 2, 4).Range.Text = ("{0:N0}" -f (1500000 + $i * 250000))
  }
  $tb2.Cell(2, 3).Merge($tb2.Cell(3, 3))      # vertical merge
  $tb2.Cell(7, 1).Merge($tb2.Cell(7, 3))      # horizontal merge
  $tb2.Cell(7, 1).Range.Text = "Jami / Итого"
  $tb2.Cell(7, 2).Range.Text = "8 750 000"
  for ($c = 1; $c -le 4; $c++) { $tb2.Cell(1, $c).Shading.BackgroundPatternColor = 14277081 }
  return $tb2
}

if ($Only -match "word") {
$word = New-Object -ComObject Word.Application
$word.Visible = $false; $word.DisplayAlerts = 0
$origUser = $word.UserName
try {
  # ---------- doc 1: order (Uzbek Cyr / Lat / Russian, 4+ pages)
  $word.UserName = "Test Reviewer"
  $doc = $word.Documents.Add()
  $doc.PageSetup.PaperSize = 7   # A4
  MakeHeaderFooter $doc
  $TNR = "Times New Roman"
  [void](AddP $doc $T.title_cyr $TNR 14 1 1)
  [void](AddP $doc $T.no $TNR 14 0 1)
  [void](AddP $doc $T.place_cyr $TNR 14 0 0)
  [void](AddP $doc $T.head_cyr $TNR 14 1 1)
  $r = AddP $doc $T.p_cyr[0] $TNR 14 0 3
  $r = AddP $doc $T.p_cyr[1] $TNR 14 0 3
  # footnote on 2nd paragraph
  $fr = $doc.Range($r.End - 3, $r.End - 1)
  [void]$doc.Footnotes.Add($fr, $miss, $T.fn)
  $ls = $doc.Content.End - 1
  foreach ($l in $T.list_cyr) { [void](AddP $doc $l $TNR 14 0 0) }
  $le = $doc.Content.End - 1
  $doc.Range($ls, $le - 1).ListFormat.ApplyNumberDefault()
  $br = AddP $doc $T.bm_text $TNR 14 0 3
  $doc.Bookmarks.ShowHidden = $true
  [void]$doc.Bookmarks.Add("_hrm_desc_", $doc.Range($br.Start, $br.End - 1))
  [void](AddP $doc $T.p_cyr[2] $TNR 14 0 3)
  $pb = $doc.Range($doc.Content.End - 1, $doc.Content.End - 1); $pb.InsertBreak(7)

  [void](AddP $doc $T.title_lat $TNR 14 1 1)
  [void](AddP $doc $T.place_lat $TNR 14 0 0)
  [void](AddP $doc $T.head_lat $TNR 14 1 1)
  foreach ($p in $T.p_lat) { [void](AddP $doc $p $TNR 14 0 3) }
  [void](AddTable $doc $T)
  $ir = $doc.Range($doc.Content.End - 1, $doc.Content.End - 1)
  [void]$doc.InlineShapes.AddPicture($png, $false, $true, $ir)
  [void]$doc.Paragraphs.Last.Range.InsertParagraphAfter()
  $pb = $doc.Range($doc.Content.End - 1, $doc.Content.End - 1); $pb.InsertBreak(7)

  [void](AddP $doc $T.title_ru "Calibri" 14 1 1)
  [void](AddP $doc $T.place_ru "Calibri" 12 0 0)
  [void](AddP $doc $T.head_ru "Calibri" 12 1 1)
  $r1 = AddP $doc $T.p_ru[0] "Calibri" 12 0 3
  $r2 = AddP $doc $T.p_ru[1] "Calibri" 12 0 3
  [void](AddP $doc $T.p_ru[2] "Calibri" 12 0 3)
  # text box
  $tb = $doc.Shapes.AddTextbox(1, 300, 380, 220, 80, $r1)
  $tb.TextFrame.TextRange.Text = $T.tb
  $tb.TextFrame.TextRange.Font.Name = "Calibri"; $tb.TextFrame.TextRange.Font.Size = 10
  # tracked change + comment
  $doc.TrackRevisions = $true
  $ip = $doc.Range($r2.End - 1, $r2.End - 1); $ip.InsertAfter($T.ins)
  $doc.TrackRevisions = $false
  [void]$doc.Comments.Add($doc.Range($r1.Start, $r1.Start + 20), $T.comment)
  # section break -> landscape page
  $sb = $doc.Range($doc.Content.End - 1, $doc.Content.End - 1); $sb.InsertBreak(2)
  $doc.Sections($doc.Sections.Count).PageSetup.Orientation = 1
  [void](AddP $doc $T.title_ru "Calibri" 14 1 1)
  [void](AddTable $doc $T)
  [void](AddP $doc "Calibri 11 / Times New Roman 14 mix: Приложение № 1." "Calibri" 11 0 0)
  $doc.Repaginate()
  $pages = $doc.ComputeStatistics(2)
  Write-Host "order pages: $pages"
  $doc.SaveAs2("$Out\hrm_order.docx", 16)
  $doc.SaveAs2("$Out\hrm_order.doc", 0)
  $doc.Close(0)

  # ---------- doc 2: letter (Russian + Latin, Calibri, simple)
  $doc = $word.Documents.Add()
  $doc.PageSetup.PaperSize = 7
  MakeHeaderFooter $doc
  [void](AddP $doc "Служебная записка / Xizmat xati" "Calibri" 16 1 1)
  foreach ($p in $T.p_ru) { [void](AddP $doc $p "Calibri" 11 0 3) }
  foreach ($p in $T.p_lat) { [void](AddP $doc $p "Calibri" 11 0 3) }
  $ls = $doc.Content.End - 1
  foreach ($l in $T.list_cyr) { [void](AddP $doc $l "Calibri" 11 0 0) }
  $le = $doc.Content.End - 1
  $doc.Range($ls, $le - 1).ListFormat.ApplyBulletDefault()
  [void](AddTable $doc $T)
  [void]$doc.Bookmarks.Add("_hrm_desc_", $doc.Paragraphs(2).Range)
  $doc.SaveAs2("$Out\hrm_letter.docx", 16)
  $doc.SaveAs2("$Out\hrm_letter.doc", 0)
  $doc.Close(0)
} finally {
  try { $word.UserName = $origUser } catch {}
  Stop-Mine $word "WINWORD"
}

}
# =================== EXCEL ===================
if ($Only -match "excel") {
$xl = New-Object -ComObject Excel.Application
$xl.Visible = $false; $xl.DisplayAlerts = $false
try {
  $wb = $xl.Workbooks.Add()
  while ($wb.Worksheets.Count -lt 3) { [void]$wb.Worksheets.Add([Type]::Missing, $wb.Worksheets($wb.Worksheets.Count)) }
  $s1 = $wb.Worksheets(1); $s1.Name = "Xodimlar"
  $s2 = $wb.Worksheets(2); $s2.Name = "Ish_haqi"
  $s3 = $wb.Worksheets(3); $s3.Name = "Xulosa"
  $s1.Range("A1:F1").Merge(); $s1.Range("A1").Value2 = $T.xl_title
  $s1.Range("A1").Font.Bold = $true; $s1.Range("A1").Font.Size = 14; $s1.Range("A1").HorizontalAlignment = -4108
  $hd = "ID", "F.I.Sh.", "Bo'lim", "Lavozim", "Maosh", "Qabul sanasi"
  for ($c = 0; $c -lt 6; $c++) { $s1.Cells(3, $c + 1).Value2 = $hd[$c]; $s1.Cells(3, $c + 1).Font.Bold = $true; $s1.Cells(3, $c + 1).Interior.Color = 14277081 }
  $base = [DateTime]"2019-03-01"
  for ($i = 0; $i -lt 12; $i++) {
    $row = 4 + $i
    $s1.Cells($row, 1).Value2 = [double](1001 + $i)
    $s1.Cells($row, 2).Value2 = $T.names[$i % 5] + " " + ($i + 1)
    $s1.Cells($row, 3).Value2 = $T.dept[$i % 5]
    $s1.Cells($row, 4).Value2 = $T.pos[$i % 5]
    $s1.Cells($row, 5).Value2 = [double](3500000 + ($i * 417000) % 4000000)
    $s1.Cells($row, 6).Value2 = [double]($base.AddDays($i * 97).ToOADate())
  }
  $s1.Range("E4:E15").NumberFormat = "#,##0"
  $s1.Range("F4:F15").NumberFormatLocal = "дд.мм.гггг"   # NumberFormat "dd.mm.yyyy" writes a corrupt code on a ru-RU Excel
  $s1.Columns("A:F").AutoFit() | Out-Null
  $fc = $s1.Range("E4:E15").FormatConditions.Add(1, 5, "=6000000")   # cellvalue > 6e6
  $fc.Interior.Color = 13421823; $fc.Font.Color = 393372
  $s1.Activate(); $xl.ActiveWindow.FreezePanes = $false; $xl.ActiveWindow.SplitColumn = 0; $xl.ActiveWindow.SplitRow = 3; $xl.ActiveWindow.FreezePanes = $true
  try { $s1.PageSetup.PrintArea = '$A$1:$F$15' } catch { Write-Host "PrintArea skipped: $($_.Exception.Message)" }

  $s2.Range("A1:E1").Merge(); $s2.Range("A1").Value2 = "Ish haqi hisobi / Расчёт зарплаты"; $s2.Range("A1").Font.Bold = $true
  $h2 = "ID", "F.I.Sh.", "Maosh", "Mukofot", "Jami"
  for ($c = 0; $c -lt 5; $c++) { $s2.Cells(3, $c + 1).Value2 = $h2[$c]; $s2.Cells(3, $c + 1).Font.Bold = $true }
  for ($i = 0; $i -lt 12; $i++) {
    $row = 4 + $i
    $s2.Cells($row, 1).Formula = "=Xodimlar!A$row"
    $s2.Cells($row, 2).Formula = "=VLOOKUP(A$row,Xodimlar!`$A`$4:`$F`$15,2,FALSE)"
    $s2.Cells($row, 3).Formula = "=VLOOKUP(A$row,Xodimlar!`$A`$4:`$F`$15,5,FALSE)"
    $s2.Cells($row, 4).Formula = "=IF(C$row>5000000,C$row*0.05,C$row*0.1)"
    $s2.Cells($row, 5).Formula = "=C$row+D$row"
  }
  $s2.Range("A16").Value2 = "Jami"; foreach ($col in "C", "D", "E") { $s2.Range("${col}16").Formula = "=SUM(${col}4:${col}15)" }
  $s2.Range("C4:E16").NumberFormat = "#,##0"
  $s3 = $wb.Worksheets("Xulosa")
  $s3.Range("A1").Value2 = "Bo'lim bo'yicha / По отделам"; $s3.Range("A1").Font.Bold = $true
  $s3.Range("A2").Value2 = "Bo'lim"; $s3.Range("B2").Value2 = "Jami maosh"; $s3.Range("C2").Value2 = "Holat"
  for ($i = 0; $i -lt 5; $i++) {
    $row = 3 + $i
    $s3.Cells($row, 1).Value2 = $T.dept[$i]
    $s3.Cells($row, 2).Formula = "=SUMIFS(Ish_haqi!`$E`$4:`$E`$15,Xodimlar!`$C`$4:`$C`$15,A$row)"
    $s3.Cells($row, 3).Formula = "=IF(B$row>15000000,""yuqori"",""normal"")"
  }
  $s3.Range("B3:B7").NumberFormat = "#,##0"
  $s3.Range("A9").Value2 = "Jami"; $s3.Range("B9").Formula = "=SUM(B3:B7)"; $s3.Range("B9").NumberFormat = "#,##0"
  $s3.Range("A10").Value2 = "Sana"; $s3.Range("B10").Formula = "=DATE(2026,10,15)"; $s3.Range("B10").NumberFormatLocal = "дд мммм гггг"
  $co = $s3.ChartObjects().Add(250, 20, 360, 220)
  $co.Chart.ChartType = 51
  $co.Chart.SetSourceData($s3.Range("A2:B7"))
  $co.Chart.HasTitle = $true; $co.Chart.ChartTitle.Text = "Jami maosh / bo'lim"
  $s1.Activate()
  $wb.SaveAs("$Out\hrm_staff.xlsx", 51)
  $wb.SaveAs("$Out\hrm_staff.xls", 56)
  $wb.Close($false)

  # second workbook: timesheet (simple grid, dates, SUM)
  $wb = $xl.Workbooks.Add()
  $w = $wb.Worksheets(1); $w.Name = "Tabel"
  $w.Range("A1:H1").Merge(); $w.Range("A1").Value2 = "Табель учёта / Ish vaqti jadvali"; $w.Range("A1").Font.Bold = $true; $w.Range("A1").HorizontalAlignment = -4108
  $w.Range("A2").Value2 = "F.I.Sh."
  for ($d = 0; $d -lt 5; $d++) { $w.Cells(2, $d + 2).Value2 = [double](([DateTime]"2026-10-12").AddDays($d).ToOADate()); $w.Cells(2, $d + 2).NumberFormatLocal = "дд.мм" }
  $w.Range("G2").Value2 = "Jami soat"; $w.Range("H2").Value2 = "Foiz"
  for ($i = 0; $i -lt 5; $i++) {
    $row = 3 + $i
    $w.Cells($row, 1).Value2 = $T.names[$i]
    for ($d = 0; $d -lt 5; $d++) { $w.Cells($row, $d + 2).Value2 = [double](6 + (($i + $d) % 3)) }
    $w.Cells($row, 7).Formula = "=SUM(B${row}:F${row})"
    $w.Cells($row, 8).Formula = "=G$row/40"; $w.Cells($row, 8).NumberFormat = "0%"
  }
  $w.Range("A8:H8").Borders.LineStyle = 1; $w.Range("A2:H7").Borders.LineStyle = 1
  $w.Columns("A:H").AutoFit() | Out-Null
  $wb.SaveAs("$Out\hrm_timesheet.xlsx", 51)
  $wb.SaveAs("$Out\hrm_timesheet.xls", 56)
  $wb.Close($false)
} finally { Stop-Mine $xl "EXCEL" }

}
# =================== POWERPOINT ===================
if ($Only -match "ppt") {
$pp = New-Object -ComObject PowerPoint.Application
try {
  $pr = $pp.Presentations.Add(0)
  $s = $pr.Slides.Add(1, 1)
  $s.Shapes(1).TextFrame.TextRange.Text = $T.ppt_title
  $s.Shapes(2).TextFrame.TextRange.Text = $T.ppt_sub
  $s = $pr.Slides.Add(2, 2)
  $s.Shapes(1).TextFrame.TextRange.Text = "Asosiy ko'rsatkichlar / Основные показатели"
  $s.Shapes(2).TextFrame.TextRange.Text = ($T.ppt_bul -join "`r")
  $s.NotesPage.Shapes.Placeholders(2).TextFrame.TextRange.Text = $T.ppt_notes
  $s = $pr.Slides.Add(3, 12)   # blank
  $tbl = $s.Shapes.AddTable(4, 3, 40, 60, 640, 200)
  for ($r = 1; $r -le 4; $r++) { for ($c = 1; $c -le 3; $c++) {
      $txt = if ($r -eq 1) { $T.th[$c - 1] } elseif ($c -eq 1) { $T.names[$r - 2] } elseif ($c -eq 2) { $T.pos[$r - 2] } else { "{0:N0}" -f (1000000 * $r) }
      $tbl.Table.Cell($r, $c).Shape.TextFrame.TextRange.Text = $txt } }
  $s.NotesPage.Shapes.Placeholders(2).TextFrame.TextRange.Text = "Jadval slayd izohi"
  $s = $pr.Slides.Add(4, 12)
  [void]$s.Shapes.AddPicture($png, 0, -1, 80, 80, 320, 180)
  $s.Shapes.AddTextbox(1, 80, 20, 500, 40).TextFrame.TextRange.Text = "Rasm / Изображение"
  $chartOk = $true
  try {
    $s = $pr.Slides.Add(5, 12)
    $sh = $s.Shapes.AddChart2(-1, 51, 60, 60, 560, 340)   # default sample data
  } catch { $chartOk = $false; Write-Host "PPT chart skipped: $($_.Exception.Message)" }
  $pr.SaveAs("$Out\hrm_deck.pptx", 24)
  $pr.SaveAs("$Out\hrm_deck.ppt", 1)
  $pr.Close()
} finally {
  try { $pp.Quit() } catch {}
  try { [void][Runtime.InteropServices.Marshal]::ReleaseComObject($pp) } catch {}
  Start-Sleep 2
  Get-Process POWERPNT, EXCEL -ErrorAction SilentlyContinue | Stop-Process -Force
}
}
Get-ChildItem $Out | Select Name, Length
