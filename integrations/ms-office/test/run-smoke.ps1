<#
.SYNOPSIS
  Registers the add-in (HKCU, Word + Excel + PowerPoint) only for the length of the smoke run, runs the smoke tests
  against Office instances of their own, then unregisters and verifies nothing is left - also when a test fails.
.PARAMETER Out      build.sh output root ($OFFICE_TARGET); the add-in is in <Out>\ms-office\addin, test files go to <Out>\test-docs.
.PARAMETER Apps     which apps to test (default Word, Excel, PowerPoint).
.PARAMETER Big      Word: paragraphs of the large benchmark document; Excel: rows of the large sheet (0 = skip).
.PARAMETER FastOnly Word benchmark: only the fast mode.
#>
param([Parameter(Mandatory = $true)][string]$Out, [string[]]$Apps = @('Word', 'Excel', 'PowerPoint'), [int]$Big = 0, [switch]$FastOnly)
$ErrorActionPreference = 'Stop'
$here = $PSScriptRoot
$root = (Resolve-Path (Join-Path $here '..\..\..')).Path
$addin = Join-Path $Out 'ms-office\addin'
$docs = Join-Path $Out 'test-docs'
. (Join-Path $here '..\tools.ps1')
$csc = Find-Csc
$word = Join-Path $Out 'ms-office\smoke.exe'
$office = Join-Path $Out 'ms-office\smoke-office.exe'
& $csc -nologo -out:$word -r:Microsoft.CSharp.dll -r:System.Core.dll (Join-Path $here 'Smoke.cs')
if ($LASTEXITCODE) { throw 'smoke.exe build failed' }
& $csc -nologo -out:$office -r:Microsoft.CSharp.dll -r:System.Core.dll (Join-Path $here 'SmokeOffice.cs')
if ($LASTEXITCODE) { throw 'smoke-office.exe build failed' }

$rc = 0
try {
  & (Join-Path $here '..\register.ps1') -Dir $addin -Apps Word, Excel, PowerPoint
  foreach ($app in $Apps) {
    Write-Output "=== $app"
    if ($app -eq 'Word') {
      $smokeArgs = @($docs, (Join-Path $root 'crates\uz-translit\data\golden.tsv'))
      if ($Big -gt 0) { $smokeArgs += @('--big', $Big) }
      if ($FastOnly) { $smokeArgs += '--fast-only' }
      & $word @smokeArgs
    } else {
      $smokeArgs = @($app.ToLower(), $docs)
      if ($Big -gt 0) { $smokeArgs += @('--big', $Big) }
      & $office @smokeArgs
      $code = $LASTEXITCODE
      # The test's own Office process must exit once the test is gone (the add-in must not keep it alive).
      $pidFile = Join-Path $docs "pids-$($app.ToLower()).txt"
      if (Test-Path $pidFile) {
        foreach ($id in (Get-Content $pidFile).Split(',') | Where-Object { $_ }) {
          $t = 0; while ((Get-Process -Id $id -ErrorAction SilentlyContinue) -and $t -lt 80) { Start-Sleep -Milliseconds 250; $t++ }
          if (Get-Process -Id $id -ErrorAction SilentlyContinue) { Write-Output "FAIL $app process $id still running 20 s after the test"; $code = 1 }
          else { Write-Output "PASS $app process $id exited ($($t * 250) ms after the test)" }
        }
        Remove-Item $pidFile
      }
      $global:LASTEXITCODE = $code
    }
    if ($LASTEXITCODE -eq 3) { Write-Output "$app skipped" }
    elseif ($LASTEXITCODE) { $rc = 1 }
  }
} catch { Write-Output "ERROR $_"; $rc = 1 }
finally {
  & (Join-Path $here '..\unregister.ps1')
  if ($LASTEXITCODE) { $rc = 2 }
}
exit $rc
