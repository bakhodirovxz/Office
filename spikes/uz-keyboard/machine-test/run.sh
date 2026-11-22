#!/bin/bash
# Spike machine test runner (non-elevated). Waits for an unlocked desktop (max 20 min), snapshots HKCU, asks for ONE
# UAC prompt (elevated.ps1 registers, waits, cleans), adds the profile to the user's list, runs the suite, restores
# the list, signals the elevated side to clean up, then compares the snapshots.
SRC="$(cd "$(dirname "$0")" && pwd)"
B=E:/office-target/kbd-spike
M=$B/m
MW='E:\office-target\kbd-spike\m'
exec > >(tee "$B/run-last.log") 2>&1  # the coordinator reads the result from here
rm -rf "$M"; mkdir -p "$M/stage/x64" "$M/stage/x86" "$M/hkcu"
cp $B/release/uzkbd_spike.dll $B/release/kbdctl.exe "$M/stage/x64/"
cp $B/i686-pc-windows-msvc/release/uzkbd_spike.dll $B/i686-pc-windows-msvc/release/kbdctl.exe "$M/stage/x86/"
cp "$SRC/elevated.ps1" "$SRC/lang-add.ps1" "$SRC/lang-restore.ps1" "$M/"
K="$M/stage/x64/kbdctl.exe"

echo "=== $(date +%T) wait for an unlocked desktop (max 20 min; no keys are sent while locked)"
"$K" wait-unlock 20 || { echo "LOCKED: nothing registered, no keys sent"; exit 4; }

snap() {
  for k in 'HKCU\Software\Microsoft\CTF' 'HKCU\Keyboard Layout' 'HKCU\Control Panel\International' 'HKCU\Software\Microsoft\Input'; do
    n=$(echo "$k" | tr '\\ ' '__')
    MSYS_NO_PATHCONV=1 reg export "$k" "$M/hkcu/$n-$1.reg" /y >/dev/null 2>&1 || echo "export failed: $k ($1)"
  done
  powershell -NoProfile -Command "Get-WinUserLanguageList | ForEach-Object { \$_.LanguageTag + ' ' + (\$_.InputMethodTips -join ',') }" > "$M/hkcu/langlist-$1.txt"
}
snap before
echo "=== language list before"; cat "$M/hkcu/langlist-before.txt"

echo "=== $(date +%T) UAC prompt (elevated.ps1)"
timeout 900 powershell -NoProfile -Command "Start-Process powershell -Verb RunAs -WindowStyle Hidden -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File','$MW\\elevated.ps1'" \
  || { echo "UAC refused, timed out or failed: nothing registered"; exit 5; }
for i in $(seq 1 90); do { [ -f "$M/registered.ok" ] || [ -f "$M/failed" ]; } && break; sleep 2; done
if [ ! -f "$M/registered.ok" ]; then
  echo "registration FAILED"; cat "$M/elevated.log"
  for i in $(seq 1 90); do [ -f "$M/cleaned.ok" ] && break; sleep 2; done
  exit 6
fi
echo "=== $(date +%T) registered"; cat "$M/elevated.log"
"$K" enum

echo "=== $(date +%T) add the profile to the user's language list"
powershell -NoProfile -ExecutionPolicy Bypass -File "$MW\\lang-add.ps1"
sleep 2

echo "=== $(date +%T) suite"
timeout 900 "$K" suite notepad wordpad edge
echo "suite exit=$?"
echo "=== TIP log (activations and errors)"
grep -E "activate|error|terminated|probe" "$LOCALAPPDATA/Temp/uzkbd-spike.log" | sort | uniq -c | head -40
echo "=== TIP-side per-key time (us), all keys"
grep -oE "us=[0-9]+" "$LOCALAPPDATA/Temp/uzkbd-spike.log" | cut -d= -f2 | sort -n | awk '{a[NR]=$1} END {if (NR) printf "n=%d median=%d p95=%d max=%d\n", NR, a[int((NR+1)/2)], a[int(NR*0.95+0.5)], a[NR]}'

echo "=== $(date +%T) restore the language list"
powershell -NoProfile -ExecutionPolicy Bypass -File "$MW\\lang-restore.ps1"
sleep 2
touch "$M/done"
for i in $(seq 1 150); do [ -f "$M/cleaned.ok" ] && break; sleep 2; done
echo "=== $(date +%T) elevated log"; cat "$M/elevated.log"

snap after
echo "=== comparison"
for f in "$M"/hkcu/*-before.reg "$M"/hkcu/langlist-before.txt "$M"/hklm/*-before.reg; do
  a="${f/-before/-after}"
  if cmp -s "$f" "$a"; then echo "identical: $(basename "$f")"; else
    echo "DIFFERENT: $(basename "$f")"
    diff <(iconv -f UTF-16LE -t UTF-8 "$f" 2>/dev/null || cat "$f") <(iconv -f UTF-16LE -t UTF-8 "$a" 2>/dev/null || cat "$a") | head -30
  fi
done
ls -R "C:/Program Files/uzkbd-spike" 2>&1 | head -6
rm -f "$LOCALAPPDATA/Temp/uzkbd-spike.log"
rm -rf "$M/edge-ud" "$M/stage"
echo "=== $(date +%T) end"
