<#
.SYNOPSIS
  Removes everything register.ps1 wrote (both registry views, every Office app it knows), and checks nothing is left.
  Exit code 1 if a key survived.
#>
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'addin-ids.ps1')

$keys = @("Software\Classes\CLSID\$Clsid", "Software\Classes\$ProgId") + ($AllApps | ForEach-Object { "Software\Microsoft\Office\$_\Addins\$ProgId" })
$left = @()
foreach ($view in 'Registry64', 'Registry32') {
  $hkcu = [Microsoft.Win32.RegistryKey]::OpenBaseKey('CurrentUser', $view)
  try {
    foreach ($k in $keys) { $hkcu.DeleteSubKeyTree($k, $false) }
    foreach ($k in $keys) { if ($null -ne $hkcu.OpenSubKey($k)) { $left += "$view $k" } }
  } finally { $hkcu.Dispose() }
}
if ($left.Count) { "still registered: $($left -join '; ')"; exit 1 }
"unregistered $ProgId; no keys left (checked $($keys.Count) keys in both views)"
exit 0
