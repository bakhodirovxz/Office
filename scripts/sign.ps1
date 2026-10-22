<#
.SYNOPSIS
  Authenticode-sign files with signtool (SHA-256 + RFC 3161 timestamp), then verify (PLAN WP13, docs/phase4/signing.md).
.DESCRIPTION
  The certificate is chosen by thumbprint from a certificate store. A hardware token or cloud HSM that exposes its
  certificate through a CSP/KSP/minidriver shows up in the store too, so the same call works for it (signtool asks for
  the PIN). The private key never leaves the store, and nothing here handles key material.
  Exit code: 0 all signed and verified, 1 sign or verify failed, 2 bad usage or signtool missing.
.EXAMPLE
  scripts/sign.ps1 -Thumbprint 0123...ABCD E:\office-target\p1\msvc\docs\x86_64-pc-windows-msvc\release\docs-desktop.exe
.EXAMPLE
  scripts/sign.ps1 -Thumbprint $tp -Store LocalMachine -TimestampUrl http://timestamp.example.org -Path a.exe,b.msi
#>
param(
  [Parameter(Mandatory)][string]$Thumbprint,
  [Parameter(Mandatory, Position = 0, ValueFromRemainingArguments)][string[]]$Path,
  [string]$TimestampUrl = 'http://timestamp.sectigo.com',
  [ValidateSet('CurrentUser', 'LocalMachine')][string]$Store = 'CurrentUser',
  [string]$SignTool
)
$ErrorActionPreference = 'Stop'

if (-not $SignTool) {
  $SignTool = (Get-Command signtool.exe -ErrorAction SilentlyContinue).Source
  if (-not $SignTool) {   # newest SDK x64 signtool
    $SignTool = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\signtool.exe" -ErrorAction SilentlyContinue |
      Sort-Object { [version]$_.Directory.Parent.Name } | Select-Object -Last 1 -ExpandProperty FullName
  }
}
if (-not $SignTool -or -not (Test-Path $SignTool)) { Write-Error 'signtool.exe not found (install the Windows SDK, docs/phase4/msvc.md)'; exit 2 }

$Thumbprint = $Thumbprint -replace '\s', ''
if (-not (Test-Path "Cert:\$Store\My\$Thumbprint")) { Write-Error "No certificate $Thumbprint in $Store\My (token plugged in and middleware installed?)"; exit 2 }
$storeArgs = @('/sha1', $Thumbprint, '/s', 'My'); if ($Store -eq 'LocalMachine') { $storeArgs += '/sm' }

$files = $Path | ForEach-Object { Get-Item -LiteralPath $_ } | Select-Object -ExpandProperty FullName
$rc = 0
foreach ($f in $files) {
  & $SignTool sign /fd sha256 /tr $TimestampUrl /td sha256 @storeArgs $f
  if ($LASTEXITCODE -ne 0) { Write-Host "SIGN FAILED: $f"; $rc = 1; continue }
  & $SignTool verify /pa /v $f
  if ($LASTEXITCODE -ne 0) { Write-Host "VERIFY FAILED: $f"; $rc = 1; continue }
  Write-Host "OK: $f"
}
exit $rc

