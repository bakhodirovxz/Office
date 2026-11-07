# R6 inputs: Excel and PowerPoint re-save the generated files (Office-authored inputs) and Word/Excel export PDFs.
# Only the Office PID this script starts is ever killed (com.ps1). Usage: resave.ps1 <dir> [<extra docx>]
param([Parameter(Mandatory)][string]$Dir, [string]$Docx = "")
$ErrorActionPreference = "Stop"
. "$PSScriptRoot\..\com.ps1"
$x, $xp = New-Office "Excel.Application" "EXCEL"
try {
  $x.DisplayAlerts = $false
  $wb = $x.Workbooks.Open("$Dir\features.xlsx"); $wb.SaveAs("$Dir\features-excel.xlsx", 51); $wb.ExportAsFixedFormat(0, "$Dir\features-excel.pdf"); $wb.Close($false)
} finally { Stop-Mine $x $xp }
$p, $pp = New-Office "PowerPoint.Application" "POWERPNT"
try {
  $pr = $p.Presentations.Open("$Dir\features.pptx", 0, 0, 0); $pr.SaveAs("$Dir\features-ppt.pptx", 24); $pr.SaveAs("$Dir\features-ppt.pdf", 32); $pr.Close()
} finally { Stop-Mine $p $pp }
if ($Docx) {
  $w, $wp = New-Office "Word.Application" "WINWORD"
  try { $w.DisplayAlerts = 0; $d = $w.Documents.Open($Docx, $false, $true); $d.ExportAsFixedFormat("$Dir\letter-word.pdf", 17); $d.Close(0) } finally { Stop-Mine $w $wp }
}
"resaved"
