# WP23 usability test: reset the CURRENT user's product state between participants (docs/phase4/usability.md).
#   powershell -ExecutionPolicy Bypass -File scripts\wp23-reset.ps1            # dry run: lists what it would delete, deletes nothing
#   powershell -ExecutionPolicy Bypass -File scripts\wp23-reset.ps1 -Yes       # does it
#   options: -WorkDir <dir> (default %USERPROFILE%\Desktop\WP23)  -Fixtures <dir>
# Deletes ONLY these folders (names come from brand\brand.toml, never a wildcard): under %APPDATA% and %LOCALAPPDATA% the folder named
# like the product, and the upstream-named folder of each app (the apps still keep their prefs there). Plus the contents of -WorkDir.
# It never kills a process: if a product app is running it stops and asks you to close it. Per-user only, no admin rights needed.
# Exit code: 0 ok / dry run, 2 bad input or an app is running.
param(
  [switch]$Yes,
  [string]$WorkDir = (Join-Path $env:USERPROFILE 'Desktop\WP23'),
  [string]$Fixtures = ''
)
$ErrorActionPreference = 'Stop'
# 5.1 leaves $PSScriptRoot empty in param defaults, so resolve the script dir here (as msi-test.ps1 does)
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
# the kit folder has brand.toml and fixtures next to the script; in the repo they are one level up / under docs
$brandFile = @((Join-Path $here 'brand.toml'), (Join-Path $here '..\brand\brand.toml')) | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
if (-not $brandFile) { Write-Host 'brand.toml not found (next to this script or in ..\brand)'; exit 2 }
if (-not $Fixtures) { $Fixtures = @((Join-Path $here 'fixtures'), (Join-Path $here '..\docs\phase4\wp23\fixtures')) | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1 }
if (-not $Fixtures -or -not (Test-Path -LiteralPath $Fixtures)) { Write-Host 'fixtures folder not found (pass -Fixtures)'; exit 2 }

# ---- names from brand.toml (the same regex reader as msi-test.ps1)
$toml = Get-Content -LiteralPath $brandFile -Raw -Encoding UTF8
function Sec($n) { [regex]::Match($toml, "(?ms)^\[$([regex]::Escape($n))\]\s*\r?\n(.*?)(?=^\[|\z)").Groups[1].Value }
function Key($s, $k) { [regex]::Match($s, "(?m)^$k\s*=\s*`"([^`"]*)`"").Groups[1].Value }
$product = Key (Sec 'product') 'name'
$names = New-Object System.Collections.Generic.List[string]
$bins = New-Object System.Collections.Generic.List[string]
$names.Add($product)
foreach ($a in 'docs', 'sheets', 'slides', 'pdf', 'convert') {
  $s = Sec "apps.$a"
  $bins.Add((Key $s 'bin'))
  $names.Add(((Key $s 'data_dir') -split '[/\\]')[0])   # "Hoshiya/Docs" -> the product folder again
  foreach ($u in [regex]::Matches([regex]::Match($s, '(?m)^upstream_names\s*=\s*\[([^\]]*)\]').Groups[1].Value, '"([^"]+)"')) { $names.Add($u.Groups[1].Value) }
}
$bins.Add(((Key (Sec 'apps.docs') 'bin') -replace '-docs$', ''))   # the host: hoshiya.exe, hoshiya-serve.exe
$bins.Add(((Key (Sec 'apps.docs') 'bin') -replace '-docs$', '-serve'))
$names = @($names | Where-Object { $_ } | Select-Object -Unique)
# a folder name must be a plain single name: no separators, no dots-only, no wildcards
foreach ($n in $names) { if ($n -notmatch '^[A-Za-z0-9][A-Za-z0-9 _-]*$') { Write-Host "refusing odd folder name from brand.toml: '$n'"; exit 2 } }
if ($names.Count -lt 2) { Write-Host 'brand.toml gave too few names, refusing'; exit 2 }

# ---- never kill, only ask
$running = @(Get-Process -ErrorAction SilentlyContinue | Where-Object { $bins -contains $_.ProcessName })
if ($running) { Write-Host ('These product programs are running: ' + (($running | ForEach-Object { $_.ProcessName + ' (' + $_.Id + ')' }) -join ', ')); Write-Host 'Close them (all windows) and run again. Nothing was changed.'; exit 2 }

# ---- the product folders that exist
$bases = @(@{ env = 'APPDATA'; p = $env:APPDATA }, @{ env = 'LOCALAPPDATA'; p = $env:LOCALAPPDATA })
$targets = New-Object System.Collections.Generic.List[string]
foreach ($b in $bases) {
  if (-not $b.p) { Write-Host "$($b.env) is not set"; exit 2 }
  foreach ($n in $names) {
    $p = Join-Path $b.p $n
    if (-not (Test-Path -LiteralPath $p)) { continue }
    $item = Get-Item -LiteralPath $p -Force
    if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { Write-Host "SKIP (link, not touched): $p"; continue }
    if ((Split-Path -Parent $item.FullName) -ne ([IO.Path]::GetFullPath($b.p).TrimEnd('\'))) { Write-Host "SKIP (not a direct child of $($b.env)): $p"; continue }
    $targets.Add($item.FullName)
  }
}

# ---- the work folder: must be clearly a WP23 folder, never a broad one
$wd = [IO.Path]::GetFullPath($WorkDir).TrimEnd('\')
if ((Split-Path -Leaf $wd) -notlike '*WP23*' -or ($wd -split '\\').Count -lt 4) { Write-Host "refusing -WorkDir '$wd': its last folder name must contain WP23 and it must be at least 3 levels deep"; exit 2 }

Write-Host ($(if ($Yes) { 'DELETING' } else { 'DRY RUN (nothing is deleted; add -Yes)' }))
Write-Host "brand: $brandFile  product: $product"
if ($targets.Count -eq 0) { Write-Host 'product folders: none exist' }
foreach ($t in $targets) {
  $m = Get-ChildItem -LiteralPath $t -Recurse -Force -File -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum
  Write-Host ('  delete folder {0}  ({1} files, {2} KB)' -f $t, $m.Count, [int]($m.Sum / 1KB))
}
$old = if (Test-Path -LiteralPath $wd) { @(Get-ChildItem -LiteralPath $wd -Force) } else { @() }
Write-Host "work folder $wd : $($old.Count) entries to clear, then copy the fixtures:"
foreach ($f in Get-ChildItem -LiteralPath $Fixtures -File) { Write-Host "  copy $($f.Name)" }
if (-not $Yes) { exit 0 }

foreach ($t in $targets) { Remove-Item -LiteralPath $t -Recurse -Force; if (Test-Path -LiteralPath $t) { Write-Host "COULD NOT remove $t"; exit 2 } }
New-Item -ItemType Directory -Force $wd | Out-Null
foreach ($e in $old) { Remove-Item -LiteralPath $e.FullName -Recurse -Force }
Copy-Item -Path (Join-Path $Fixtures '*') -Destination $wd -Force
Write-Host ('done: {0} product folders removed, work folder holds {1} fixture file(s)' -f $targets.Count, @(Get-ChildItem -LiteralPath $wd -File).Count)
