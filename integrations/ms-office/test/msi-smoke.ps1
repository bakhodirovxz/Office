<#
.SYNOPSIS
  End-to-end check of the MSI: builds it (build-msi.ps1), installs it per-user, runs the Excel smoke test against the
  INSTALLED add-in (no register.ps1: the registration is the MSI's), then uninstalls and verifies that no file and no
  key is left - also when the test fails. While installed, the add-in would load into any Office app started by the
  user, so the window is kept to the length of one Excel test (about 30 s).
.PARAMETER Out   build.sh output root ($OFFICE_TARGET).
#>
param([Parameter(Mandatory = $true)][string]$Out, [string]$Wix)
$ErrorActionPreference = 'Stop'
$here = $PSScriptRoot
. (Join-Path $here '..\addin-ids.ps1')
$msiDir = Join-Path $Out 'ms-office\msi-test'
$buildArgs = @{ AddinDir = (Join-Path $Out 'ms-office\addin'); Out = $msiDir }
if ($Wix) { $buildArgs.Wix = $Wix }
& (Join-Path $here '..\build-msi.ps1') @buildArgs
if ($LASTEXITCODE) { throw 'build-msi failed' }
$msi = Get-ChildItem $msiDir -Filter *.msi | Sort-Object LastWriteTime | Select-Object -Last 1
. (Join-Path $here '..\tools.ps1')
$csc = Find-Csc
$office = Join-Path $Out 'ms-office\smoke-office.exe'
& $csc -nologo -out:$office -r:Microsoft.CSharp.dll -r:System.Core.dll (Join-Path $here 'SmokeOffice.cs')

function Keys {
  $keys = @("Software\Classes\CLSID\$Clsid", "Software\Classes\$ProgId") + ($AllApps | ForEach-Object { "Software\Microsoft\Office\$_\Addins\$ProgId" })
  $found = @()
  foreach ($view in 'Registry64', 'Registry32') {
    $h = [Microsoft.Win32.RegistryKey]::OpenBaseKey('CurrentUser', $view)
    try { foreach ($k in $keys) { if ($null -ne $h.OpenSubKey($k)) { $found += "$view $k" } } } finally { $h.Dispose() }
  }
  $found
}

$rc = 0
$log = Join-Path $msiDir ('install-' + (Get-Date -Format 'HHmmss') + '.log')
try {
  $t = [Diagnostics.Stopwatch]::StartNew()
  $p = Start-Process msiexec -ArgumentList @('/i', "`"$($msi.FullName)`"", '/qn', '/l*v', "`"$log`"") -PassThru; $p.WaitForExit()   # not -Wait: it also waits for child processes
  Write-Output "install: exit $($p.ExitCode), $($t.ElapsedMilliseconds) ms; keys: $((Keys).Count)"
  if ($p.ExitCode) { throw "msiexec /i failed ($($p.ExitCode))" }
  $cb = [Microsoft.Win32.RegistryKey]::OpenBaseKey('CurrentUser', 'Registry32').OpenSubKey("Software\Classes\CLSID\$Clsid\InprocServer32").GetValue('CodeBase')
  Write-Output "installed CodeBase: $cb"
  & $office excel (Join-Path $Out 'test-docs')
  if ($LASTEXITCODE) { $rc = 1 }
  # The test's Excel holds the installed DLL until it has exited: uninstall only after that.
  $pidFile = Join-Path (Join-Path $Out 'test-docs') 'pids-excel.txt'
  if (Test-Path $pidFile) {
    foreach ($id in (Get-Content $pidFile).Split(',') | Where-Object { $_ }) {
      $n = 0; while ((Get-Process -Id $id -ErrorAction SilentlyContinue) -and $n -lt 120) { Start-Sleep -Milliseconds 250; $n++ }
      Write-Output "test Excel $id exited: $(-not (Get-Process -Id $id -ErrorAction SilentlyContinue)) ($($n * 250) ms)"
    }
    Remove-Item $pidFile
  }
} finally {
  $t = [Diagnostics.Stopwatch]::StartNew()
  $p = Start-Process msiexec -ArgumentList @('/x', "`"$($msi.FullName)`"", '/qn', '/l*v', "`"$log.uninstall`"") -PassThru; $p.WaitForExit()
  $left = Keys
  $dir = Join-Path $env:LOCALAPPDATA 'Programs'
  $files = Get-ChildItem $dir -Recurse -Filter 'UzTranslit.OfficeAddin.dll' -ErrorAction SilentlyContinue
  Write-Output "uninstall: exit $($p.ExitCode), $($t.ElapsedMilliseconds) ms; keys left: $($left.Count); add-in DLLs left under $dir : $(@($files).Count)"
  if ($p.ExitCode -or $left.Count -or @($files).Count) { $rc = 2 }
  # MSIRESTARTMANAGERCONTROL=Disable: the uninstall must never shut down (and relaunch) an application.
  $rm = Get-Content "$log.uninstall" -Encoding Unicode -ErrorAction SilentlyContinue | Select-String 'RESTART MANAGER: (Successfully shut down|Previously shut down applications have been restarted)'
  Write-Output "Restart Manager shut down / relaunched an application during uninstall: $(@($rm).Count -gt 0)"
  if (@($rm).Count) { $rc = 2 }
}
exit $rc
