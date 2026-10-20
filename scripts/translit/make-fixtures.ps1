# Phase 2 WP9: Word-written .docx fixtures for the transliteration preservation tests (PLAN 8.3).
# Synthetic text only (golden vocabulary), no personal data. Drives MS Word 16 through COM with its own process:
#   powershell -ExecutionPolicy Bypass -File scripts/translit/make-fixtures.ps1 -Out <dir> [-What docx|xlsx|pptx|all]
# Only the PIDs this script started are ever killed. It refuses to start if WINWORD/EXCEL/POWERPNT is already running.
param([Parameter(Mandatory)][string]$Out, [string]$What = 'all')
$ErrorActionPreference = 'Stop'
$Out = [IO.Path]::GetFullPath($Out)
New-Item -ItemType Directory -Force $Out | Out-Null
$M = [Type]::Missing
$oq = [string][char]0x2BB   # modifier letter turned comma (oʻ, gʻ)
$ap = [string][char]0x2BC   # modifier letter apostrophe (maʼno)
$rq = [string][char]0x2019  # right single quotation mark
$lq = [string][char]0x2018  # left single quotation mark

function Start-Own($prog, $progId) {
    if (Get-Process $prog -ErrorAction SilentlyContinue) { throw "$prog is already running; refusing to touch it" }
    $app = New-Object -ComObject $progId
    $script:pids += @(Get-Process $prog | ForEach-Object { $_.Id })
    return $app
}
$script:pids = @()
function Stop-Own() {
    foreach ($i in $script:pids) { Stop-Process -Id $i -Force -ErrorAction SilentlyContinue }
    $script:pids = @()
}

# ------------------------------------------------------------------------------------------ Word
function Add-Runs($doc, $runs, $style) {
    # $runs: array of hashtables @{t=..; b=1; i=1; u=1; c=<rgb int>; s=<pt>; f=<font>; lang=<id>; hl=<color idx>}
    $end = $doc.Content.End - 1
    $start = $end
    foreach ($r in $runs) {
        $rng = $doc.Range($end, $end)
        $rng.InsertAfter($r.t)
        if ($r.b)    { $rng.Font.Bold = 1 }
        if ($r.i)    { $rng.Font.Italic = 1 }
        if ($r.u)    { $rng.Font.Underline = 1 }
        if ($r.c)    { $rng.Font.Color = $r.c }
        if ($r.s)    { $rng.Font.Size = $r.s }
        if ($r.f)    { $rng.Font.Name = $r.f }
        if ($r.lang) { $rng.LanguageID = $r.lang }
        if ($r.hl)   { $rng.HighlightColorIndex = $r.hl }
        $end = $rng.End
    }
    $para = $doc.Range($start, $end)
    if ($style) { $para.Style = $style }
    $doc.Content.InsertParagraphAfter()
    return $para
}
function T($t) { return @{ t = $t } }

function New-Doc($app) {
    $d = $app.Documents.Add()
    $d.TrackRevisions = $false
    return $d
}
function Save-Doc($doc, $name) {
    $p = Join-Path $Out $name
    if (Test-Path $p) { Remove-Item $p }
    [string]$pp = $p; $ff = 16
    $doc.SaveAs2([ref]$pp, [ref]$ff)
    $doc.Close([ref]0)
    "wrote $p"
}

