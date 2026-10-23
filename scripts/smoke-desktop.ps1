# Desktop host smoke (PLAN phase 4, WP9): start hoshiya.exe with a docx on the command line, record the time to the window and the
# time until the local server answers, the memory of the host and its WebView2 processes, check the editor was started with the
# Cyrillic path, then launch a second hoshiya.exe with an xlsx and check it was handed to the running instance. Kills only what it started.
#   powershell -File scripts/smoke-desktop.ps1 [-Exe <hoshiya.exe>] [-Dist <dist dir>] [-Docs <docs.exe>] [-Sheets <sheets.exe>]
# The editors here are the unbranded Phase 1 desktop builds, copied under the brand's exe names into a temp bin dir.
param(
  [string]$Exe = "E:\office-target\p1\services\debug\hoshiya.exe",
  [string]$Dist = "E:\office-target\p1\dist",
  [string]$Docs = "E:\office-target\p1\docs\release\docs.exe",
  [string]$Sheets = "E:\office-target\p1\sheets\release\sheets.exe",
  [string]$Docx = "$PSScriptRoot\..\docs\phase0\mobile-b\d-saved-desktop.docx",
  [int]$Settle = 6
)
$ErrorActionPreference = 'Stop'
$OrigExe = $Exe
$root = Join-Path $env:TEMP ("suite-smoke-" + [guid]::NewGuid().ToString('N').Substring(0, 8))
$work = Join-Path $root 'Рабочий стол'; $bin = Join-Path $root 'bin'; $data = Join-Path $root 'data'
New-Item -ItemType Directory -Force $work, $bin, $data | Out-Null
$brand = (Resolve-Path "$PSScriptRoot\..\brand\brand.toml").Path
$binDocs = ((Get-Content $brand -Raw) -split '\[apps\.docs\]')[1] -match 'bin = "([^"]+)"' | ForEach-Object { $Matches[1] }
$binSheets = ((Get-Content $brand -Raw) -split '\[apps\.sheets\]')[1] -match 'bin = "([^"]+)"' | ForEach-Object { $Matches[1] }
Copy-Item $Docs (Join-Path $bin "$binDocs.exe"); Copy-Item $Sheets (Join-Path $bin "$binSheets.exe")
$doc = Join-Path $work 'Буйруқ 1.docx'; Copy-Item $Docx $doc
$xlsx = Join-Path $work 'Hisobot 2026.xlsx'
# a minimal valid xlsx is not available offline: the editor may refuse it, the hand-off is what is checked
[IO.File]::WriteAllBytes($xlsx, [byte[]](0x50, 0x4B, 5, 6) + (New-Object byte[] 18))

# the windows-gnu build imports WebView2Loader.dll (the msvc build links it statically): it must sit next to hoshiya.exe
$app = Join-Path $root 'app'; New-Item -ItemType Directory -Force $app | Out-Null
Copy-Item $Exe (Join-Path $app 'hoshiya.exe'); $Exe = Join-Path $app 'hoshiya.exe'
$loader = Join-Path (Split-Path $Exe) 'WebView2Loader.dll'
$src = if (Test-Path (Join-Path (Split-Path $OrigExe) 'WebView2Loader.dll')) { Join-Path (Split-Path $OrigExe) 'WebView2Loader.dll' } else { (Get-ChildItem -Recurse (Join-Path (Split-Path $OrigExe) 'build') -Filter WebView2Loader.dll | Where-Object { $_.Directory.Name -eq 'x64' } | Select-Object -First 1).FullName }
Copy-Item $src $loader
$env:SUITE_DATA_DIR = $data; $env:SUITE_INSTANCE = 'smoke' + $PID
$common = @('--dist', $Dist, '--brand', $brand, '--bin-dir', $bin)
function Tree($rootPid) {
  $all = Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId, Name
  $ids = @($rootPid); $grow = $true
  while ($grow) { $new = @($all | Where-Object { $ids -contains $_.ParentProcessId -and $ids -notcontains $_.ProcessId } | ForEach-Object ProcessId); $grow = $new.Count -gt 0; $ids += $new }
  $all | Where-Object { $ids -contains $_.ProcessId }
}
$sw = [Diagnostics.Stopwatch]::StartNew()
$argsFor = { param($f) (@($common) + $f | ForEach-Object { '"' + $_ + '"' }) -join ' ' }
$p = Start-Process -FilePath $Exe -ArgumentList (& $argsFor $doc) -PassThru
try {
  $tWin = $null; $tServe = $null
  while ($sw.Elapsed.TotalSeconds -lt 60 -and -not ($tWin -and $tServe)) {
    $p.Refresh()
    if (-not $tWin -and $p.MainWindowHandle -ne 0) { $tWin = $sw.ElapsedMilliseconds }
    $hj = Join-Path $data 'host.json'
    if (-not $tServe -and (Test-Path $hj)) {
      $port = (Get-Content $hj -Raw | ConvertFrom-Json).port
      # the server answers 403 without the per-launch key cookie (review M1): any HTTP answer means it is up
      try { Invoke-WebRequest "http://127.0.0.1:$port/shell-config.json" -UseBasicParsing | Out-Null } catch { if ($_.Exception.Response -and [int]$_.Exception.Response.StatusCode -eq 403) { $tServe = $sw.ElapsedMilliseconds; $noKey = 403 } }
    }
    Start-Sleep -Milliseconds 25
  }
  Start-Sleep -Seconds $Settle
  $p.Refresh()
  $tree = Tree $p.Id
  $mb = [math]::Round((($tree | ForEach-Object { (Get-Process -Id $_.ProcessId -ErrorAction SilentlyContinue).WorkingSet64 } | Measure-Object -Sum).Sum) / 1MB)
  $host_mb = [math]::Round($p.WorkingSet64 / 1MB)
  $wv = @($tree | Where-Object Name -eq 'msedgewebview2.exe')
  $wvmb = [math]::Round((($wv | ForEach-Object { (Get-Process -Id $_.ProcessId -ErrorAction SilentlyContinue).WorkingSet64 } | Measure-Object -Sum).Sum) / 1MB)
  $ed = @($tree | Where-Object { $_.Name -eq "$binDocs.exe" })
  $title = (Get-Process -Id $p.Id).MainWindowTitle
  "window_title=[$title] host_ws_mb=$host_mb webview2_procs=$($wv.Count) webview2_ws_mb=$wvmb"
  "start_to_window_ms=$tWin start_to_server_ms=$tServe request_without_key=$noKey"
  "editor_started($binDocs)=$($ed.Count -ge 1) tree_ws_mb_incl_editor=$mb"
  # second launch: must exit 0 and start the Sheets editor from the running instance
  $sw2 = [Diagnostics.Stopwatch]::StartNew()
  $p2 = Start-Process -FilePath $Exe -ArgumentList (& $argsFor $xlsx) -PassThru -Wait
  "second_launch_exit=$($p2.ExitCode) second_launch_ms=$($sw2.ElapsedMilliseconds)"
  Start-Sleep -Seconds 2
  $tree = Tree $p.Id
  "second_file_to_running_instance($binSheets)=$((@($tree | Where-Object { $_.Name -eq "$binSheets.exe" })).Count -ge 1) suite_exe_count=$((@(Get-Process -Name suite -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $Exe })).Count)"
} finally {
  # only what this script started: the host and everything below it
  foreach ($t in (Tree $p.Id | Sort-Object ProcessId -Descending)) { Stop-Process -Id $t.ProcessId -Force -ErrorAction SilentlyContinue }
  Start-Sleep -Milliseconds 500
  Remove-Item -Recurse -Force $root -ErrorAction SilentlyContinue
}
