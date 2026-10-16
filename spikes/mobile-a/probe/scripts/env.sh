export MSYS_NO_PATHCONV=1 ANDROID_HOME="$LOCALAPPDATA/Android/Sdk"
export PATH="$ANDROID_HOME/platform-tools:$ANDROID_HOME/emulator:$PATH"
SP=C:/Users/uge226/AppData/Local/Temp/claude/c--Users-uge226-Desktop-office/b68ec07d-bc0a-419a-9f7a-096aa34db428/scratchpad
shot() { python -I $SP/shot.py "$@"; }
EV=C:/Users/uge226/Desktop/office/docs/phase0/mobile-a
APK=C:/Users/uge226/Desktop/office/spikes/mobile-a/probe/android/app/build/outputs/apk/debug/app-debug.apk
boot() { /e/office-target/mobile-a-gate.sh  # waits for no rustc + 3 GB free; (nohup emulator -avd HRM_Pixel_8 -no-snapshot-save -gpu swiftshader_indirect > /e/office-target/logs/mobile-a-emulator.log 2>&1 &); sleep 2; adb wait-for-device; until [ "$(adb shell getprop sys.boot_completed | tr -d '\r')" = "1" ]; do sleep 5; done; sleep 10; adb install -r $APK; }
keys() { python -I $SP/keys.py "$@"; }
clr() { adb shell input keycombination 113 29; adb shell input keyevent 67; }
yo() { adb shell "input motionevent DOWN 442 1718; sleep 1.2; input motionevent MOVE 556 1549; sleep 0.3; input motionevent UP 556 1549"; }
lang() { python -I -W ignore $SP/lang.py; }
glide() { python -I $SP/glide.py "$@"; }
setlang() { case $1 in en) y=1020;; ru) y=1210;; uz) y=1400;; esac; adb shell input swipe 327 2178 327 2178 900; sleep 1.2; adb shell input tap 425 $y; sleep 1.5; }
fixkb() { G=com.google.android.inputmethod.latin/com.android.inputmethod.latin.LatinIME; adb shell am force-stop com.google.android.inputmethod.latin; adb shell ime disable $G >/dev/null; sleep 1; adb shell ime enable $G >/dev/null; adb shell ime set $G >/dev/null; sleep 3; adb shell input tap 540 250; sleep 8; }
back() { adb shell input swipe 3 1400 500 1400 150; }
golang() { for i in 1 2 3 4; do cur=$(python -I -W ignore $SP/lang.py | tail -1); [ "$cur" = "$1" ] && return 0; adb shell input tap 327 2178; sleep 1.7; done; echo "golang $1 failed (cur=$cur)"; }

kbwait() { for i in 1 2 3 4 5 6 7 8; do cur=$(python -I -W ignore $SP/lang.py 2>/dev/null | tail -1); [ "$cur" != "none" ] && return 0; sleep 2; done; return 1; }
kbready() { for try in 1 2 3; do adb shell input tap 540 250; sleep 4; kbwait && return 0; fixkb; kbwait && return 0; done; echo "kbready failed"; return 1; }
