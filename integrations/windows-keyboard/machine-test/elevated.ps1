# Product TIP machine test, elevated part (run.sh starts it through one UAC prompt). Registers the DLLs the way the MSI
# will (regsvr32 = DllRegisterServer, x64 and x86; KbdInstallForDefaultUser), waits for the non-elevated runner (file
# "done", at most 40 min), then removes everything in the same process: no second prompt.
$ErrorActionPreference = 'Continue'
$M = 'E:\office-target\kbd-tip\m'
$Root = 'C:\Program Files\kbd-machine-test'
# A run folder of its own: a DLL still loaded from an earlier run (Explorer keeps it until restart) must not block this one.
$D = "$Root\run-$(Get-Date -Format yyyyMMddHHmmss)"
$C = '{E384AABE-7F88-4BB2-BFB5-3AA607601E3E}'
$K = "$M\stage\x64\kbd-harness.exe"
$log = "$M\elevated.log"
function L($s) { Add-Content -Path $log -Value ("{0:HH:mm:ss} {1}" -f (Get-Date), $s) -Encoding UTF8 }
function Exists($k, $v) { & reg.exe query $k "/reg:$v" *> $null; return ($LASTEXITCODE -eq 0) }
# regsvr32 exit codes: 0 ok, 3 LoadLibrary failed, 4 export missing, 5 the export returned an error.
function RegSvr($exe, $dll, [switch]$u) {
    $a = @('/s'); if ($u) { $a += '/u' }; $a += "`"$dll`""
    $p = Start-Process -FilePath $exe -ArgumentList $a -Wait -PassThru -WindowStyle Hidden
    L "$exe $($a -join ' ') -> $($p.ExitCode)"; return $p.ExitCode
}
function Call($export) { $o = & $K call "$D\x64\kbd.dll" $export 2>&1; L "$export -> $LASTEXITCODE | $($o -join ' | ')"; return $LASTEXITCODE }
# The default-user hive as a .reg export (load, export, unload under a name of our own).
function DefUser($tag) {
    $hive = Join-Path (Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\ProfileList').Default 'NTUSER.DAT'
    $hive = [Environment]::ExpandEnvironmentVariables($hive)
    & reg.exe load 'HKU\kbdtest-snap' $hive *> $null
    if ($LASTEXITCODE -ne 0) { L "defuser ${tag}: reg load failed ($hive)"; Set-Content "$M\hklm\defuser-$tag.reg" 'load failed'; return }
    & reg.exe export 'HKU\kbdtest-snap\Software\Microsoft\CTF' "$M\hklm\defuser-ctf-$tag.reg" /y *> $null
    & reg.exe export 'HKU\kbdtest-snap\Keyboard Layout' "$M\hklm\defuser-$tag.reg" /y *> $null
    Get-Content "$M\hklm\defuser-ctf-$tag.reg" -ErrorAction SilentlyContinue | Add-Content "$M\hklm\defuser-$tag.reg"
    Remove-Item "$M\hklm\defuser-ctf-$tag.reg" -ErrorAction SilentlyContinue
    [gc]::Collect(); Start-Sleep 1
    & reg.exe unload 'HKU\kbdtest-snap' *> $null; L "defuser ${tag}: exported, unload -> $LASTEXITCODE"
}
function Snap($tag) {
    & reg.exe export 'HKLM\SOFTWARE\Microsoft\CTF' "$M\hklm\ctf64-$tag.reg" /y /reg:64 | Out-Null
    & reg.exe export 'HKLM\SOFTWARE\WOW6432Node\Microsoft\CTF' "$M\hklm\ctf32-$tag.reg" /y /reg:64 | Out-Null
    foreach ($v in '64', '32') {
        L "$tag /reg:$v CLSID exists=$(Exists "HKLM\SOFTWARE\Classes\CLSID\$C" $v) CTF\TIP exists=$(Exists "HKLM\SOFTWARE\Microsoft\CTF\TIP\$C" $v)"
    }
    DefUser $tag
}

L "elevated start, user $env:USERNAME"
New-Item -ItemType Directory -Force "$M\hklm" | Out-Null
Snap 'before'
try {
    if (Test-Path $D) { throw "folder already exists" }
    New-Item -ItemType Directory "$D\x64", "$D\x86" | Out-Null
    Copy-Item "$M\stage\x64\kbd.dll" "$D\x64\"
    Copy-Item "$M\stage\x86\kbd.dll" "$D\x86\"
    Set-Content "$M\dll-x64.txt" "$D\x64\kbd.dll" -NoNewline
    if ((RegSvr "$env:WINDIR\System32\regsvr32.exe" "$D\x64\kbd.dll") -ne 0) { throw "regsvr32 x64" }
    if ((RegSvr "$env:WINDIR\SysWOW64\regsvr32.exe" "$D\x86\kbd.dll") -ne 0) { throw "regsvr32 x86" }
    foreach ($v in '64', '32') {
        $ip = (& reg.exe query "HKLM\SOFTWARE\Classes\CLSID\$C\InprocServer32" /ve "/reg:$v" 2>&1) -join ' '
        L "registered /reg:$v InprocServer32: $ip"
    }
    if ((Call 'KbdInstallForDefaultUser') -ne 0) { L "KbdInstallForDefaultUser FAILED (continuing: the user test does not need it)" }
    DefUser 'during'
    Set-Content "$M\registered.ok" 'ok'
    L "registered; waiting for done"
    $t0 = Get-Date
    while (-not (Test-Path "$M\done") -and ((Get-Date) - $t0).TotalMinutes -lt 40) { Start-Sleep 2 }
    L "done seen=$(Test-Path "$M\done")"
}
catch { L "ERROR $_"; Set-Content "$M\failed" "$_" }

# Cleanup, always: the product's own reverse steps first, then a check that nothing is left.
if (Test-Path "$D\x64\kbd.dll") { Call 'KbdUninstallForDefaultUser' | Out-Null }
if (Test-Path "$D\x64\kbd.dll") { RegSvr "$env:WINDIR\System32\regsvr32.exe" "$D\x64\kbd.dll" -u | Out-Null }
if (Test-Path "$D\x86\kbd.dll") { RegSvr "$env:WINDIR\SysWOW64\regsvr32.exe" "$D\x86\kbd.dll" -u | Out-Null }
foreach ($v in '64', '32') {
    foreach ($k in "HKLM\SOFTWARE\Classes\CLSID\$C", "HKLM\SOFTWARE\Microsoft\CTF\TIP\$C") {
        if (Exists $k $v) { L "LEFT AFTER UNREGISTER: $k /reg:$v (deleting)"; & reg.exe delete $k /f "/reg:$v" | Out-Null }
    }
}
$holders = Get-Process | Where-Object { try { $_.Modules | Where-Object { $_.FileName -like "$D*" } } catch { $null } } | ForEach-Object { "$($_.Name):$($_.Id)" }
L "DLL still loaded in (64-bit view): $($holders -join ', ')"
for ($i = 0; $i -lt 10 -and (Test-Path $D); $i++) {
    try { Remove-Item -Recurse -Force $D -ErrorAction Stop } catch { L "folder remove try $i failed: $_"; Start-Sleep 3 }
}
if ((Test-Path $Root) -and -not (Get-ChildItem $Root)) { Remove-Item $Root }
L "after folder exists=$(Test-Path $D) (left in $Root from earlier runs: $((Get-ChildItem $Root -ErrorAction SilentlyContinue).Name -join ', '))"
Snap 'after'
Set-Content "$M\cleaned.ok" 'ok'
L "elevated end"
