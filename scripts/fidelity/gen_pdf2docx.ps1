# Fixture generator 2/2: the PLAN 3.4 PDF -> DOCX fixtures, built by construction in Word 16 so the truth is known.
# HRM style: Times New Roman 14, justified, EXACT line spacing, first-line indent, numbered list, table with set column
# widths, header and footer with a PAGE field. Writes <name>.docx, <name>.pdf and <name>.truth.json per fixture.
#   p2d_latn / p2d_cyrl / p2d_ru : the order (uz-Latn, uz-Cyrl, ru)
#   p2d_overimage                 : a stamp picture behind the text plus a text box drawn over it (known words STAMPWORD, SIGNWORD)
#   p2d_letter                    : a letter with a numbered table, four fonts and a stamp over the signature (positioned output)
#   p2d_widths                    : table with known column widths and merged cells
# ASCII-only source; the texts come from content.json at run time. Synthetic only.
param([Parameter(Mandatory)][string]$Dir, [string]$Png = "")
$ErrorActionPreference = "Stop"
. "$PSScriptRoot\com.ps1"
$utf8 = New-Object Text.UTF8Encoding $false
$T = [IO.File]::ReadAllText("$PSScriptRoot\content.json", [Text.Encoding]::UTF8) | ConvertFrom-Json
if (-not $Png) { $Png = "$Dir\sample.png" }
$ORDER_W = @(30, 190, 130, 90)

function Truth($name, $obj) { [IO.File]::WriteAllText("$Dir\$name.truth.json", ($obj | ConvertTo-Json -Depth 5), $utf8) }
function Setup-Doc($wd) {
    $d = $wd.Documents.Add()
    $ps = $d.PageSetup; $ps.PaperSize = 7; $ps.LeftMargin = 85; $ps.RightMargin = 42.5; $ps.TopMargin = 56.7; $ps.BottomMargin = 56.7
    $n = $d.Styles.Item(-1); $n.Font.Name = "Times New Roman"; $n.Font.Size = 14
    $pf = $n.ParagraphFormat; $pf.Alignment = 3; $pf.LineSpacingRule = 4; $pf.LineSpacing = 18; $pf.FirstLineIndent = 35.4; $pf.SpaceAfter = 0; $pf.SpaceBefore = 0
    return $d
}
function Add-HeaderFooter($d, $htext) {
    $sec = $d.Sections.Item(1)
    $h = $sec.Headers.Item(1).Range; $h.Text = $htext; $h.Font.Name = "Times New Roman"; $h.Font.Size = 10
    $h.ParagraphFormat.Alignment = 1; $h.ParagraphFormat.FirstLineIndent = 0; $h.ParagraphFormat.LineSpacingRule = 0
    $f = $sec.Footers.Item(1).Range; $f.Text = "- "; $f.Font.Name = "Times New Roman"; $f.Font.Size = 10
    $f.ParagraphFormat.Alignment = 1; $f.ParagraphFormat.FirstLineIndent = 0; $f.ParagraphFormat.LineSpacingRule = 0
    $e = $sec.Footers.Item(1).Range; $e.Collapse(0); [void]$d.Fields.Add($e, 33)
    $e = $sec.Footers.Item(1).Range; $e.InsertAfter(" -")
}
function Para($d, $text, $align, $indent, $bold) {   # Range-based (Selection.TypeText hung Word); $null = keep the Normal style (setting FirstLineIndent on a long justified paragraph hung it)
    $r = $d.Range($d.Content.End - 1, $d.Content.End - 1)
    $r.Text = $text
    $r.Font.Bold = $bold
    if ($align -eq $null) { $r.ParagraphFormat.Reset() }   # back to the Normal style (justified, first-line indent)
    if ($align -ne $null) { $r.ParagraphFormat.Alignment = $align }; if ($indent -ne $null) { $r.ParagraphFormat.FirstLineIndent = $indent }
    $r.InsertParagraphAfter()
}
function Save-Fixture($wd, $d, $name) {
    $d.SaveAs2("$Dir\$name.docx", 16); $d.ExportAsFixedFormat("$Dir\$name.pdf", 17); $d.Close(0)
}

