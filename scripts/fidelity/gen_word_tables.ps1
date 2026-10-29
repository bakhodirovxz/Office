# Word 16 builds synthetic table documents through COM, one per table feature class, saves each as .docx and exports
# Word's own PDF next to it (the truth for docs/phase4/tables.md). Synthetic text only; the outputs stay under -Out (E:),
# are never committed (they are files produced by Word).
#   powershell -NoProfile -File scripts/fidelity/gen_word_tables.ps1 -Out E:/office-target/p4-tables/fid/word [-Only t05,t06]
# Starts its own Word and kills only that PID (com.ps1).
# Not here: .doc / .rtf copies. SaveAs2 to those formats hung a hidden Word on this PC (2026-10-29, read-only or not).
param([Parameter(Mandatory)][string]$Out, [string]$Only = "")
$ErrorActionPreference = "Stop"
. "$PSScriptRoot\com.ps1"
New-Item -ItemType Directory -Force $Out | Out-Null

$names = @("Karimov Aziz Botirovich", "Абдуллаева Нилуфар Шавкатовна", "Rahimova Dilnoza", "Тошпулатов Шерзод Акмалович",
  "Yusupov Jamshid", "Норматова Гулнора", "Xolmatov Bekzod Erkinovich", "Сафарова Мадина Рустамовна")
$posts = @("bosh mutaxassis", "ведущий инженер", "bo'lim boshlig'i", "бухгалтер", "operator", "техник")

function Cm($app, $v) { $app.CentimetersToPoints($v) }
function Fill($t, [string[][]]$rows) {
  for ($r = 0; $r -lt $rows.Count; $r++) { for ($c = 0; $c -lt $rows[$r].Count; $c++) {
      if ($rows[$r][$c] -ne $null) { $t.Cell($r + 1, $c + 1).Range.Text = $rows[$r][$c] } } }
}
function StaffRows($n, $cols) {
  $rows = @()
  for ($i = 0; $i -lt $n; $i++) {
    $row = @("$($i + 1).", $names[$i % $names.Count], $posts[$i % $posts.Count], "$(1200 + 37 * $i),00", "01.0$(1 + $i % 9).2026")
    $rows += , ($row[0..($cols - 1)])
  }
  return , $rows
}
function Para($doc, $text) { $r = $doc.Content; $r.InsertParagraphAfter(); $p = $doc.Paragraphs($doc.Paragraphs.Count).Range; $p.Text = $text; return $doc.Paragraphs($doc.Paragraphs.Count).Range }
function NewTable($doc, $rows, $cols, $behavior = 1, $fit = 0) {
  $doc.Content.InsertParagraphAfter()
  $r = $doc.Paragraphs($doc.Paragraphs.Count).Range
  return $doc.Tables.Add($r, $rows, $cols, $behavior, $fit)
}
function Hrm($doc) {   # the usual order look: Times New Roman 12, no spacing after, single lines
  $n = $doc.Styles.Item(-1)
  $n.Font.Name = "Times New Roman"; $n.Font.Size = 12
  $n.ParagraphFormat.SpaceAfter = 0; $n.ParagraphFormat.SpaceBefore = 0; $n.ParagraphFormat.LineSpacingRule = 0
}

