<#
.SYNOPSIS
  Builds the Office add-in MSI: stages the build output of build.sh, optionally signs the
  staged DLLs, runs WiX 5.0.2 on installer/addin.wxs, optionally signs the MSI. Installs nothing. docs/ms-addin/install.md.
.DESCRIPTION
  Product name, vendor and version come from brand/brand.toml (never hard-coded); identity and UpgradeCode from addin-ids.ps1;
  the Assembly full name is read from the built DLL. Package is x64 (both registry views are written by the 64-bit package),
  so 32-bit Windows is not supported.
  Default: per-user package (also installs per machine with `msiexec /i x.msi ALLUSERS=1 MSIINSTALLPERUSER=""`).
  -PerMachine: per-machine-default package for Group Policy Software Installation (GPO cannot pass properties).
  Signing: -Pfx (+ -PfxPassword | -PfxPasswordFile) or -Thumbprint (+ -Store) signs a STAGED COPY of the 3 DLLs before
  packaging and the MSI afterwards, through integrations/ms-office/sign.ps1.
.EXAMPLE
  powershell -File integrations/ms-office/build-msi.ps1
  powershell -File integrations/ms-office/build-msi.ps1 -PerMachine -Pfx dev.pfx -PfxPasswordFile dev.pfx.password.txt
#>
param(
  [string]$AddinDir,   # build.sh output; default $env:OFFICE_TARGET\ms-office\addin
  [string]$Out,        # stage, wix scratch and the .msi; default $env:OFFICE_TARGET\ms-office\msi
  [string]$Wix,        # WiX 5.0.2 wix.exe; default $env:WIX, PATH, or the dotnet global tool
  [switch]$PerMachine,
  [string]$Version,   # override brand.toml's version (upgrade tests)
  [string]$Pfx, [securestring]$PfxPassword, [string]$PfxPasswordFile,
  [string]$Thumbprint, [ValidateSet('CurrentUser', 'LocalMachine')][string]$Store = 'CurrentUser',
  [string]$TimestampUrl = 'http://timestamp.sectigo.com'
)
$ErrorActionPreference = 'Stop'
$here = $PSScriptRoot
. (Join-Path $here 'tools.ps1')
if (-not $AddinDir) { $AddinDir = Join-Path (Get-OfficeTarget) 'ms-office\addin' }
if (-not $Out) { $Out = Join-Path (Get-OfficeTarget) 'ms-office\msi' }
$Wix = Find-Wix $Wix
$repo = (Resolve-Path "$here\..\..").Path
. (Join-Path $here 'addin-ids.ps1')

# brand.toml [product]: a flat section of key = "value" lines
$toml = Get-Content (Join-Path $repo 'brand\brand.toml') -Raw -Encoding UTF8
$sec = [regex]::Match($toml, '(?ms)^\[product\]\s*\r?\n(.*?)(?=^\[|\z)').Groups[1].Value
function Key($k) { $m = [regex]::Match($sec, "(?m)^$k\s*=\s*`"([^`"]*)`""); if (-not $m.Success) { throw "brand.toml: [product] $k missing" }; $m.Groups[1].Value }
$brand = Key 'name'; $vendor = Key 'vendor'; $id = Key 'id'
if (-not $Version) { $Version = Key 'version' }

$dll = Join-Path $AddinDir 'UzTranslit.OfficeAddin.dll'
foreach ($p in $dll, "$AddinDir\brand-name.txt", "$AddinDir\x86\uz_translit_ffi.dll", "$AddinDir\x64\uz_translit_ffi.dll") { if (-not (Test-Path $p)) { throw "missing build output: $p (integrations/ms-office/build.sh)" } }
if ((Get-Content "$AddinDir\brand-name.txt" -Raw -Encoding UTF8).Trim() -ne $brand) { throw 'brand-name.txt differs from brand.toml: rebuild the add-in (build.sh)' }
if (-not (Test-Path $Wix)) { throw "wix.exe not found: $Wix" }
$asm = [Reflection.AssemblyName]::GetAssemblyName((Resolve-Path $dll).Path).FullName

$stage = Join-Path $Out 'stage'
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force "$stage\x86", "$stage\x64" | Out-Null
Copy-Item $dll, "$AddinDir\brand-name.txt" $stage
foreach ($a in 'x86', 'x64') { Copy-Item "$AddinDir\$a\uz_translit_ffi.dll" "$stage\$a" }
foreach ($f in 'NOTICE', 'LICENSE-MIT', 'LICENSE-APACHE') { Copy-Item (Join-Path $repo $f) $stage }

$signArgs = @{}
if ($Pfx) { $signArgs.Pfx = $Pfx; if ($PfxPassword) { $signArgs.PfxPassword = $PfxPassword }; if ($PfxPasswordFile) { $signArgs.PfxPasswordFile = $PfxPasswordFile } }
elseif ($Thumbprint) { $signArgs.Thumbprint = $Thumbprint; $signArgs.Store = $Store }
$sign = $signArgs.Count -gt 0
if ($sign) {
  $signArgs.TimestampUrl = $TimestampUrl
  & "$here\sign.ps1" @signArgs -Path "$stage\UzTranslit.OfficeAddin.dll", "$stage\x86\uz_translit_ffi.dll", "$stage\x64\uz_translit_ffi.dll"
  if ($LASTEXITCODE) { throw 'signing the DLLs failed' }
}

$scope = if ($PerMachine) { 'perMachine' } else { 'perUserOrMachine' }
$msi = Join-Path $Out ("$id-office-addin-$Version-x64" + $(if ($PerMachine) { '-machine' }) + '.msi')
$env:TEMP = $Out; $env:TMP = $Out   # keep wix scratch off C:
& $Wix build -arch x64 -o $msi "$here\installer\addin.wxs" `
  -d "Stage=$stage" -d "ProductName=$brand Office add-in" -d "Brand=$brand" -d "Vendor=$vendor" -d "Version=$Version" `
  -d "UpgradeCode=$UpgradeCode" -d "Scope=$scope" -d "Clsid=$Clsid" -d "ProgId=$ProgId" -d "ClassName=$ClassName" `
  -d "Assembly=$asm" -d "FriendlySuffix=$FriendlySuffix" -d "AddinDescription=$Description"
if ($LASTEXITCODE) { throw "wix build failed ($LASTEXITCODE)" }
if ($sign) { & "$here\sign.ps1" @signArgs -Path $msi; if ($LASTEXITCODE) { throw 'signing the MSI failed' } }
"built $msi ($([math]::Round((Get-Item $msi).Length / 1KB)) KB, $scope, version $Version)"
