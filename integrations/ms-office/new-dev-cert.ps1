<#
.SYNOPSIS
  Creates a self-signed code-signing certificate for DEVELOPMENT signing tests and leaves it only as files:
  dev-codesign.pfx, dev-codesign.cer (public part) and dev-codesign.pfx.password.txt (random password, dev only).
.DESCRIPTION
  New-SelfSignedCertificate needs a store, so the cert is created in Cert:\CurrentUser\My, exported, then removed
  together with its private key, and the script verifies it is gone. It never touches Root, TrustedPublisher or any
  LocalMachine store. Do not use this certificate for anything real (docs/ms-addin/install.md has the real options).
#>
param([string]$OutDir = (Join-Path $(if ($env:OFFICE_TARGET) { $env:OFFICE_TARGET } else { $env:USERPROFILE }) 'certs'))
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force $OutDir | Out-Null
$pfx = Join-Path $OutDir 'dev-codesign.pfx'; $cer = Join-Path $OutDir 'dev-codesign.cer'; $pwf = "$pfx.password.txt"

$pw = -join ((48..57) + (65..90) + (97..122) | Get-Random -Count 24 | ForEach-Object { [char]$_ })
$cert = New-SelfSignedCertificate -Type CodeSigningCert -Subject 'CN=Office add-in DEV code signing (not for production)' `
  -KeyAlgorithm RSA -KeyLength 3072 -HashAlgorithm SHA256 -KeyExportPolicy Exportable -NotAfter (Get-Date).AddYears(2) `
  -CertStoreLocation Cert:\CurrentUser\My
try {
  Export-PfxCertificate -Cert $cert -FilePath $pfx -Password (ConvertTo-SecureString $pw -AsPlainText -Force) | Out-Null
  Export-Certificate -Cert $cert -FilePath $cer | Out-Null
  Set-Content -LiteralPath $pwf -Value $pw -Encoding ascii -NoNewline
} finally {
  # the API also leaves a copy in CurrentUser\CA (intermediate store) on this PC; remove from both, key with the My copy
  Remove-Item "Cert:\CurrentUser\My\$($cert.Thumbprint)" -DeleteKey -Force
  Remove-Item "Cert:\CurrentUser\CA\$($cert.Thumbprint)" -Force -ErrorAction SilentlyContinue
}
if (Get-ChildItem Cert:\CurrentUser, Cert:\LocalMachine -Recurse -ErrorAction SilentlyContinue | Where-Object Thumbprint -eq $cert.Thumbprint) { throw "certificate $($cert.Thumbprint) is still in a store" }
"created $pfx, $cer, $pwf ; thumbprint $($cert.Thumbprint) ; not in any store"
