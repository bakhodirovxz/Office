# Spike 3 B3 harness (throwaway), adapted from spikes/mobile-a/probe/scripts/env.sh
export MSYS_NO_PATHCONV=1 ANDROID_HOME="$LOCALAPPDATA/Android/Sdk"
export PATH="$ANDROID_HOME/platform-tools:$ANDROID_HOME/emulator:$HOME/.cargo/bin:/e/office-tools/cargo/bin:$PATH"
SP=C:/Users/uge226/Desktop/office/spikes/mobile-b/scripts
EV=C:/Users/uge226/Desktop/office/docs/phase0/mobile-b
shot() { python -I $SP/shot.py "$@"; }
keys() { python -I $SP/keys.py "$@"; }
glide() { python -I $SP/glide.py "$@"; }
lang() { python -I -W ignore $SP/lang.py; }
yo() { adb shell "input motionevent DOWN 442 1718; sleep 1.2; input motionevent MOVE 556 1549; sleep 0.3; input motionevent UP 556 1549"; }
G=com.google.android.inputmethod.latin/com.android.inputmethod.latin.LatinIME
fixkb() { adb shell am force-stop com.google.android.inputmethod.latin; adb shell ime disable $G >/dev/null; sleep 1; adb shell ime enable $G >/dev/null; adb shell ime set $G >/dev/null; sleep 3; }
golang() { for i in 1 2 3 4; do cur=$(python -I -W ignore $SP/lang.py | tail -1); [ "$cur" = "$1" ] && return 0; adb shell input tap 327 2178; sleep 1.7; done; echo "golang $1 failed (cur=$cur)"; }
back() { adb shell input swipe 3 1400 500 1400 150; }
