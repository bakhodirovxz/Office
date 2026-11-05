<#
.SYNOPSIS
  Per-user registration of the shared Office add-in (docs/ms-addin/PLAN.md section 6): one COM class, one Addins key
  per Office app. Writes HKCU only, in both the 64-bit and the 32-bit registry view, so 32-bit and 64-bit Office
  (MSI or Click-to-Run, 2010 and later) find it. unregister.ps1 removes exactly what this writes.
.PARAMETER Dir
  The folder holding UzTranslit.OfficeAddin.dll, x86\, x64\ and brand-name.txt (the output of build.sh).
.PARAMETER Apps
  Office apps to load into. Default: Word, Excel, PowerPoint.
#>
param(
  [Parameter(Mandatory = $true)][string]$Dir,
  [string[]]$Apps = @('Word', 'Excel', 'PowerPoint')
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'addin-ids.ps1')

$dll = (Resolve-Path (Join-Path $Dir 'UzTranslit.OfficeAddin.dll')).Path
foreach ($a in 'x86', 'x64') { if (-not (Test-Path (Join-Path $Dir "$a\uz_translit_ffi.dll"))) { throw "missing $a\uz_translit_ffi.dll in $Dir" } }
$brand = (Get-Content (Join-Path $Dir 'brand-name.txt') -Raw -Encoding UTF8).Trim()
$asm = [Reflection.AssemblyName]::GetAssemblyName($dll).FullName
$codeBase = ([Uri]$dll).AbsoluteUri

foreach ($view in 'Registry64', 'Registry32') {
  $hkcu = [Microsoft.Win32.RegistryKey]::OpenBaseKey('CurrentUser', $view)
  try {
    $k = $hkcu.CreateSubKey("Software\Classes\CLSID\$Clsid")
    $k.SetValue('', $ClassName)
    $k.CreateSubKey('ProgId').SetValue('', $ProgId)
    $k.CreateSubKey('Implemented Categories\{62C8FE65-4EBB-45E7-B440-6E39B2CDBF29}') | Out-Null
    $s = $k.CreateSubKey('InprocServer32')
    $s.SetValue('', 'mscoree.dll')
    $s.SetValue('ThreadingModel', 'Both')
    $s.SetValue('Class', $ClassName)
    $s.SetValue('Assembly', $asm)
    $s.SetValue('RuntimeVersion', 'v4.0.30319')
    $s.SetValue('CodeBase', $codeBase)
    $hkcu.CreateSubKey("Software\Classes\$ProgId\CLSID").SetValue('', $Clsid)
    foreach ($app in $Apps) {
      $a = $hkcu.CreateSubKey("Software\Microsoft\Office\$app\Addins\$ProgId")
      $a.SetValue('FriendlyName', "$brand $FriendlySuffix")
      $a.SetValue('Description', $Description)
      $a.SetValue('LoadBehavior', 3, 'DWord')
    }
  } finally { $hkcu.Dispose() }
}
"registered $ProgId $Clsid for $($Apps -join ', ') from $dll"