$wd, $wdp = New-Office "Word.Application" "WINWORD"; $wd.Visible = $false; $wd.DisplayAlerts = 0
$wd.ScreenUpdating = $false; $wd.Options.CheckSpellingAsYouType = $false; $wd.Options.CheckGrammarAsYouType = $false
$wd.Options.AutoFormatAsYouTypeApplyNumberedLists = $false; $wd.Options.AutoFormatAsYouTypeApplyBulletedLists = $false
try {
    # ---- the order in three languages
    $langs = @(
        @{ n = "latn"; t = $T.title_lat; pl = $T.place_lat; hd = $T.head_lat; ps = $T.p_lat; li = $T.p_lat; hdr = $T.title_lat + " / HRM" },
        @{ n = "cyrl"; t = $T.title_cyr; pl = $T.place_cyr; hd = $T.head_cyr; ps = $T.p_cyr; li = $T.list_cyr; hdr = $T.title_cyr + " / HRM" },
        @{ n = "ru"; t = $T.title_ru; pl = $T.place_ru; hd = $T.head_ru; ps = $T.p_ru; li = $T.p_ru; hdr = $T.title_ru + " / HRM" })
    foreach ($L in $langs) {
        $d = Setup-Doc $wd
        
        Para $d $L.t 1 0 1; Para $d $L.pl 1 0 0; Para $d $L.hd 1 0 1
        for ($k = 0; $k -lt 4; $k++) { foreach ($p in $L.ps) { Para $d $p $null $null 0 } }
        $a = $d.Content.End - 1
        foreach ($i in $L.li) { Para $d $i 0 0 0 }
        $d.Range($a, $d.Content.End - 1).ListFormat.ApplyNumberDefault()
        $d.Paragraphs.Last.Range.ListFormat.RemoveNumbers()
        Para $d $L.ps[0] $null $null 0
        $tb = $d.Tables.Add($d.Range($d.Content.End - 1, $d.Content.End - 1), 4, 4); $tb.Borders.Enable = $true; $tb.AllowAutoFit = $false
        $tb.Range.ParagraphFormat.FirstLineIndent = 0; $tb.Range.ParagraphFormat.Alignment = 0; $tb.Range.ParagraphFormat.LineSpacingRule = 0; $tb.Range.Font.Size = 12
        for ($c = 1; $c -le 4; $c++) { $tb.Columns.Item($c).Width = $ORDER_W[$c - 1]; $tb.Cell(1, $c).Range.Text = $T.th[$c - 1]; $tb.Cell(1, $c).Range.Font.Bold = $true }
        for ($r = 2; $r -le 4; $r++) {
            $tb.Cell($r, 1).Range.Text = [string]($r - 1); $tb.Cell($r, 2).Range.Text = $T.names[$r - 2]
            $tb.Cell($r, 3).Range.Text = $T.pos[$r - 2]; $tb.Cell($r, 4).Range.Text = ("{0:N0}" -f (1500000 + $r * 250000))
        }
        Para $d $L.ps[1] $null $null 0
        Add-HeaderFooter $d $L.hdr
        Truth "p2d_$($L.n)" @{ lang = $L.n; kind = "order"; known_words = @(); col_widths_pt = $ORDER_W }
        Save-Fixture $wd $d "p2d_$($L.n)"
    }
    # ---- text over a picture: picture BEHIND the text, a transparent text box in front of it
    $d = Setup-Doc $wd
    Para $d $T.title_lat 1 0 1
    $anchor = $d.Paragraphs.Item(1).Range
    $pic = $d.Shapes.AddPicture($Png, $false, $true, 150, 230, 240, 135, $anchor)
    $pic.WrapFormat.Type = 5; $pic.RelativeHorizontalPosition = 1; $pic.RelativeVerticalPosition = 1; $pic.Left = 150; $pic.Top = 230
    for ($k = 0; $k -lt 3; $k++) { foreach ($p in $T.p_lat) { Para $d $p $null $null 0 } }
    Para $d "SIGNWORD" 0 0 1
    $tbx = $d.Shapes.AddTextbox(1, 190, 280, 160, 40, $anchor)
    $tbx.TextFrame.TextRange.Text = "STAMPWORD"; $tbx.TextFrame.TextRange.Font.Name = "Times New Roman"; $tbx.TextFrame.TextRange.Font.Size = 16
    $tbx.Fill.Visible = 0; $tbx.Line.Visible = 0; $tbx.WrapFormat.Type = 3; $tbx.RelativeHorizontalPosition = 1; $tbx.RelativeVerticalPosition = 1; $tbx.Left = 190; $tbx.Top = 280
    Add-HeaderFooter $d "HRM / overimage"
    Truth "p2d_overimage" @{ lang = "latn"; kind = "overimage"; known_words = @("STAMPWORD", "SIGNWORD"); col_widths_pt = @() }
    Save-Fixture $wd $d "p2d_overimage"
    # ---- a letter with a stamp (owner report 2026-10-29: Word output moved the text): a two-row centred title, a numbered
    # table, paragraphs in three more fonts, a two-row footer and a stamp picture over the signature, so the native engine
    # keeps the page as positioned text boxes. fid/positions.py measures where the words land.
    $d = Setup-Doc $wd
    Para $d ($T.title_lat + " " + $T.head_lat + " " + $T.place_lat) 1 0 1
    $tb = $d.Tables.Add($d.Range($d.Content.End - 1, $d.Content.End - 1), 11, 2); $tb.Borders.Enable = $true; $tb.AllowAutoFit = $false
    $tb.Range.ParagraphFormat.FirstLineIndent = 0; $tb.Range.ParagraphFormat.Alignment = 0; $tb.Range.ParagraphFormat.LineSpacingRule = 0; $tb.Range.Font.Size = 12
    $tb.Columns.Item(1).Width = 30; $tb.Columns.Item(2).Width = 300
    for ($r = 1; $r -le 11; $r++) { $tb.Cell($r, 1).Range.Text = [string]$r; $tb.Cell($r, 2).Range.Text = '"' + $T.names[$r % 5] + '" AJ' }
    $fonts = @(@("Calibri", 11, 3), @("Verdana", 10, 0), @("Arial", 12, 2))
    for ($k = 0; $k -lt 3; $k++) {
        Para $d $T.p_lat[$k] $fonts[$k][2] 0 0
        $r = $d.Paragraphs.Item($d.Paragraphs.Count - 1).Range; $r.Font.Name = $fonts[$k][0]; $r.Font.Size = $fonts[$k][1]
    }
    Para $d "SIGNWORD" 0 0 1
    $pic = $d.Shapes.AddPicture($Png, $false, $true, 60, 0, 150, 85, $d.Paragraphs.Item($d.Paragraphs.Count - 1).Range)
    $pic.WrapFormat.Type = 5; $pic.RelativeVerticalPosition = 2; $pic.Top = -30
    $ft = $d.Paragraphs.Item($d.Paragraphs.Count).Range; $ft.Text = $T.dept[0] + " " + $T.place_lat; $ft.Font.Size = 9
    $ft.ParagraphFormat.Alignment = 0; $ft.ParagraphFormat.FirstLineIndent = 0; $ft.ParagraphFormat.RightIndent = 300
    Truth "p2d_letter" @{ lang = "latn"; kind = "letter"; known_words = @("SIGNWORD"); col_widths_pt = @(30, 300) }
    Save-Fixture $wd $d "p2d_letter"
    # ---- table widths and merged cells (known w:gridCol)
    $W5 = @(24, 70, 150, 110, 60)
    $d = Setup-Doc $wd
    Para $d $T.title_cyr 1 0 1; Para $d $T.head_cyr 1 0 0
    $tb = $d.Tables.Add($d.Range($d.Content.End - 1, $d.Content.End - 1), 6, 5); $tb.Borders.Enable = $true; $tb.AllowAutoFit = $false
    $tb.Range.ParagraphFormat.FirstLineIndent = 0; $tb.Range.ParagraphFormat.Alignment = 0; $tb.Range.ParagraphFormat.LineSpacingRule = 0; $tb.Range.Font.Size = 12
    for ($c = 1; $c -le 5; $c++) { $tb.Columns.Item($c).Width = $W5[$c - 1] }
    for ($r = 1; $r -le 6; $r++) { for ($c = 1; $c -le 5; $c++) { $tb.Cell($r, $c).Range.Text = ("{0}.{1} " -f $r, $c) + $T.names[($r + $c) % 5] } }
    $tb.Cell(1, 2).Merge($tb.Cell(1, 3)); $tb.Cell(3, 4).Merge($tb.Cell(4, 4))
    Para $d $T.p_cyr[0] $null $null 0
    Truth "p2d_widths" @{ lang = "cyrl"; kind = "widths"; known_words = @(); col_widths_pt = $W5 }
    Save-Fixture $wd $d "p2d_widths"
} finally { Stop-Mine $wd $wdp }
Get-ChildItem $Dir -Filter "p2d_*" | Select-Object Name, Length | Format-Table -AutoSize | Out-String | Write-Host
