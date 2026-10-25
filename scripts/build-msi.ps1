# Build the suite MSI (PLAN phase 4, WP14, 7.1-7.2): stage the MSVC release builds under the brand's exe names, generate
# packaging/windows/suite.wxs and default-associations.xml from brand/brand.toml, run WiX 5.0.2 (pinned, scripts/tools.lock).
# Nothing here installs anything. The product name, exe names, AUMIDs and the codes all come from brand.toml.
#   powershell -File scripts/build-msi.ps1 [-Target E:\office-target\p1] [-Wix <wix.exe>] [-SignThumbprint <sha1>]
# Inputs: `scripts/build.sh all all --release --msvc` ($Target\msvc), the web dists + `scripts/shell-dist.sh` ($Dist), `cargo xt overlay all` ($Target\overlay).
param(
  [string]$Target = 'E:\office-target\p1',
  [string]$Dist = (Join-Path $Target 'dist'),   # web dists + shell (scripts/shell-dist.sh)
  [string]$Wix = 'E:\office-tools\wix-5.0.2\PFiles64\WiX Toolset v5.0\bin\wix.exe',
  [string]$VcRedist = ((Get-Item 'E:\VS\BuildTools\VC\Redist\MSVC\14.*\x64\Microsoft.VC143.CRT' | Sort-Object { $_.Parent.Parent.Name } | Select-Object -Last 1).FullName),
  [string]$SignThumbprint
)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path "$PSScriptRoot\..").Path
$out = Join-Path $Target 'msi'; $stage = Join-Path $out 'stage'
$pk = Join-Path $repo 'packaging\windows'; New-Item -ItemType Directory -Force $pk | Out-Null