function Make-Docx() {
    $app = Start-Own 'WINWORD' 'Word.Application'
    try {
        $app.Visible = $false
        $app.DisplayAlerts = 0
        $app.Options.AutoFormatAsYouTypeReplaceQuotes = $false
        $app.Options.AutoFormatAsYouTypeReplaceSymbols = $false
        $UZL = 1091; $UZC = 2115; $RU = 1049; $EN = 1033; $DE = 1031
        $red = 255; $blue = 16711680; $green = 32768

        # 1. digraphs split across runs, both scripts
        $d = New-Doc $app
        $null = Add-Runs $d @((T 'Splitting of digraphs'), @{t=' '}) 'Heading 1'
        $null = Add-Runs $d @(@{t='S'; b=1}, (T 'hahar '), @{t='C'; i=1}, (T 'hiroyli '), @{t='O'; c=$red}, (T "${oq}zbekiston "), @{t='Y'; s=16}, (T 'eti '), @{t='G'; f='Courier New'}, (T "${oq}oya"))
        $null = Add-Runs $d @((T 'sh'), @{t='a'; b=1}, (T 'hrim, c'), @{t='h'; u=1}, (T 'i'), @{t='l'; i=1}, (T 'onzor, yo'), @{t="${oq}"; b=1}, (T "lda, ma${ap}no, e${ap}lon"))
        $null = Add-Runs $d @(@{t="$([char]0x428)"; b=1}, (T "$([char]0x430)$([char]0x2060)"), @{t="$([char]0x445)$([char]0x430)$([char]0x440)"; i=1}, (T " $([char]0x427)$([char]0x438)$([char]0x440)$([char]0x43E)$([char]0x439)$([char]0x43B)$([char]0x438)"), @{t=" $([char]0x401)$([char]0x43B)"; c=$blue}, (T " $([char]0x42E)$([char]0x441)$([char]0x443)$([char]0x444)"), @{t=" $([char]0x42F)$([char]0x448)$([char]0x438)$([char]0x43B)$([char]0x430)$([char]0x440)"; s=14; f='Arial'})
        $null = Add-Runs $d @(@{t='ISHCHI '; b=1}, @{t='SHAHAR '; i=1}, @{t='CHIROYLI'; b=1; i=1; c=$green}, (T ' Shohrux, Chilonzor, Yoqub, YOSHLAR'))
        Save-Doc $d '01-split-runs.docx'

        # 2. hyperlinks
        $d = New-Doc $app
        $null = Add-Runs $d @((T 'Havola: '))
        $r = $d.Range($d.Content.End - 1, $d.Content.End - 1)
        $null = $d.Hyperlinks.Add($r, 'https://example.org/shahar', $M, 'Test tip', "Toshkent shahri")
        $null = Add-Runs $d @((T ' va '))
        $r = $d.Range($d.Content.End - 2, $d.Content.End - 2)
        $d.Bookmarks.Add('joy', $d.Range(0, 5)) | Out-Null
        $null = $d.Hyperlinks.Add($r, '', 'joy', 'Internal', "bosh joy")
        $null = Add-Runs $d @((T ' yoki '))
        $r = $d.Range($d.Content.End - 2, $d.Content.End - 2)
        $null = $d.Hyperlinks.Add($r, 'mailto:test@example.org', $M, $M, 'test@example.org')
        $null = Add-Runs $d @(@{t='Matn '; b=1}, (T 'oddiy '), @{t='yana havola'; i=1})
        $r = $d.Range($d.Content.End - 7, $d.Content.End - 1)
        Save-Doc $d '02-hyperlinks.docx'

        # 3. fields
        $d = New-Doc $app
        $null = Add-Runs $d @((T 'Mundarija')) 'Heading 1'
        $null = Add-Runs $d @((T 'Birinchi bob')) 'Heading 1'
        $null = Add-Runs $d @((T 'Ikkinchi bo'), @{t='b'; b=1}) 'Heading 2'
        $null = Add-Runs $d @((T 'Sahifa: '))
        $r = $d.Range($d.Content.End - 2, $d.Content.End - 2); $null = $d.Fields.Add($r, -1, 'PAGE', $false)
        $null = Add-Runs $d @((T ' sana: '))
        $r = $d.Range($d.Content.End - 2, $d.Content.End - 2); $null = $d.Fields.Add($r, -1, 'DATE \@ "yyyy"', $false)
        $null = Add-Runs $d @((T ' havola: '))
        $d.Bookmarks.Add('belgi', $d.Range(10, 20)) | Out-Null
        $r = $d.Range($d.Content.End - 2, $d.Content.End - 2); $null = $d.Fields.Add($r, -1, 'REF belgi \h', $false)
        $null = Add-Runs $d @((T ' ism: '))
        $r = $d.Range($d.Content.End - 2, $d.Content.End - 2); $null = $d.Fields.Add($r, 59, 'ism', $false)
        $null = Add-Runs $d @((T ' shart: '))
        $r = $d.Range($d.Content.End - 2, $d.Content.End - 2); $null = $d.Fields.Add($r, -1, 'IF 1 = 1 "shahar" "qishloq"', $false)
        $r = $d.Range(0, 0)
        $null = $d.TablesOfContents.Add($r, $true, 1, 2)
        $d.Fields.Update() | Out-Null
        Save-Doc $d '03-fields.docx'

        # 4. tables, nested
        $d = New-Doc $app
        $null = Add-Runs $d @((T 'Jadval'))
        $r = $d.Range($d.Content.End - 1, $d.Content.End - 1)
        $t = $d.Tables.Add($r, 3, 3)
        $t.Borders.Enable = 1
        $t.Cell(1,1).Range.Text = 'Shahar'; $t.Cell(1,1).Range.Font.Bold = 1
        $t.Cell(1,2).Range.Text = 'Yoshi'
        $t.Cell(1,3).Range.Text = 'Chiqish'; $t.Cell(1,3).Shading.BackgroundPatternColor = 65535
        $t.Cell(2,1).Range.Text = "Toshkent"; $t.Cell(2,1).Range.Font.Italic = 1
        $t.Cell(2,2).Range.Text = '25'
        $t.Cell(2,3).Range.Text = "Yo${oq}q"; $t.Cell(2,3).Range.Font.Color = 255
        $t.Cell(3,1).Range.Text = 'Shu'; $t.Cell(3,1).Range.Characters.Item(1).Font.Bold = 1
        $t.Cell(3,3).Range.Text = 'Outer'
        $n = $d.Tables.Add($t.Cell(3,2).Range, 2, 2)
        $n.Cell(1,1).Range.Text = "Ichki"; $n.Cell(1,2).Range.Text = "jadval"
        $n.Cell(2,1).Range.Text = "Gu${oq}zal"; $n.Cell(2,2).Range.Text = 'chiroyli'; $n.Cell(2,2).Range.Font.Bold = 1
        $t.Cell(1,1).Merge($t.Cell(1,2))
        Save-Doc $d '04-tables.docx'

        # 5. header, footer, footnote, endnote, comment, text box
        $d = New-Doc $app
        $null = Add-Runs $d @((T 'Asosiy matn '), @{t='sha'; b=1}, (T 'hrimiz.'))
        $d.Sections(1).Headers(1).Range.Text = "Sahifa ${oq}shapka"
        $d.Sections(1).Headers(1).Range.Font.Italic = 1
        $d.Sections(1).Footers(1).Range.Text = 'Pastki chiziq choy'
        $null = $d.Footnotes.Add($d.Range(7, 12), $M, 'Izoh: shahar tashqarisi')
        $null = $d.Endnotes.Add($d.Range(13, 17), $M, "Oxirgi izoh, ${oq}zbek")
        $null = $d.Comments.Add($d.Range(0, 6), 'Sharh matni chiroyli')
        $d.Content.InsertParagraphAfter()
        $r = $d.Range($d.Content.End - 1, $d.Content.End - 1)
        $sh = $d.Shapes.AddTextbox(1, 100, 300, 250, 60, $r)
        $sh.TextFrame.TextRange.Text = "Matn qutisi: shahar va ${oq}rmon"
        foreach ($k in 1..4) { $sh.TextFrame.TextRange.Characters.Item($k).Font.Bold = 1 }
        Save-Doc $d '05-parts.docx'

        # 6a. existing tracked changes, recording off in the file
        $d = New-Doc $app
        $null = Add-Runs $d @((T 'Birinchi gap shahar haqida. '), (T 'Ikkinchi gap chiroyli. '), (T 'Uchinchi gap.'))
        $d.TrackRevisions = $true
        $app.UserName = 'Test User'
        $d.Range(0, 8).Text = 'Yangi'
        $d.Range(20, 32).Delete() | Out-Null
        $r = $d.Range($d.Content.End - 1, $d.Content.End - 1); $r.InsertAfter(' Qoshilgan matn shoshilinch.')
        $d.TrackRevisions = $false
        Save-Doc $d '06-tracked-off.docx'

        # 6b. the same, with the recording switch left on in the file
        $d = New-Doc $app
        $null = Add-Runs $d @((T 'Birinchi gap shahar haqida. '), (T 'Ikkinchi gap chiroyli. '), (T 'Uchinchi gap.'))
        $d.TrackRevisions = $true
        $d.Range(0, 8).Text = 'Yangi'
        $r = $d.Range($d.Content.End - 1, $d.Content.End - 1); $r.InsertAfter(' Qoshilgan matn shoshilinch.')
        Save-Doc $d '06-tracked-on.docx'

        # 7. language tags
        $d = New-Doc $app
        $null = Add-Runs $d @(@{t='Toshkent shahri chiroyli. '; lang=$UZL}, @{t='Shahar haqida. '; lang=$EN}, @{t="$([char]0x428)$([char]0x430)$([char]0x445)$([char]0x430)$([char]0x440) $([char]0x43A)$([char]0x440)$([char]0x430)$([char]0x441)$([char]0x438)$([char]0x432)$([char]0x44B)$([char]0x439)."; lang=$RU}, @{t=' Die Schule und Chor.'; lang=$DE}, @{t=" $([char]0x428)$([char]0x430)$([char]0x2060)$([char]0x445)$([char]0x430)$([char]0x440) $([char]0x438)$([char]0x448)$([char]0x447)$([char]0x438)"; lang=$UZC})
        $null = Add-Runs $d @(@{t='The church and the school. '; lang=$EN; b=1}, @{t='Chicago shahri. '; lang=$EN}, @{t='Chiroyli shahar'; lang=$UZL; i=1})
        Save-Doc $d '07-languages.docx'

        # 8. locked content control, apostrophes, URLs, codes
        $d = New-Doc $app
        $null = Add-Runs $d @((T 'Boshida '))
        $r = $d.Range($d.Content.End - 1, $d.Content.End - 1)
        $cc = $d.ContentControls.Add(1, $r)
        $cc.Range.Text = 'qulf shahar'
        $cc.LockContentControl = $true
        $null = Add-Runs $d @((T " va ${oq}zbek, o'zbek, o${rq}zbek, o${lq}zbek, o${ap}zbek; ma${ap}no, ma'no, ma${rq}no, ma${lq}no."))
        $null = Add-Runs $d @((T 'https://example.org/shahar/chiroyli test@example.org PF-1234 XXI asr, ID 12345, shahar@gmail.com'))
        $null = Add-Runs $d @(@{t='Ish ho'; b=1}, (T 'rdiq: 12-son, 3-chi, 2025-yil, 15:30.'))
        Save-Doc $d '08-controls-codes.docx'
    }
    finally {
        try { $app.Quit(0) } catch {}
        Start-Sleep -Seconds 2
        Stop-Own
    }
}

