# Tool discovery shared by build-msi.ps1 and the tests (dot-source it). No machine-specific paths: an explicit value,
# then an environment variable, then the standard install locations (vswhere, PATH, dotnet global tools).

function Find-Csc {
  if ($env:CSC) { return $env:CSC }
  $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
  if (Test-Path $vswhere) {
    $c = & $vswhere -latest -products '*' -find 'MSBuild\**\Bin\Roslyn\csc.exe' | Select-Object -First 1
    if ($c) { return $c }
  }
  throw 'csc.exe not found: install Visual Studio or the Build Tools (MSBuild), or set CSC'
}

function Find-Wix([string]$Explicit) {
  if ($Explicit) { return $Explicit }
  if ($env:WIX) { return $env:WIX }
  $c = Get-Command wix.exe -ErrorAction SilentlyContinue
  if ($c) { return $c.Source }
  $t = Join-Path $env:USERPROFILE '.dotnet\tools\wix.exe'
  if (Test-Path $t) { return $t }
  throw 'wix.exe not found: dotnet tool install --global wix --version 5.0.2, or pass -Wix / set WIX'
}

function Get-OfficeTarget {
  if ($env:OFFICE_TARGET) { return $env:OFFICE_TARGET }
  throw 'set OFFICE_TARGET (the build output root, as for build.sh)'
}
