#!/bin/bash
# Product TIP machine test (docs/keyboard/PLAN.md 11.3, 11.4; adapted from spikes/uz-keyboard/machine-test). Not elevated.
#  1. waits for an unlocked desktop (max 20 min; no keys are sent while it is locked);
#  2. snapshots HKCU (CTF, Keyboard Layout, International, Input, the keyboard's settings key) and the language list;
#  3. ONE UAC prompt: elevated.ps1 copies the DLLs to a fresh run folder, regsvr32 (x64 + x86 = DllRegisterServer),
#     writes the MSI's Active Setup entry (HKLM, 64-bit view), then waits and cleans up (entry removed, regsvr32 /u) in
#     the same process;
#  4. the Active Setup entry's StubPath (rundll32 kbd.dll,KbdActiveSetup), run as this user, adds the keyboard to this
#     user's list, as Windows does for every user at their next logon; then KbdInstallForUser (the MSI's call for the
#     installing user), which must leave the list unchanged;
#  5. the SendInput suite: Notepad (CUAS), WordPad (TSF-aware RichEdit), Edge (Chromium), each plain then with the TIP,
#     then per app an attach check (does AttachThreadInput alone change the session's input? with the TIP, then with
#     another of the user's layouts);
#  6. KbdUninstallForUser, the language list restored as it was, the cleanup signalled, every snapshot compared.
# Build first (README.md). Output: E:/office-target/kbd-tip/run-last.log.
SRC="$(cd "$(dirname "$0")" && pwd)"
B=E:/office-target/kbd-tip
M=$B/m
MW='E:\office-target\kbd-tip\m'
for f in $B/release/kbd.dll $B/release/kbd-harness.exe $B/i686-pc-windows-msvc/release/kbd.dll; do
  [ -f "$f" ] || { echo "missing $f: build both architectures first (README.md)"; exit 2; }
done
exec > >(tee "$B/run-last.log") 2>&1  # the coordinator reads the result from here
rm -rf "$M"; mkdir -p "$M/stage/x64" "$M/stage/x86" "$M/hkcu"
cp $B/release/kbd.dll $B/release/kbd-harness.exe "$M/stage/x64/"
cp $B/i686-pc-windows-msvc/release/kbd.dll "$M/stage/x86/"
cp "$SRC/elevated.ps1" "$SRC/lang-restore.ps1" "$M/"
K="$M/stage/x64/kbd-harness.exe"
SETTINGS="HKCU\\$("$K" settings-key)"

echo "=== $(date +%T) wait for an unlocked desktop (max 20 min; no keys are sent while locked)"
"$K" wait-unlock 20 || { echo "LOCKED: nothing registered, no keys sent"; exit 4; }

snap() {
  for k in 'HKCU\Software\Microsoft\CTF' 'HKCU\Keyboard Layout' 'HKCU\Control Panel\International' 'HKCU\Software\Microsoft\Input' "$SETTINGS"; do
    n=$(echo "$k" | tr '\\ ' '__')
    MSYS_NO_PATHCONV=1 reg export "$k" "$M/hkcu/$n-$1.reg" /y >/dev/null 2>&1 || echo "absent" > "$M/hkcu/$n-$1.reg"
  done
  powershell -NoProfile -Command "Get-WinUserLanguageList | ForEach-Object { \$_.LanguageTag + ' ' + (\$_.InputMethodTips -join ',') }" > "$M/hkcu/langlist-$1.txt"
}
snap before
cp "$M/hkcu/langlist-before.txt" "$M/langlist-saved.txt"
echo "=== language list before"; cat "$M/hkcu/langlist-before.txt"

# The TIP log is on for the whole run, the elevated calls included (kbd-harness `call` also turns it on for itself);
# a copy is kept in $M/tip.log.
LOG="$("$K" log-path)"
rm -f "$LOG"
"$K" debug on

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
DLL="$(cat "$M/dll-x64.txt")"

echo "=== $(date +%T) Active Setup StubPath as this user (adds 0443:{clsid}{profile} to this user's list)"
cat "$M/stubpath.cmd"
cmd.exe //c "$MW\stubpath.cmd"; echo "StubPath exit=$?"
grep KbdActiveSetup "$LOG" | sed 's/^/  log: /'
powershell -NoProfile -Command "Get-WinUserLanguageList | ForEach-Object { \$_.LanguageTag + ' ' + (\$_.InputMethodTips -join ',') }"
echo "=== $(date +%T) KbdInstallForUser (already listed: no change expected)"
"$K" call "$DLL" KbdInstallForUser
powershell -NoProfile -Command "Get-WinUserLanguageList | ForEach-Object { \$_.LanguageTag + ' ' + (\$_.InputMethodTips -join ',') }"
"$K" enum
sleep 2

echo "=== $(date +%T) suite"
timeout 900 "$K" suite notepad wordpad edge
echo "suite exit=$?"
echo "=== TIP log (activations, toggles, errors, panics)"
grep -E "activate|foreground|error|terminated|PANIC|failed|open=" "$LOG" | sed -E 's/^[0-9]+ [0-9]+ //' | sort | uniq -c | head -40
echo "=== TIP-side per-key time (us), all keys"
grep -oE " us=[0-9]+" "$LOG" | cut -d= -f2 | sort -n | awk '{a[NR]=$1} END {if (NR) printf "n=%d median=%d p95=%d max=%d\n", NR, a[int((NR+1)/2)], a[int(NR*0.95+0.5)], a[NR]}'

echo "=== keys the TIP handled: $(grep -c 'key vk=' "$LOG") 'key vk=' lines; let through: $(grep -c ' rejected ' "$LOG")"
echo "=== layout (HKL) at activation, per process"
grep -oE "activate tid=.*" "$LOG" | sed -E 's/^activate tid=[0-9]+ //; s/settings=Settings \{[^}]*\} //' | sort | uniq -c | head
echo "=== keys let through (first 12; ch= is what the TIP read, via= which source answered)"
grep " rejected " "$LOG" | sed -E 's/^[0-9]+ [0-9]+ //' | head -12

echo "=== $(date +%T) KbdUninstallForUser, then restore the language list"
"$K" call "$DLL" KbdUninstallForUser
powershell -NoProfile -ExecutionPolicy Bypass -File "$MW\\lang-restore.ps1" "$MW\\langlist-saved.txt"
"$K" debug off
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
ls -R "C:/Program Files/kbd-machine-test" 2>&1 | head -6
cp "$LOG" "$M/tip.log" 2>/dev/null && echo "TIP log kept: $M/tip.log ($(wc -l < "$M/tip.log") lines)"
rm -f "$LOG"
rm -rf "$M/edge-ud" "$M/stage"
echo "=== $(date +%T) end"