# ------------------------------------------------------------------------------------------ Excel
function Make-Xlsx() {
    $app = Start-Own 'EXCEL' 'Excel.Application'
    try {
        $app.Visible = $false; $app.DisplayAlerts = $false
        # 1. text, numbers, formulas with text literals, styles
        $wb = $app.Workbooks.Add(); $ws = $wb.Worksheets.Item(1); $ws.Name = 'Hisobot'
        $ws.Range('A1').Value2 = 'Shahar'; $ws.Range('B1').Value2 = "Yoshi"; $ws.Range('C1').Value2 = 'Narx'; $ws.Range('D1').Value2 = 'Holat'
        $ws.Range('A1:D1').Font.Bold = $true; $ws.Range('A1:D1').Interior.Color = 15773696; $ws.Range('A1:D1').Font.Color = 16777215
        $names = @('Toshkent', 'Chirchiq', "Qo${oq}qon", 'Shahrisabz', "Yo${oq}lchi")
        for ($i = 0; $i -lt 5; $i++) {
            $row = $i + 2
            $ws.Cells.Item($row, 1).Value2 = $names[$i]
            $ws.Cells.Item($row, 2).Value2 = [double](20 + $i)
            $ws.Cells.Item($row, 3).Value2 = [double](1234.5 * ($i + 1))
            $ws.Cells.Item($row, 4).Formula = "=IF(B$row>21,""Chiroyli"",""Yo${oq}q"")"
        }
        try { $ws.Range('C2:C6').NumberFormatLocal = '0,00' } catch { Write-Warning 'NumberFormat not set' }
        $ws.Range('A2:A3').Font.Italic = $true; $ws.Range('A4').Font.Color = 255; $ws.Range('A5').Font.Name = 'Courier New'; $ws.Range('A5').Font.Size = 14
        $ws.Range('A2:D6').Borders.LineStyle = 1
        $ws.Range('A8').Value2 = 'Shahar nomi: '; $ws.Range('B8').Formula = '="Natija: "&A2&" shahri"'
        $ws.Range('A9:C9').Merge(); $ws.Range('A9').Value2 = "Birlashgan katak, ${oq}zbek"; $ws.Range('A9').HorizontalAlignment = -4108
        $ws.Range('A10').Value2 = "Yo${oq}l"; $ws.Range('A10').WrapText = $true; $ws.Rows.Item(10).RowHeight = 30
        $ws.Range('E2').Value2 = 'Matn'; $ws.Range('E2').Interior.Color = 65535
        $p = Join-Path $Out '01-styles.xlsx'; if (Test-Path $p) { Remove-Item $p }
        $wb.SaveAs($p, 51); $wb.Close($false); "wrote $p"

        # 2. a table with headers, rich-text cells
        $wb = $app.Workbooks.Add(); $ws = $wb.Worksheets.Item(1); $ws.Name = 'Jadval'
        $ws.Range('A1').Value2 = 'Ism'; $ws.Range('B1').Value2 = "Lavozim"; $ws.Range('C1').Value2 = 'Maosh'
        $rows = @(@('Shohrux', "Yo${oq}lboshchi", 100), @('Chingiz', 'Ishchi', 200), @('Yoqub', "Sho${oq}pir", 300))
        for ($i = 0; $i -lt 3; $i++) { for ($j = 0; $j -lt 3; $j++) { $ws.Cells.Item($i + 2, $j + 1).Formula = [string]$rows[$i][$j] } }
        $lo = $ws.ListObjects.Add(1, $ws.Range('A1:C4'), $null, 1); $lo.Name = 'Xodimlar'; $lo.TableStyle = 'TableStyleMedium2'
        $ws.Range('E1').Value2 = 'Shahar chiroyli'
        $ws.Range('E1').Characters(1, 3).Font.Bold = $true
        $ws.Range('E1').Characters(7, 4).Font.Color = 255
        $ws.Range('E2').Value2 = "Cho${oq}l shoshilinch"
        $ws.Range('E2').Characters(1, 1).Font.Italic = $true
        $ws.Range('E3').Formula = '=COUNTA(Xodimlar[Ism])&" ta xodim"'
        $p = Join-Path $Out '02-table-rich.xlsx'; if (Test-Path $p) { Remove-Item $p }
        $wb.SaveAs($p, 51); $wb.Close($false); "wrote $p"
    }
    finally { try { $app.Quit() } catch {}; Start-Sleep -Seconds 2; Stop-Own }
}

