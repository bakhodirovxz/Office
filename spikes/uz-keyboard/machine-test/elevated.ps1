# Spike only (docs/keyboard/PLAN.md 11.3). Runs elevated once: registers the spike TIP per machine, waits for the
# non-elevated test runner to finish (file "done", at most 40 min), then removes everything it created. One UAC prompt.
$ErrorActionPreference = 'Continue'
$M = 'E:\office-target\kbd-spike\m'
$P = 'C:\Program Files\uzkbd-spike'
# A run folder of its own: a DLL left loaded by an earlier run (Explorer keeps it until restart) must not block this one.
$D = "$P\run-$(Get-Date -Format yyyyMMddHHmmss)"
$C = '{71A6784F-8ECA-4FC0-92C0-0B1756DE11CB}'
$log = "$M\elevated.log"
function L($s) { Add-Content -Path $log -Value ("{0:HH:mm:ss} {1}" -f (Get-Date), $s) -Encoding UTF8 }
function RegRun([string[]]$a) { $o = & reg.exe @a 2>&1; L ("reg " + ($a -join ' ') + " -> $LASTEXITCODE " + ($o -join ' ')); return $LASTEXITCODE }
function Exists($k, $v) { & reg.exe query $k "/reg:$v" *> $null; return ($LASTEXITCODE -eq 0) }

L "elevated start, user $env:USERNAME"
New-Item -ItemType Directory -Force "$M\hklm" | Out-Null
& reg.exe export 'HKLM\SOFTWARE\Microsoft\CTF' "$M\hklm\ctf64-before.reg" /y /reg:64 | Out-Null
& reg.exe export 'HKLM\SOFTWARE\WOW6432Node\Microsoft\CTF' "$M\hklm\ctf32-before.reg" /y /reg:64 | Out-Null
foreach ($v in '64', '32') {
    L "before /reg:$v CLSID exists=$(Exists "HKLM\SOFTWARE\Classes\CLSID\$C" $v) CTF\TIP exists=$(Exists "HKLM\SOFTWARE\Microsoft\CTF\TIP\$C" $v)"
}
L "before folder exists=$(Test-Path $D)"
try {
    if (Test-Path $D) { throw "folder already exists" }
    New-Item -ItemType Directory "$D\x64", "$D\x86" | Out-Null
    Copy-Item "$M\stage\x64\*" "$D\x64\"
    Copy-Item "$M\stage\x86\*" "$D\x86\"
    foreach ($p in @(@('64', 'x64'), @('32', 'x86'))) {
        $k = "HKLM\SOFTWARE\Classes\CLSID\$C"
        if ((RegRun @('add', $k, '/ve', '/d', 'Uzbek 2026 letters (spike)', '/f', "/reg:$($p[0])")) -ne 0) { throw "reg add" }
        if ((RegRun @('add', "$k\InprocServer32", '/ve', '/d', "$D\$($p[1])\uzkbd_spike.dll", '/f', "/reg:$($p[0])")) -ne 0) { throw "reg add" }
        if ((RegRun @('add', "$k\InprocServer32", '/v', 'ThreadingModel', '/d', 'Apartment', '/f', "/reg:$($p[0])")) -ne 0) { throw "reg add" }
    }
    foreach ($a in 'x64', 'x86') {
        $o = & "$D\$a\kbdctl.exe" machine-register 2>&1
        L "machine-register $a -> $LASTEXITCODE | $($o -join ' | ')"
        if ($LASTEXITCODE -ne 0) { throw "machine-register $a" }
    }
    Set-Content "$M\registered.ok" 'ok'
    L "registered; waiting for done"
    $t0 = Get-Date
    while (-not (Test-Path "$M\done") -and ((Get-Date) - $t0).TotalMinutes -lt 40) { Start-Sleep 2 }
    L "done seen=$(Test-Path "$M\done")"
}
catch { L "ERROR $_"; Set-Content "$M\failed" "$_" }

# Cleanup, always.
foreach ($a in 'x64', 'x86') {
    if (Test-Path "$D\$a\kbdctl.exe") { $o = & "$D\$a\kbdctl.exe" machine-unregister 2>&1; L "machine-unregister $a -> $LASTEXITCODE | $($o -join ' | ')" }
}
foreach ($v in '64', '32') {
    foreach ($k in "HKLM\SOFTWARE\Classes\CLSID\$C", "HKLM\SOFTWARE\Microsoft\CTF\TIP\$C") {
        if (Exists $k $v) { RegRun @('delete', $k, '/f', "/reg:$v") | Out-Null }
    }
}
$holders = Get-Process | Where-Object { try { $_.Modules | Where-Object { $_.FileName -like "$D*" } } catch { $null } } | ForEach-Object { "$($_.Name):$($_.Id)" }
L "DLL still loaded in (64-bit view): $($holders -join ', ')"
for ($i = 0; $i -lt 10 -and (Test-Path $D); $i++) {
    try { Remove-Item -Recurse -Force $D -ErrorAction Stop } catch { L "folder remove try $i failed: $_"; Start-Sleep 3 }
}
if ((Test-Path $P) -and -not (Get-ChildItem $P)) { Remove-Item $P }
L "after folder exists=$(Test-Path $D) (left in $P from earlier runs: $((Get-ChildItem $P -ErrorAction SilentlyContinue).Name -join ', '))"
foreach ($v in '64', '32') {
    L "after /reg:$v CLSID exists=$(Exists "HKLM\SOFTWARE\Classes\CLSID\$C" $v) CTF\TIP exists=$(Exists "HKLM\SOFTWARE\Microsoft\CTF\TIP\$C" $v)"
}
& reg.exe export 'HKLM\SOFTWARE\Microsoft\CTF' "$M\hklm\ctf64-after.reg" /y /reg:64 | Out-Null
& reg.exe export 'HKLM\SOFTWARE\WOW6432Node\Microsoft\CTF' "$M\hklm\ctf32-after.reg" /y /reg:64 | Out-Null
Set-Content "$M\cleaned.ok" 'ok'
L "elevated end"
