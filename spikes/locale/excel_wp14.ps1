# WP14 Excel cross-check (docs/phase3/numlocale.md section 7). Starts its OWN Excel and kills only that PID.
# Usage: powershell -File excel_wp14.ps1 -Out result.tsv [-Dir <folder with en.xlsx ru.xlsx uz-latn.xlsx uz-cyrl.xlsx>] [-Cases a.xlsx,b.xlsx]
# Runs under the current Windows culture (the dev PC is ru-RU); a uz culture needs a culture switch (owner question Q4).
param([string]$Dir, [string]$Out, [string]$Cases, [string]$Csv)
$ErrorActionPreference = 'Stop'
$before = @(Get-Process EXCEL -ErrorAction SilentlyContinue | ForEach-Object Id)
$xl = New-Object -ComObject Excel.Application
$pidX = (Get-Process EXCEL | Where-Object { $before -notcontains $_.Id } | Select-Object -First 1).Id
$lines = New-Object System.Collections.Generic.List[string]
function Add($s) { $script:lines.Add($s) }
try {
  $xl.Visible = $false; $xl.DisplayAlerts = $false
  Add "culture`t$((Get-Culture).Name)`tdecsep=$($xl.International(3))`tlistsep=$($xl.International(5))"
  Add ("thousep_codepoint`tU+{0:X4}" -f [int][char]($xl.International(4)))
  $wb = $xl.Workbooks.Add()
  $ws = $wb.Worksheets.Item(1)

  # C3 typed input: FormulaLocal parses like typing into the cell
  # month-name inputs follow the culture: ru by default, Latin or Cyrillic Uzbek when the culture is uz
  $names = @("12 окт 2026","12 октябрь 2026","окт 2026")
  if ((Get-Culture).Name -like 'uz-Latn*') { $names = @('12 okt 2026','12 oktabr 2026','okt 2026') } elseif ((Get-Culture).Name -like 'uz-Cyrl*') { $names = @("12 окт 2026","12 октябр 2026","окт 2026") }
  $typed = @('3,5','3.5','1 234,5','1.234','1,234','12.10.2026','12/10/2026','12.10.26','12.10','10.13.2026','31.02.2026','2026-10-12',
             '12.10.2026 15:00','25,5%','(1 234)','1,5E3','10:30','TRUE',$names[0],$names[1],'12 Oct 2026',$names[2],'10.2026','1 23 456','1,234,567','-1 234,50','$5,5')
  $r = 1
  foreach ($t in $typed) {
    $c = $ws.Cells.Item($r, 1)
    try {
      $c.Clear() | Out-Null
      $c.FormulaLocal = $t
      $v = $c.Value2
      Add ("typed`t{0}`ttype={1}`tvalue2={2}`tfmt={3}`ttext={4}" -f $t, $(if ($v -ne $null) { $v.GetType().Name } else { 'null' }), [string]$v, $c.NumberFormat, $c.Text)
    } catch { Add ("typed`t{0}`tERR {1}" -f $t, $_.Exception.Message) }
    $r++
  }

  # C4 formulas and C9 arrays
  $ws.Cells.Clear() | Out-Null
  foreach ($f in @('=SUM(1;2)', '=1,5+1', '=IF(1>0;"a;b";"c,d")', '=SUM(1,2)', '=ROUND(2,675;2)')) {
    $c = $ws.Cells.Item(1, 1)
    try { $c.FormulaLocal = $f; Add ("formula`t{0}`tFormula={1}`tvalue2={2}`ttext={3}" -f $f, $c.Formula, [string]$c.Value2, $c.Text) }
    catch { Add ("formula`t{0}`tERR {1}" -f $f, $_.Exception.Message) }
  }
  $c = $ws.Cells.Item(1, 1)
  $c.Formula = '=SUM({1,2;3,4})'
  Add ("array`t=SUM({1,2;3,4}) via .Formula`tFormulaLocal=" + $c.FormulaLocal)

  # C1/C6/C7/C8 displayed text: the cases workbooks (gen_locale_xlsx.py, gen_wp14_xlsx.py) hold the en-US codes
  if ($Cases) {
    foreach ($f in ($Cases -split ',')) {
      $w = $xl.Workbooks.Open($f); $s = $w.Worksheets.Item(1)
      $s.Columns.Item(2).ColumnWidth = 45
      for ($k = 1; $k -le $s.UsedRange.Rows.Count; $k++) { Add ("text`t{0}`t{1}`t{2}" -f (Split-Path $f -Leaf), $s.Cells.Item($k,1).Value2, $s.Cells.Item($k,2).Text) }
      $w.Close($false)
    }
  }

  # C5 CSV: delimiter, decimal and the group byte of SaveAs (xlCSV = 6, xlCSVUTF8 = 62). The workbook holds en-US codes
  # (COM's .NumberFormat takes the Windows culture's own separators, so it cannot be set here); -Csv is made by gen_wp14_csv.py.
  if ($Csv) {
    foreach ($kind in @(@('xlCSV', 6), @('xlCSVUTF8', 62))) {
      $w = $xl.Workbooks.Open($Csv)
      $p = Join-Path $env:TEMP ("wp14_{0}.csv" -f $kind[0])
      Remove-Item $p -ErrorAction SilentlyContinue
      $w.SaveAs($p, $kind[1])
      $w.Close($false)
      $bytes = [System.IO.File]::ReadAllBytes($p)
      Add ("csv`t{0}`thex={1}" -f $kind[0], (($bytes | ForEach-Object { '{0:X2}' -f $_ }) -join ' '))
      Remove-Item $p -ErrorAction SilentlyContinue
    }
  }
  $wb.Close($false)

  # C2 invariance: every cell's Value2, Formula and NumberFormat of the files our locales saved
  if ($Dir) {
    $dump = @{}
    foreach ($name in @('en', 'ru', 'uz-latn', 'uz-cyrl')) {
      $f = Join-Path $Dir "$name.xlsx"
      if (-not (Test-Path $f)) { continue }
      $w = $xl.Workbooks.Open($f); $s = $w.Worksheets.Item(1); $rows = New-Object System.Collections.Generic.List[string]
      foreach ($cell in $s.UsedRange.Cells) { $rows.Add(("{0}|{1}|{2}|{3}" -f $cell.Address($false, $false), [string]$cell.Value2, $cell.Formula, $cell.NumberFormat)) }
      $w.Close($false); $dump[$name] = $rows
    }
    foreach ($name in $dump.Keys) {
      $same = ($dump[$name].Count -eq $dump['en'].Count) -and (@(Compare-Object $dump[$name] $dump['en']).Count -eq 0)
      Add ("invariance`t{0} vs en`tcells={1}`tequal={2}" -f $name, $dump[$name].Count, $same)
    }
  }
  $lines | Set-Content -Encoding UTF8 $Out
} finally {
  try { $xl.Quit() } catch {}
  if ($pidX) { Stop-Process -Id $pidX -Force -ErrorAction SilentlyContinue }
}