# ------------------------------------------------------------------------------------------ PowerPoint
function Make-Pptx() {
    $app = Start-Own 'POWERPNT' 'PowerPoint.Application'
    try {
        # 1. text frames, group, table, notes, slide-number field, hyperlink
        $pr = $app.Presentations.Add(0)
        $s = $pr.Slides.Add(1, 12)   # ppLayoutBlank
        $tb = $s.Shapes.AddTextbox(1, 40, 30, 500, 100)
        $tr = $tb.TextFrame.TextRange; $tr.Text = "Shahar chiroyli va ${oq}zbek"
        $tr.Characters(1, 1).Font.Bold = -1       # S | hahar
        $tr.Characters(9, 3).Font.Italic = -1
        $tr.Characters(9, 3).Font.Color.RGB = 255
        $tr.Characters(13, 5).Font.Size = 28
        $null = $tr.InsertAfter("`rIkkinchi abzats: chi").Font.Name
        $tb.TextFrame.TextRange.Paragraphs(2).Characters(18, 2).Font.Bold = -1
        $tb.TextFrame.TextRange.Paragraphs(2).Characters(1, 5).ActionSettings(1).Hyperlink.Address = 'https://example.org/shahar'
        $a = $s.Shapes.AddShape(1, 40, 200, 150, 60); $a.TextFrame.TextRange.Text = 'Shoshilinch'
        $b = $s.Shapes.AddShape(9, 220, 200, 150, 60); $b.TextFrame.TextRange.Text = "Cho${oq}l"
        $b.TextFrame.TextRange.Characters(1, 1).Font.Bold = -1
        $g = $s.Shapes.Range(@($a.Name, $b.Name)).Group()
        $tbl = $s.Shapes.AddTable(2, 2, 40, 300, 400, 80)
        $tbl.Table.Cell(1, 1).Shape.TextFrame.TextRange.Text = 'Shahar'
        $tbl.Table.Cell(1, 2).Shape.TextFrame.TextRange.Text = "Yo${oq}q"
        $tbl.Table.Cell(2, 1).Shape.TextFrame.TextRange.Text = 'Chiroyli'
        $tbl.Table.Cell(2, 1).Shape.TextFrame.TextRange.Characters(1, 1).Font.Bold = -1
        $tbl.Table.Cell(2, 2).Shape.TextFrame.TextRange.Text = 'Qizil'
        $s.NotesPage.Shapes.Placeholders(2).TextFrame.TextRange.Text = "Eslatma: shahar va ${oq}rmon haqida"
        $s.NotesPage.Shapes.Placeholders(2).TextFrame.TextRange.Characters(10, 5).Font.Bold = -1
        $s.HeadersFooters.SlideNumber.Visible = -1
        $p = Join-Path $Out '01-shapes-notes.pptx'; if (Test-Path $p) { Remove-Item $p }
        $pr.SaveAs($p, 24); $pr.Close(); "wrote $p"

        # 2. language tags, mixed runs on two slides
        $pr = $app.Presentations.Add(0)
        $s = $pr.Slides.Add(1, 2)   # title and content
        $s.Shapes.Item(1).TextFrame.TextRange.Text = 'Sarlavha: shahar'
        $s.Shapes.Item(1).TextFrame.TextRange.Characters(1, 1).Font.Bold = -1
        $body = $s.Shapes.Item(2).TextFrame.TextRange
        $body.Text = "Birinchi band chiroyli`rVtoroy punkt na russkom: $([char]0x428)$([char]0x430)$([char]0x445)$([char]0x430)$([char]0x440)`rThird line in English: church"
        $body.Paragraphs(1).LanguageID = 1091
        $body.Paragraphs(2).LanguageID = 1049
        $body.Paragraphs(3).LanguageID = 1033
        $body.Paragraphs(1).Characters(1, 1).Font.Italic = -1
        $body.Paragraphs(1).Characters(1, 1).Font.Color.RGB = 16711680
        $s2 = $pr.Slides.Add(2, 12)
        $t2 = $s2.Shapes.AddTextbox(1, 40, 40, 500, 60); $t2.TextFrame.TextRange.Text = "ISHCHI SHAHAR yo${oq}lda"
        $t2.TextFrame.TextRange.Characters(1, 2).Font.Bold = -1
        $t2.TextFrame.TextRange.Characters(3, 2).Font.Underline = -1
        $s2.NotesPage.Shapes.Placeholders(2).TextFrame.TextRange.Text = 'Notes: chiroyli shahar'
        $p = Join-Path $Out '02-languages.pptx'; if (Test-Path $p) { Remove-Item $p }
        $pr.SaveAs($p, 24); $pr.Close(); "wrote $p"
    }
    finally { try { $app.Quit() } catch {}; Start-Sleep -Seconds 2; Stop-Own }
}

if ($What -in 'docx', 'all') { Make-Docx }
if ($What -in 'xlsx', 'all') { Make-Xlsx }
if ($What -in 'pptx', 'all') { Make-Pptx }
