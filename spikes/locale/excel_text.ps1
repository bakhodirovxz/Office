# Dump Excel's displayed .Text per cell (ground truth for the current Windows culture) + typed-input probes.
# Usage: powershell -File excel_text.ps1 -In cases.xlsx -Out excel_text.tsv
# Starts its own Excel instance and kills only that PID. Run only with >= 2 GB free RAM.
param([string]$In, [string]$Out)
$ErrorActionPreference = 'Stop'
$before = @(Get-Process EXCEL -ErrorAction SilentlyContinue | ForEach-Object Id)
if ($before.Count) { throw "EXCEL already running (PIDs $before); refusing" }
$xl = New-Object -ComObject Excel.Application
$pidX = (Get-Process EXCEL | Select-Object -First 1).Id
try {
  $xl.Visible = $false; $xl.DisplayAlerts = $false
  $wb = $xl.Workbooks.Open((Resolve-Path $In).Path)
  $ws = $wb.Worksheets.Item(1)
  $lines = @("culture`t$((Get-Culture).Name)", "decsep`t$($xl.International(3))", "thousep`t$($xl.International(4))", "listsep`t$($xl.International(5))")
  $n = $ws.UsedRange.Rows.Count
  for ($r = 1; $r -le $n; $r++) {
    $lines += "{0}`t{1}" -f $ws.Cells.Item($r,1).Value2, $ws.Cells.Item($r,2).Text
  }
  # typed-input probes in a scratch sheet
  $t = $wb.Worksheets.Add()
  # FormulaLocal parses like typing into the cell UI (locale decimal/list separators, locale function names).
  $sum = ([string][char]0x0421)+[char]0x0423+[char]0x041C+[char]0x041C   # "SUMM" in Cyrillic = Russian SUM
  $probes = @(
    @('SUM(1,2) via .Formula (en-US storage)', 'Formula', '=SUM(1,2)'),
    @('ru SUM(1;2) via .FormulaLocal', 'FormulaLocal', "=$sum(1;2)"),
    @('en SUM(1;2) via .FormulaLocal', 'FormulaLocal', '=SUM(1;2)'),
    @('typed 3,5', 'FormulaLocal', '3,5'),
    @('typed 3.5', 'FormulaLocal', '3.5'),
    @('typed 12.10.2026', 'FormulaLocal', '12.10.2026'),
    @('typed 2026-10-12', 'FormulaLocal', '2026-10-12'),
    @('typed 1 234,5', 'FormulaLocal', '1 234,5'),
    @('typed =1,5+1', 'FormulaLocal', '=1,5+1')
  )
  $row = 1
  foreach ($p in $probes) {
    $c = $t.Cells.Item($row,1)
    try { $c.($p[1]) = $p[2]; $lines += "probe`t$($p[0])`ttext=$($c.Text)`ttype=$(if($c.Value2 -ne $null){$c.Value2.GetType().Name})`tfmt=$($c.NumberFormat)`tFormula=$($c.Formula)" }
    catch { $lines += "probe`t$($p[0])`tERR $($_.Exception.Message)" }
    $row++
  }
  $wb.Close($false)
  $lines | Set-Content -Encoding UTF8 $Out
} finally {
  try { $xl.Quit() } catch {}
  Stop-Process -Id $pidX -Force -ErrorAction SilentlyContinue
}
