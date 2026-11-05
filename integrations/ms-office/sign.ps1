<#
.SYNOPSIS
  Authenticode-signs files for the Office add-in (SHA-256 + RFC 3161 timestamp), then reports the signature and runs
  `signtool verify /pa`. It also takes a .pfx file.
.DESCRIPTION
  Certificate: -Pfx <file> (+ -PfxPassword SecureString, or -PfxPasswordFile, or a prompt) or -Thumbprint <sha1>
  (+ -Store CurrentUser|LocalMachine).
  A self-signed certificate (new-dev-cert.ps1) signs fine but `verify /pa` fails with an untrusted-root error on a PC that
  does not trust it; that case prints "signed, chain not trusted on this PC" and is NOT a failure. Anything else is.
  Note: signtool takes a pfx password on its command line, so it is visible to local processes while signtool runs.
  Exit code: 0 all signed (verified or chain-not-trusted), 1 sign or verify failed, 2 bad usage or signtool missing.
.EXAMPLE
  integrations/ms-office/sign.ps1 -Pfx dev-codesign.pfx -PfxPasswordFile dev-codesign.pfx.password.txt a.dll b.msi
#>
[CmdletBinding(DefaultParameterSetName = 'Pfx')]
param(
  [Parameter(Mandatory, ParameterSetName = 'Pfx')][string]$Pfx,
  [Parameter(ParameterSetName = 'Pfx')][securestring]$PfxPassword,
  [Parameter(ParameterSetName = 'Pfx')][string]$PfxPasswordFile,
  [Parameter(Mandatory, ParameterSetName = 'Thumbprint')][string]$Thumbprint,
  [Parameter(ParameterSetName = 'Thumbprint')][ValidateSet('CurrentUser', 'LocalMachine')][string]$Store = 'CurrentUser',
  [Parameter(Mandatory, Position = 0, ValueFromRemainingArguments)][string[]]$Path,
  [string]$TimestampUrl = 'http://timestamp.sectigo.com',
  [string]$SignTool
)
$ErrorActionPreference = 'Stop'

if (-not $SignTool) {
  $SignTool = (Get-Command signtool.exe -ErrorAction SilentlyContinue).Source
  if (-not $SignTool) {
    $SignTool = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\signtool.exe" -ErrorAction SilentlyContinue |
      Sort-Object { [version]$_.Directory.Parent.Name } | Select-Object -Last 1 -ExpandProperty FullName
  }
}
if (-not $SignTool -or -not (Test-Path $SignTool)) { Write-Error 'signtool.exe not found (install the Windows SDK)'; exit 2 }

if ($PSCmdlet.ParameterSetName -eq 'Pfx') {
  if (-not (Test-Path -LiteralPath $Pfx)) { Write-Error "pfx not found: $Pfx"; exit 2 }
  if ($PfxPasswordFile) { $plain = (Get-Content -LiteralPath $PfxPasswordFile -Raw).Trim() }
  else {
    if (-not $PfxPassword) { $PfxPassword = Read-Host "Password for $Pfx" -AsSecureString }
    $plain = [Net.NetworkCredential]::new('', $PfxPassword).Password
  }
  $certArgs = @('/f', (Resolve-Path -LiteralPath $Pfx).Path, '/p', $plain)
} else {
  $Thumbprint = $Thumbprint -replace '\s', ''
  if (-not (Test-Path "Cert:\$Store\My\$Thumbprint")) { Write-Error "No certificate $Thumbprint in $Store\My"; exit 2 }
  $certArgs = @('/sha1', $Thumbprint, '/s', 'My'); if ($Store -eq 'LocalMachine') { $certArgs += '/sm' }
}

$rc = 0
foreach ($f in ($Path | ForEach-Object { Get-Item -LiteralPath $_ } | Select-Object -ExpandProperty FullName)) {
  # the public timestamp servers fail now and then (seen on this PC): retry a few times
  foreach ($try in 1..3) { & $SignTool sign /fd sha256 /tr $TimestampUrl /td sha256 @certArgs $f; if ($LASTEXITCODE -eq 0) { break }; Start-Sleep 2 }
  if ($LASTEXITCODE -ne 0) { Write-Host "SIGN FAILED: $f"; $rc = 1; continue }
  $sig = Get-AuthenticodeSignature -LiteralPath $f
  $sub = if ($sig.SignerCertificate) { $sig.SignerCertificate.Subject } else { '(none)' }
  $ts = if ($sig.TimeStamperCertificate) { $sig.TimeStamperCertificate.Subject } else { '(none)' }
  Write-Host "  signer: $sub`n  timestamp: $ts`n  status: $($sig.Status)"
  $ErrorActionPreference = 'Continue'   # signtool writes its verdict on stderr; do not let 2>&1 turn it into a terminating error
  $out = & $SignTool verify /pa /v $f 2>&1 | Out-String; $vrc = $LASTEXITCODE
  $ErrorActionPreference = 'Stop'
  if ($vrc -eq 0) { Write-Host "OK (verified): $f"; continue }
  # Chain built but the root is not trusted here: accepted only for a self-signed signer with a timestamp.
  $selfSigned = $sig.SignerCertificate -and ($sig.SignerCertificate.Subject -eq $sig.SignerCertificate.Issuer)
  if ($selfSigned -and $sig.TimeStamperCertificate -and $out -match 'root certificate which is not trusted|not trusted by the trust provider') {
    Write-Host "OK (signed, chain not trusted on this PC; self-signed): $f"
  } else { Write-Host $out; Write-Host "VERIFY FAILED: $f"; $rc = 1 }
}
exit $rc
