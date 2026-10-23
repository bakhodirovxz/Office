# Fetch WiX 5.0.2 (pinned in scripts/tools.lock) into E:\office-tools\wix-5.0.2 without installing it: download the official CLI msi, check
# the SHA-256 from tools.lock, unpack it with an administrative image (msiexec /a: no admin, nothing registered). v6+ is not used (PLAN F12).
param([string]$Dest = 'E:\office-tools\wix-5.0.2', [string]$Cache = 'E:\office-tools\dl-wix')
$ErrorActionPreference = 'Stop'
$line = Get-Content "$PSScriptRoot\tools.lock" | Where-Object { $_ -match '^wix 5\.0\.2 ' } | Select-Object -First 1
if (-not $line) { throw 'no wix 5.0.2 line in tools.lock' }
$f = $line -split '\s+'; $url = $f[2]; $sha = $f[3]
New-Item -ItemType Directory -Force $Cache | Out-Null
$msi = Join-Path $Cache 'wix-cli-x64.msi'
if (-not (Test-Path $msi)) { curl.exe -sL --ssl-no-revoke -o $msi $url }
$got = (Get-FileHash $msi -Algorithm SHA256).Hash
if ($got -ne $sha.ToUpper()) { Remove-Item $msi; throw "WiX msi hash mismatch: $got" }
if (-not (Test-Path "$Dest\PFiles64\WiX Toolset v5.0\bin\wix.exe")) { Start-Process msiexec -ArgumentList "/a `"$msi`" /qn TARGETDIR=`"$Dest`"" -Wait }
& "$Dest\PFiles64\WiX Toolset v5.0\bin\wix.exe" --version