$cases = [ordered]@{
  t01_grid_fixed = { param($app, $doc) Hrm $doc; Para $doc "Simple table, Insert Table defaults (fixed column width, auto)" | Out-Null
    $t = NewTable $doc 6 5 1 0; Fill $t (StaffRows 6 5) }
  t02_autofit_content = { param($app, $doc) Hrm $doc; Para $doc "AutoFit to contents" | Out-Null
    $t = NewTable $doc 6 5 1 1; Fill $t (StaffRows 6 5); $t.AutoFitBehavior(1) }
  t03_autofit_window = { param($app, $doc) Hrm $doc; Para $doc "AutoFit to window" | Out-Null
    $t = NewTable $doc 5 4 1 2; Fill $t (StaffRows 5 4); $t.AutoFitBehavior(2) }
  t04_set_widths = { param($app, $doc) Hrm $doc; Para $doc "Column widths set by hand (1, 6.5, 4, 2.5, 2.5 cm)" | Out-Null
    $t = NewTable $doc 6 5 1 0; Fill $t (StaffRows 6 5)
    $w = @(1.0, 6.5, 4.0, 2.5, 2.5); for ($c = 1; $c -le 5; $c++) { $t.Columns($c).Width = (Cm $app $w[$c - 1]) } }
  t05_merge_header = { param($app, $doc) Hrm $doc; Para $doc "Two-level header: merged across and down" | Out-Null
    $t = NewTable $doc 7 6 1 0
    $w = @(1.0, 5.0, 3.5, 2.2, 2.2, 2.6); for ($c = 1; $c -le 6; $c++) { $t.Columns($c).Width = (Cm $app $w[$c - 1]) }
    Fill $t (@(, @("№", "F.I.Sh.", "Lavozimi", "Ish kunlari", $null, "Izoh")) + @(, @($null, $null, $null, "reja", "fakt", $null)))
    $rows = StaffRows 5 3; for ($r = 0; $r -lt 5; $r++) { Fill2 $t ($r + 3) $rows[$r] }
    foreach ($r in 1, 2) { $t.Rows($r).Range.Font.Bold = $true; $t.Rows($r).Range.ParagraphFormat.Alignment = 1 }
    $t.Range.Cells.VerticalAlignment = 1
    $t.Cell(1, 4).Merge($t.Cell(1, 5))
    $t.Cell(1, 6 - 1).Merge($t.Cell(2, 6)) ; $t.Cell(1, 3).Merge($t.Cell(2, 3)); $t.Cell(1, 2).Merge($t.Cell(2, 2)); $t.Cell(1, 1).Merge($t.Cell(2, 1)) }
  t06_merge_vertical_body = { param($app, $doc) Hrm $doc; Para $doc "Vertical merge in the body (department spans rows)" | Out-Null
    $t = NewTable $doc 7 4 1 0
    $w = @(4.0, 1.0, 6.0, 4.0); for ($c = 1; $c -le 4; $c++) { $t.Columns($c).Width = (Cm $app $w[$c - 1]) }
    Fill $t (@(, @("Bo'lim", "№", "Xodim", "Lavozim")))
    for ($r = 2; $r -le 7; $r++) { Fill2 $t $r @($(if ($r -lt 5) { "Moliya bo'limi" } else { "Отдел кадров" }), "$($r - 1).", $names[$r % 8], $posts[$r % 6]) }
    $t.Cell(2, 1).Merge($t.Cell(4, 1)); $t.Cell(5, 1).Merge($t.Cell(7, 1)); $t.Cell(2, 1).VerticalAlignment = 1; $t.Cell(5, 1).VerticalAlignment = 1 }
  t07_style_banded = { param($app, $doc) Hrm $doc; Para $doc "Table style with header row and banded rows" | Out-Null
    $t = NewTable $doc 7 4 1 0; Fill $t (@(, @("Name", "Lavozim", "Oklad", "Sana")) + ((StaffRows 6 5) | ForEach-Object { , @($_[1], $_[2], $_[3], $_[4]) }))
    foreach ($s in @(-206, "Grid Table 4 - Accent 1", "Таблица-сетка 4 — акцент 1", "Light Grid Accent 1")) { try { $t.Style = $s; break } catch {} } }
  t08_borders_custom = { param($app, $doc) Hrm $doc; Para $doc "Custom borders: thick outside, none inside vertical, double under header" | Out-Null
    $t = NewTable $doc 6 4 1 0; Fill $t (StaffRows 6 4)
    $t.Borders(-6).LineStyle = 0
    foreach ($b in -1, -2, -3, -4) { $t.Borders($b).LineStyle = 1; $t.Borders($b).LineWidth = 12 }
    $t.Rows(1).Borders(-3).LineStyle = 7
    $t.Cell(4, 2).Borders(-3).LineStyle = 1; $t.Cell(4, 2).Borders(-3).LineWidth = 18; $t.Cell(4, 2).Borders(-3).Color = 255 }
  t09_no_borders_layout = { param($app, $doc) Hrm $doc; Para $doc "Layout table without borders (signature block)" | Out-Null
    $t = NewTable $doc 3 3 0 0
    $w = @(7.0, 4.0, 5.5); for ($c = 1; $c -le 3; $c++) { $t.Columns($c).Width = (Cm $app $w[$c - 1]) }
    Fill $t @(@("Bosh direktor", "________", "A. B. Karimov"), @("Бош ҳисобчи", "________", "Н. Ш. Абдуллаева"), @("Kelishildi:", "", "Yuriskonsult"))
    $t.Columns(3).Select(); $app.Selection.ParagraphFormat.Alignment = 2
    Para $doc "Text after the table." | Out-Null }
  t10_shading_valign_dir = { param($app, $doc) Hrm $doc; Para $doc "Header shading, vertical alignment and vertical text" | Out-Null
    $t = NewTable $doc 5 5 1 0; Fill $t (@(, @("№", "F.I.Sh.", "Smena", "Soat", "Imzo")) + (StaffRows 4 5))
    $t.Rows(1).Height = (Cm $app 2.5); $t.Rows(1).HeightRule = 1
    $t.Rows(1).Shading.BackgroundPatternColor = 14277081
    for ($c = 3; $c -le 5; $c++) { $t.Cell(1, $c).Range.Orientation = 2 }
    $t.Cell(1, 2).VerticalAlignment = 1; $t.Cell(2, 3).Shading.BackgroundPatternColor = 10092543 }
  t11_row_heights = { param($app, $doc) Hrm $doc; Para $doc "Row heights: exactly 1.5 cm, at least 1 cm, auto" | Out-Null
    $t = NewTable $doc 6 3 1 0; Fill $t (StaffRows 6 3)
    $t.Rows(2).HeightRule = 2; $t.Rows(2).Height = (Cm $app 1.5)
    $t.Rows(3).HeightRule = 1; $t.Rows(3).Height = (Cm $app 1.0)
    $t.Rows(4).HeightRule = 2; $t.Rows(4).Height = (Cm $app 0.4)
    $t.Cell(5, 2).Range.Text = "Uzun matn: " + ("so'z " * 30) }
  t12_align_indent = { param($app, $doc) Hrm $doc; Para $doc "Centred table, right-aligned table, indented table" | Out-Null
    $t = NewTable $doc 3 3 1 0; Fill $t (StaffRows 3 3); $t.PreferredWidthType = 3; $t.PreferredWidth = (Cm $app 10); $t.Columns.Width = (Cm $app 3.3); $t.Rows.Alignment = 1
    Para $doc "Right:" | Out-Null
    $t = NewTable $doc 3 3 1 0; Fill $t (StaffRows 3 3); $t.Columns.Width = (Cm $app 3); $t.Rows.Alignment = 2
    Para $doc "Indented 2 cm:" | Out-Null
    $t = NewTable $doc 3 3 1 0; Fill $t (StaffRows 3 3); $t.Columns.Width = (Cm $app 3); $t.Rows.LeftIndent = (Cm $app 2) }
  t13_nested = { param($app, $doc) Hrm $doc; Para $doc "Nested table" | Out-Null
    $t = NewTable $doc 3 2 1 0; Fill $t @(@("Outer A", "Outer B"), @("", "right cell"), @("bottom", "end"))
    $t.Columns(1).Width = (Cm $app 9); $t.Columns(2).Width = (Cm $app 7)
    $n = $t.Cell(2, 1).Range.Tables.Add($t.Cell(2, 1).Range, 3, 3, 1, 0)
    for ($r = 1; $r -le 3; $r++) { for ($c = 1; $c -le 3; $c++) { $n.Cell($r, $c).Range.Text = "n$r$c" } } }
  t14_repeat_header_long = { param($app, $doc) Hrm $doc; Para $doc "Long table, header row repeats, rows do not split" | Out-Null
    $t = NewTable $doc 60 5 1 0; Fill $t (@(, @("№", "F.I.Sh.", "Lavozimi", "Oklad", "Sana")) + (StaffRows 59 5))
    $t.Rows(1).HeadingFormat = -1; $t.Rows(1).Range.Font.Bold = $true; $t.Rows.AllowBreakAcrossPages = 0
    $t.Cell(30, 3).Range.Text = "Ko'p qatorli katak: " + ("matn " * 25) }
  t15_floating = { param($app, $doc) Hrm $doc
    for ($i = 0; $i -lt 4; $i++) { Para $doc ("Body text that wraps around the floating table. " * 6) | Out-Null }
    $t = NewTable $doc 4 2 1 0; Fill $t (StaffRows 4 2); $t.Columns.Width = (Cm $app 3.5)
    $t.Rows.WrapAroundText = -1; $t.Rows.RelativeHorizontalPosition = 0; $t.Rows.HorizontalPosition = (Cm $app 9); $t.Rows.RelativeVerticalPosition = 2; $t.Rows.VerticalPosition = (Cm $app 0.3)
    $t.Rows.DistanceLeft = (Cm $app 0.4)
    for ($i = 0; $i -lt 4; $i++) { Para $doc ("More text after the table position. " * 6) | Out-Null } }
  t16_cell_margins_spacing = { param($app, $doc) Hrm $doc; Para $doc "Cell margins 0.5 cm, top 0.2 cm; second table with cell spacing" | Out-Null
    $t = NewTable $doc 4 3 1 0; Fill $t (StaffRows 4 3); $t.LeftPadding = (Cm $app 0.5); $t.RightPadding = (Cm $app 0.5); $t.TopPadding = (Cm $app 0.2); $t.BottomPadding = (Cm $app 0.2)
    Para $doc "Spacing:" | Out-Null
    $t = NewTable $doc 4 3 1 0; Fill $t (StaffRows 4 3); $t.Spacing = (Cm $app 0.1) }
  t17_percent = { param($app, $doc) Hrm $doc; Para $doc "Preferred widths in percent (table 80 %, columns 10/50/40 %)" | Out-Null
    $t = NewTable $doc 5 3 1 0; Fill $t (StaffRows 5 3); $t.PreferredWidthType = 2; $t.PreferredWidth = 80
    $p = @(10, 50, 40); for ($c = 1; $c -le 3; $c++) { $t.Columns($c).PreferredWidthType = 2; $t.Columns($c).PreferredWidth = $p[$c - 1] } }
  t18_irregular_rows = { param($app, $doc) Hrm $doc; Para $doc "Rows with different column boundaries (split cells, own widths)" | Out-Null
    $t = NewTable $doc 5 3 1 0; Fill $t (StaffRows 5 3)
    $t.Columns(1).Width = (Cm $app 2); $t.Columns(2).Width = (Cm $app 7); $t.Columns(3).Width = (Cm $app 6)
    $t.Cell(3, 2).Split(1, 2); $t.Cell(3, 2).Range.Text = "a"; $t.Cell(3, 3).Range.Text = "b"
    $t.Cell(4, 1).Width = (Cm $app 5); $t.Cell(4, 2).Width = (Cm $app 4)
    $t.Rows(5).Cells.Merge() }
  t19_compat2003 = { param($app, $doc) Hrm $doc; $doc.SetCompatibilityMode(11); Para $doc "Word 2003 compatibility mode, indented table with merged cells" | Out-Null
    $t = NewTable $doc 6 5 1 0; Fill $t (StaffRows 6 5); $t.Rows.LeftIndent = (Cm $app 0.5); $t.Cell(1, 4).Merge($t.Cell(1, 5)) }
  t20_compat2010_autofit = { param($app, $doc) Hrm $doc; $doc.SetCompatibilityMode(14); Para $doc "Word 2010 mode, AutoFit to contents with a long cell" | Out-Null
    $t = NewTable $doc 5 4 1 1; Fill $t (StaffRows 5 4); $t.Cell(3, 2).Range.Text = "Juda uzun matn bir katakda, u qatorni bir necha marta o'raydi: " + ("so'z " * 20); $t.AutoFitBehavior(1) }
  t22_timesheet = { param($app, $doc) Hrm $doc; $doc.PageSetup.Orientation = 1; Para $doc "Timesheet: 31 narrow day columns under one merged header" | Out-Null
    $t = NewTable $doc 6 35 1 0; $t.Range.Font.Size = 8
    $t.Columns(1).Width = (Cm $app 0.7); $t.Columns(2).Width = (Cm $app 4.2); $t.Columns(3).Width = (Cm $app 2.2)
    for ($c = 4; $c -le 34; $c++) { $t.Columns($c).Width = (Cm $app 0.55) }; $t.Columns(35).Width = (Cm $app 1.2)
    for ($c = 4; $c -le 34; $c++) { $t.Cell(2, $c).Range.Text = "$($c - 3)" }
    for ($r = 3; $r -le 6; $r++) { $t.Cell($r, 1).Range.Text = "$($r - 2)"; $t.Cell($r, 2).Range.Text = $names[$r]; $t.Cell($r, 3).Range.Text = $posts[$r % 6]
      for ($c = 4; $c -le 34; $c++) { $t.Cell($r, $c).Range.Text = $(if ($c % 7 -eq 0) { "B" } else { "8" }) }; $t.Cell($r, 35).Range.Text = "176" }
    $t.Cell(1, 1).Range.Text = "№"; $t.Cell(1, 2).Range.Text = "Familiyasi, ismi"; $t.Cell(1, 3).Range.Text = "Lavozimi"; $t.Cell(1, 35).Range.Text = "Jami soat"
    $t.Cell(1, 4).Range.Text = "Oyning kunlari bo'yicha ishga kelish va kelmaslik belgilari"
    $t.Cell(1, 4).Merge($t.Cell(1, 34)) }
  t23_form_card = { param($app, $doc) Hrm $doc; Para $doc "Form card: every row has its own cell boundaries" | Out-Null
    $t = NewTable $doc 8 4 1 0
    $spec = @(@(3.0, 13.5), @(4.5, 4.0, 3.0, 5.0), @(2.0, 6.0, 2.5, 6.0), @(8.0, 8.5), @(3.5, 3.5, 3.5, 6.0), @(1.2, 9.0, 6.3), @(5.0, 2.0, 9.5), @(16.5))
    $txt = @("Tabel raqami", "Familiyasi", "Ismi", "Otasining ismi", "Tug'ilgan sanasi", "Фуқаролиги", "Маълумоти", "Mutaxassisligi")
    for ($r = 1; $r -le 8; $r++) {
      $w = $spec[$r - 1]
      while ($t.Rows($r).Cells.Count -gt $w.Count) { $t.Rows($r).Cells($t.Rows($r).Cells.Count - 1).Merge($t.Rows($r).Cells($t.Rows($r).Cells.Count)) }
      for ($c = 1; $c -le $w.Count; $c++) { $t.Cell($r, $c).Width = (Cm $app $w[$c - 1]); $t.Cell($r, $c).Range.Text = $(if ($c % 2) { $txt[($r + $c) % 8] } else { $names[($r + $c) % 8] }) } } }
  t24_autofit_long_2013 = { param($app, $doc) Hrm $doc; Para $doc "Word 2013+ mode, AutoFit to contents with a long cell" | Out-Null
    $t = NewTable $doc 5 4 1 1; Fill $t (StaffRows 5 4); $t.Cell(3, 2).Range.Text = "Juda uzun matn bir katakda, u qatorni bir necha marta o'raydi: " + ("so'z " * 20); $t.AutoFitBehavior(1) }
  t25_compat2010_window = { param($app, $doc) Hrm $doc; $doc.SetCompatibilityMode(14); Para $doc "Word 2010 mode, AutoFit to window" | Out-Null
    $t = NewTable $doc 5 4 1 2; Fill $t (StaffRows 5 4); $t.AutoFitBehavior(2) }
  t26_compat2010_fixed = { param($app, $doc) Hrm $doc; $doc.SetCompatibilityMode(14); Para $doc "Word 2010 mode, Insert Table defaults with a long cell" | Out-Null
    $t = NewTable $doc 5 4 1 0; Fill $t (StaffRows 5 4); $t.Cell(3, 2).Range.Text = "Juda uzun matn bir katakda: " + ("so'z " * 20) }
  t27_compat2010_percent = { param($app, $doc) Hrm $doc; $doc.SetCompatibilityMode(14); Para $doc "Word 2010 mode, table 80 % wide, columns 10/50/40 %" | Out-Null
    $t = NewTable $doc 5 3 1 0; Fill $t (StaffRows 5 3); $t.PreferredWidthType = 2; $t.PreferredWidth = 80
    $p = @(10, 50, 40); for ($c = 1; $c -le 3; $c++) { $t.Columns($c).PreferredWidthType = 2; $t.Columns($c).PreferredWidth = $p[$c - 1] } }
  t28_wider_than_page = { param($app, $doc) Hrm $doc; Para $doc "Columns 1.6 times the text width (a landscape table in a portrait page)" | Out-Null
    $t = NewTable $doc 4 6 1 0; Fill $t (StaffRows 4 5); for ($c = 1; $c -le 6; $c++) { $t.Columns($c).Width = (Cm $app 4.4) } }
  t29_wider_120 = { param($app, $doc) Hrm $doc; Para $doc "Columns 1.2 times the text width" | Out-Null
    $t = NewTable $doc 4 5 1 0; Fill $t (StaffRows 4 5); for ($c = 1; $c -le 5; $c++) { $t.Columns($c).Width = (Cm $app 4.0) } }
  t21_default_normal = { param($app, $doc) Para $doc "Default Normal style (Calibri 11, 8 pt after, 1.08 lines) table" | Out-Null
    $t = NewTable $doc 6 4 1 0; Fill $t (StaffRows 6 4) }
}

function Fill2($t, $r, $vals) { for ($c = 0; $c -lt $vals.Count; $c++) { if ($vals[$c] -ne $null) { $t.Cell($r, $c + 1).Range.Text = $vals[$c] } } }

$app, $mypid = New-Office "Word.Application" "WINWORD"
$res = [ordered]@{}
try {
  $app.Visible = $false; $app.DisplayAlerts = 0
  foreach ($k in @($cases.Keys)) {
    if ($Only -and -not ($Only.Split(",") | Where-Object { $k.StartsWith($_) })) { continue }
    $doc = $null
    try {
      $doc = $app.Documents.Add()
      & $cases[$k] $app $doc
      $f = [IO.Path]::GetFullPath((Join-Path $Out "$k.docx")); $p = [IO.Path]::GetFullPath((Join-Path $Out "$k.pdf"))
      $doc.SaveAs2($f, 12); $doc.ExportAsFixedFormat($p, 17)
      $res[$k] = "ok"
    } catch { $res[$k] = "error: " + $_.Exception.Message + " @ " + $_.InvocationInfo.ScriptLineNumber }
    finally { if ($doc) { try { $doc.Close(0) } catch {} } }
  }
} finally { Stop-Mine $app $mypid }
$res | ConvertTo-Json