# ---- brand.toml (the few keys we need; the file is flat enough for a section regex)
$toml = Get-Content (Join-Path $repo 'brand\brand.toml') -Raw -Encoding UTF8
function Sec($name) { $m = [regex]::Match($toml, "(?ms)^\[$([regex]::Escape($name))\]\s*\r?\n(.*?)(?=^\[|\z)"); if (-not $m.Success) { throw "brand.toml: [$name] missing" }; $m.Groups[1].Value }
function Key($sec, $k) { $m = [regex]::Match($sec, "(?m)^$k\s*=\s*`"([^`"]*)`""); if (-not $m.Success) { throw "brand.toml: $k missing" }; $m.Groups[1].Value }
$p = Sec 'product'; $id = Key $p 'id'; $name = Key $p 'name'; $version = Key $p 'version'; $vendor = Key $p 'vendor'
function Uuid5($ns, $text) {   # RFC 4122 name-based (SHA-1); the namespace is fixed here so a code never changes unless id/version do
  $n = [guid]$ns; $nb = $n.ToByteArray(); [Array]::Reverse($nb, 0, 4); [Array]::Reverse($nb, 4, 2); [Array]::Reverse($nb, 6, 2)
  $h = [Security.Cryptography.SHA1]::Create().ComputeHash($nb + [Text.Encoding]::UTF8.GetBytes($text))[0..15]
  $h[6] = ($h[6] -band 0x0F) -bor 0x50; $h[8] = ($h[8] -band 0x3F) -bor 0x80
  [Array]::Reverse($h, 0, 4); [Array]::Reverse($h, 4, 2); [Array]::Reverse($h, 6, 2); ([guid][byte[]]$h).ToString('B').ToUpper()
}
$NS = '6ba7b810-9dad-11d1-80b4-00c04fd430c8'   # RFC 4122 DNS namespace
$upgradeCode = Uuid5 $NS "upgrade.$id"; $productCode = Uuid5 $NS "product.$id.$version"

# (brand key, upstream build dir, ProgID description, extensions). The lists are PLAN 7.2 (also services/suite-desktop APPS). No odp, no ppt.
$apps = @(
  @{ k = 'docs';   desc = 'Documents';        ext = 'docx docm dotx dotm doc dot odt rtf txt md markdown html htm' },
  @{ k = 'sheets'; desc = 'Spreadsheets';     ext = 'xlsx xlsm xltx xltm xls xlt xlsb ods csv tsv' },
  @{ k = 'slides'; desc = 'Presentations';    ext = 'pptx potx ppsx' },
  @{ k = 'pdf';    desc = 'PDF viewer';       ext = 'pdf' }
)
foreach ($a in $apps) {
  $s = Sec "apps.$($a.k)"; $a.bin = Key $s 'bin'; $a.aumid = Key $s 'aumid'; $a.data = (Key $s 'data_dir') -replace '/', '\'
  $a.name = [regex]::Match($s, 'names\s*=\s*\{[^}]*\ben\s*=\s*"([^"]*)"').Groups[1].Value
  if (-not $a.name) { throw "brand.toml: apps.$($a.k).names.en missing" }
  $a.progid = "$($a.aumid).Document"; $a.ext = $a.ext -split ' '
}
$cs = Sec 'apps.convert'; $convertBin = Key $cs 'bin'

# ---- stage: final layout and final exe names
$msvc = "x86_64-pc-windows-msvc\release"
function Src($path) { if (-not (Test-Path $path)) { throw "missing build output: $path (scripts/build.sh all all --release --msvc)" }; $path }
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force $stage, "$stage\convert", "$stage\icons", "$stage\fonts", "$stage\licenses\upstream" | Out-Null
$svc = Join-Path $Target "msvc\services\$msvc"
Copy-Item (Src "$svc\hoshiya.exe") "$stage\$id.exe"; Copy-Item (Src "$svc\hoshiya-serve.exe") "$stage\$id-serve.exe"
Copy-Item (Src "$svc\convert.exe") "$stage\convert\$convertBin.exe"
foreach ($a in $apps) {
  Copy-Item (Src "$Target\msvc\$($a.k)\$msvc\$($a.k).exe") "$stage\$($a.bin).exe"
  Copy-Item (Src "$Target\msvc\$($a.k)\$msvc\$($a.k)-cli.exe") "$stage\convert\$($a.k)-cli.exe"   # services/convert looks for <domain>-cli.exe next to itself
  if ($a.k -in 'docs', 'sheets') { Copy-Item (Src "$Target\msvc\$($a.k)\$msvc\$($a.k)-diff.exe") "$stage\$($a.k)-diff.exe" }   # history_compare runs <bin_dir>\<app>-diff.exe (WP12)
  Copy-Item (Src "$Target\overlay\$($a.k)\icon-document.ico") "$stage\icons\$($a.bin)-document.ico"
}
# The MSVC exes import VCRUNTIME140*.dll (dumpbin /dependents); a clean Windows 10 has no VC++ redistributable, so ship the two DLLs app-local
# next to every exe that needs them (the app folder is searched first: /DEPENDENTLOADFLAG:0xA00). Microsoft-signed, checked here.
foreach ($dll in 'vcruntime140.dll', 'vcruntime140_1.dll') {
  $src = Src (Join-Path $VcRedist $dll); $sig = Get-AuthenticodeSignature $src
  if ($sig.Status -ne 'Valid' -or $sig.SignerCertificate.Subject -notmatch 'Microsoft') { throw "$src : not a valid Microsoft signature ($($sig.Status))" }
  Copy-Item $src $stage; Copy-Item $src "$stage\convert"
}
Set-Content "$stage\licenses\vc-redist.txt" "vcruntime140.dll and vcruntime140_1.dll (Microsoft Visual C++ 2015-2022 runtime, $(Split-Path (Split-Path (Split-Path $VcRedist)) -Leaf)) are redistributed unmodified, app-local, as the Visual Studio distributable code (VC\Redist\MSVC). They are Authenticode-signed by Microsoft. Terms: https://learn.microsoft.com/en-us/visualstudio/releases/2022/redistribution (files under VC\Redist, unmodified, for licensed Visual Studio users); see also NOTICE." -Encoding UTF8
Copy-Item (Join-Path $repo 'brand\logo\windows\hoshiya.ico') "$stage\icons\$id.ico"
Copy-Item (Join-Path $repo 'brand\brand.toml') $stage
Copy-Item (Src "$Dist\shell") "$stage\dist\shell" -Recurse
foreach ($a in $apps) { Copy-Item (Src "$Dist\$($a.k)-web") "$stage\dist\$($a.k)-web" -Recurse }
Copy-Item (Join-Path $repo 'assets\dictionaries') "$stage\dictionaries" -Recurse
Copy-Item (Join-Path $repo 'assets\fonts\fallback\*') "$stage\fonts" -Recurse
foreach ($f in 'NOTICE', 'LICENSE-MIT', 'LICENSE-APACHE') { Copy-Item (Join-Path $repo $f) $stage }
Copy-Item (Join-Path $repo 'brand\logo\LICENSE.txt') "$stage\licenses\logo-LICENSE.txt"
# Third-party crate notices (gate F3): the committed file is the output of scripts/gen-notices.sh; ci.sh job_msi runs `gen-notices.sh --check` first, so it is current.
$tpn = Join-Path $repo 'packaging\windows\THIRD-PARTY-NOTICES.txt'; if (-not (Test-Path $tpn)) { throw "missing $tpn (scripts/gen-notices.sh)" }
Copy-Item $tpn "$stage\licenses\THIRD-PARTY-NOTICES.txt"; Copy-Item (Join-Path $repo 'packaging\windows\WebView2-LICENSE.txt') "$stage\licenses\WebView2-LICENSE.txt"
foreach ($u in 'wordcraft', 'gridcraft', 'deckcraft', 'pdfcraft', 'craft-libs', 'craft-fonts') {
  New-Item -ItemType Directory -Force "$stage\licenses\upstream\$u" | Out-Null
  Get-ChildItem "$repo\upstream\$u" -File | Where-Object { $_.Name -match '^(LICENSE|NOTICE|ATTRIBUTION)' } | Copy-Item -Destination "$stage\licenses\upstream\$u"
}
# the web dists must carry fresh .br files (scripts/shell-dist.sh --check is the gate); the host serves them
if ($SignThumbprint) { & "$PSScriptRoot\sign.ps1" -Thumbprint $SignThumbprint -Store LocalMachine (Get-ChildItem $stage -Recurse -Filter *.exe | ForEach-Object FullName); if ($LASTEXITCODE) { throw 'signing failed' } }

# ---- wxs
function X($s) { [Security.SecurityElement]::Escape($s) }
$sb = [Text.StringBuilder]::new()
function L($s) { [void]$sb.AppendLine($s) }
$exeCmd = { param($bin) "&quot;[INSTALLFOLDER]$bin.exe&quot; &quot;%1&quot;" }
L '<?xml version="1.0" encoding="utf-8"?>'
L "<!-- GENERATED by scripts/build-msi.ps1 from brand/brand.toml. Do not edit. WiX 5.0.2. -->"
L '<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs">'
L "  <Package Name=`"$(X $name)`" Manufacturer=`"$(X $vendor)`" Version=`"$version`" UpgradeCode=`"$upgradeCode`" ProductCode=`"$productCode`" Language=`"1033`" Scope=`"perMachine`" Compressed=`"yes`">"
L '    <MajorUpgrade DowngradeErrorMessage="A newer version is already installed." />'
L '    <MediaTemplate EmbedCab="yes" CompressionLevel="high" />'
L "    <Icon Id=`"AppIcon`" SourceFile=`"`$(var.Stage)\icons\$id.ico`" />"
L '    <Property Id="ARPPRODUCTICON" Value="AppIcon" />'
L '    <Launch Condition="Installed OR VersionNT64 &gt;= 603" Message="This product needs Windows 8.1 or later (64-bit)." />'
L "    <StandardDirectory Id=`"ProgramFiles64Folder`"><Directory Id=`"INSTALLFOLDER`" Name=`"$(X $name)`">"
foreach ($d in 'dist\shell', 'dist\docs-web', 'dist\sheets-web', 'dist\slides-web', 'dist\pdf-web', 'dictionaries', 'fonts', 'icons', 'licenses', 'convert') {
  # Files harvests the folder into a directory of the same name; one Directory per top-level folder, dist children nested
  if ($d -like 'dist\*') { continue }
  L "      <Directory Id=`"D_$($d -replace '\W','_')`" Name=`"$d`" />"
}
L '      <Directory Id="D_dist" Name="dist">'
foreach ($a in @('shell') + ($apps | ForEach-Object { "$($_.k)-web" })) { L "        <Directory Id=`"D_dist_$($a -replace '\W','_')`" Name=`"$a`" />" }
L '      </Directory>'
L '    </Directory></StandardDirectory>'
L "    <StandardDirectory Id=`"ProgramMenuFolder`"><Directory Id=`"AppMenuDir`" Name=`"$(X $name)`" /></StandardDirectory>"
L '    <Feature Id="Main" Title="Main" Level="1" AllowAbsent="no">'
L '      <ComponentGroupRef Id="Core" /><ComponentGroupRef Id="Payload" /><ComponentGroupRef Id="OpenWith" />'
L '    </Feature>'
L '  </Package>'

# Core: the host (with the Start menu entries) and the editors
L '  <Fragment><ComponentGroup Id="Core" Directory="INSTALLFOLDER">'
L "    <Component Id=`"C_host`"><File Id=`"F_host`" Source=`"`$(var.Stage)\$id.exe`" KeyPath=`"yes`">"
L "      <Shortcut Id=`"S_host`" Directory=`"AppMenuDir`" Name=`"$(X $name)`" WorkingDirectory=`"INSTALLFOLDER`" Advertise=`"no`"><ShortcutProperty Key=`"System.AppUserModel.ID`" Value=`"$(X $name)`" /></Shortcut>"
L '    </File><RemoveFolder Id="R_menu" Directory="AppMenuDir" On="uninstall" /></Component>'
L "    <Component Id=`"C_serve`"><File Id=`"F_serve`" Source=`"`$(var.Stage)\$id-serve.exe`" KeyPath=`"yes`" /></Component>"
foreach ($a in $apps) {
  L "    <Component Id=`"C_$($a.k)`"><File Id=`"F_$($a.k)`" Source=`"`$(var.Stage)\$($a.bin).exe`" KeyPath=`"yes`">"
  L "      <Shortcut Id=`"S_$($a.k)`" Directory=`"AppMenuDir`" Name=`"$(X $a.name)`" WorkingDirectory=`"INSTALLFOLDER`" Advertise=`"no`"><ShortcutProperty Key=`"System.AppUserModel.ID`" Value=`"$(X $a.aumid)`" /></Shortcut>"
  L '    </File></Component>'
}
foreach ($k in 'docs', 'sheets') { L "    <Component Id=`"C_$k`_diff`"><File Id=`"F_$k`_diff`" Source=`"`$(var.Stage)\$k-diff.exe`" KeyPath=`"yes`" /></Component>" }
L '    <Component Id="C_brand"><File Id="F_brand" Source="$(var.Stage)\brand.toml" KeyPath="yes" /></Component>'
foreach ($f in 'vcruntime140.dll', 'vcruntime140_1.dll') { L "    <Component Id=`"C_$($f -replace '\W','_')`"><File Id=`"F_$($f -replace '\W','_')`" Source=`"`$(var.Stage)\$f`" KeyPath=`"yes`" /></Component>" }
foreach ($f in 'NOTICE', 'LICENSE-MIT', 'LICENSE-APACHE') { L "    <Component Id=`"C_$($f -replace '\W','_')`"><File Id=`"F_$($f -replace '\W','_')`" Source=`"`$(var.Stage)\$f`" KeyPath=`"yes`" /></Component>" }
L '  </ComponentGroup></Fragment>'

# Payload: whole folders
L '  <Fragment><ComponentGroup Id="Payload">'
foreach ($pair in @(@('D_dist_shell', 'dist\shell'), @('D_dist_docs-web', 'dist\docs-web'), @('D_dist_sheets-web', 'dist\sheets-web'), @('D_dist_slides-web', 'dist\slides-web'), @('D_dist_pdf-web', 'dist\pdf-web'),
                    @('D_dictionaries', 'dictionaries'), @('D_fonts', 'fonts'), @('D_icons', 'icons'), @('D_licenses', 'licenses'), @('D_convert', 'convert'))) {
  L "    <Files Include=`"`$(var.Stage)\$($pair[1])\**`" Directory=`"$($pair[0] -replace '-', '_')`" />"
}
L '  </ComponentGroup></Fragment>'

# OpenWith (PLAN 7.2). Everything under HKLM; nothing under UserChoice; no default value of any .ext key.
$root = 'HKLM'
L '  <Fragment><ComponentGroup Id="OpenWith">'
foreach ($a in $apps) {
  $pid_ = $a.progid; $exe = "$($a.bin).exe"; $cap = "Software\$($a.data)\Capabilities"
  $icon = "[INSTALLFOLDER]icons\$($a.bin)-document.ico"; $exeIcon = "[INSTALLFOLDER]$exe,0"
  L "    <Component Id=`"OW_$($a.k)`" Directory=`"INSTALLFOLDER`" Guid=`"*`">"
  L "      <RegistryKey Root=`"$root`" Key=`"Software\Classes\$pid_`" ForceDeleteOnUninstall=`"yes`">"
  L "        <RegistryValue Type=`"string`" Value=`"$(X $a.name) $(($a.desc).ToLower())`" KeyPath=`"yes`" />"
  L "        <RegistryValue Type=`"string`" Name=`"AppUserModelID`" Value=`"$(X $a.aumid)`" />"
  L "        <RegistryValue Key=`"DefaultIcon`" Type=`"string`" Value=`"$icon`" />"
  L "        <RegistryValue Key=`"shell\open\command`" Type=`"string`" Value=`"$(& $exeCmd $a.bin)`" />"
  L '      </RegistryKey>'
  L "      <RegistryKey Root=`"$root`" Key=`"Software\Classes\Applications\$exe`" ForceDeleteOnUninstall=`"yes`">"
  L "        <RegistryValue Type=`"string`" Name=`"FriendlyAppName`" Value=`"$(X $a.name)`" />"
  L "        <RegistryValue Key=`"DefaultIcon`" Type=`"string`" Value=`"$exeIcon`" />"
  L "        <RegistryValue Key=`"shell\open\command`" Type=`"string`" Value=`"$(& $exeCmd $a.bin)`" />"
  foreach ($e in $a.ext) { L "        <RegistryValue Key=`"SupportedTypes`" Type=`"string`" Name=`".$e`" Value=`"`" />" }
  L '      </RegistryKey>'
  L "      <RegistryKey Root=`"$root`" Key=`"$cap`" ForceDeleteOnUninstall=`"yes`">"
  L "        <RegistryValue Type=`"string`" Name=`"ApplicationName`" Value=`"$(X $a.name)`" />"
  L "        <RegistryValue Type=`"string`" Name=`"ApplicationDescription`" Value=`"$(X $a.name) - $($a.desc.ToLower())`" />"
  L "        <RegistryValue Type=`"string`" Name=`"ApplicationIcon`" Value=`"$exeIcon`" />"
  foreach ($e in $a.ext) { L "        <RegistryValue Key=`"FileAssociations`" Type=`"string`" Name=`".$e`" Value=`"$pid_`" />" }
  L '      </RegistryKey>'
  L "      <RegistryValue Root=`"$root`" Key=`"Software\RegisteredApplications`" Type=`"string`" Name=`"$(X $a.name)`" Value=`"$cap`" />"
  L "      <RegistryValue Root=`"$root`" Key=`"Software\Microsoft\Windows\CurrentVersion\App Paths\$exe`" Type=`"string`" Value=`"[INSTALLFOLDER]$exe`" />"
  foreach ($e in $a.ext) { L "      <RegistryValue Root=`"$root`" Key=`"Software\Classes\.$e\OpenWithProgids`" Type=`"string`" Name=`"$pid_`" Value=`"`" />" }
  L '    </Component>'
}
L '  </ComponentGroup></Fragment>'
L '</Wix>'
[IO.File]::WriteAllText("$pk\suite.wxs", $sb.ToString(), [Text.UTF8Encoding]::new($false))

# ---- GPO default associations (documented, NOT deployed; packaging/windows/README.md). B-11: Docs for docx/doc/rtf/odt, Slides for pptx,
# PDF for pdf. Excel stays the .xlsx default until the Excel-workload track, so no Sheets row.
$gpo = [Collections.Generic.List[string]]::new()
$gpo.Add('<?xml version="1.0" encoding="UTF-8"?>'); $gpo.Add('<!-- GENERATED by scripts/build-msi.ps1 from brand/brand.toml. Not deployed. See packaging/windows/README.md. -->'); $gpo.Add('<DefaultAssociations>')
foreach ($row in @(@('docs', 'docx doc rtf odt'), @('slides', 'pptx'), @('pdf', 'pdf'))) {
  $a = $apps | Where-Object k -eq $row[0]
  foreach ($e in $row[1] -split ' ') { $gpo.Add("  <Association Identifier=`".$e`" ProgId=`"$($a.progid)`" ApplicationName=`"$(X $a.name)`" />") }
}
$gpo.Add('</DefaultAssociations>')
[IO.File]::WriteAllText("$pk\default-associations.xml", ($gpo -join "`n") + "`n", [Text.UTF8Encoding]::new($false))

# ---- WiX
$msi = Join-Path $out "$id-$version-x64.msi"
New-Item -ItemType Directory -Force $out | Out-Null
$env:TEMP = $out; $env:TMP = $out   # keep wix scratch off C:
& $Wix build -arch x64 -d "Stage=$stage" -o $msi "$pk\suite.wxs"
if ($LASTEXITCODE) { throw "wix build failed ($LASTEXITCODE)" }
if ($SignThumbprint) { & "$PSScriptRoot\sign.ps1" -Thumbprint $SignThumbprint -Store LocalMachine $msi; if ($LASTEXITCODE) { throw 'signing the msi failed' } }
$h = (Get-FileHash $msi -Algorithm SHA256).Hash
"MSI $msi  $([math]::Round((Get-Item $msi).Length / 1MB, 1)) MB  sha256 $h"
"ProductCode $productCode  UpgradeCode $upgradeCode  version $version"
