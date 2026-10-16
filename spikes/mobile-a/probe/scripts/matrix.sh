#!/bin/bash
# usage: matrix.sh shim|stock [renderer] -- scripted a..h run with per-test logs under $EV/logs
source /c/Users/uge226/AppData/Local/Temp/claude/c--Users-uge226-Desktop-office/b68ec07d-bc0a-419a-9f7a-096aa34db428/scratchpad/env.sh
MODE=$1; REND=${2:-glow}; TAG=$MODE-$REND
mkdir -p $EV/logs
if [ "$MODE" = shim ]; then F=on; else F=off; fi
adb shell am force-stop com.office.probe
G=com.google.android.inputmethod.latin/com.android.inputmethod.latin.LatinIME; adb shell am force-stop com.google.android.inputmethod.latin; adb shell ime disable $G >/dev/null; sleep 1; adb shell ime enable $G >/dev/null; adb shell ime set $G >/dev/null; sleep 3
adb shell run-as com.office.probe sh -c "'echo $F > files/shim; echo ${REND%%+*} > files/renderer; if [ \"$REND\" = wgpu+gl ]; then echo gl > files/backend; else rm -f files/backend; fi'"
adb shell am start -W -n com.office.probe/.MainActivity | grep TotalTime
sleep 6
kbready
t() { # name; reads actions from remaining args as a command string
  name=$1; shift
  adb logcat -c
  eval "$@"
  sleep 1.5
  adb logcat -d -s probe | grep -v mesa | cut -c19- > $EV/logs/$TAG-$name.txt
  shot $TAG-$name 0.4 > /dev/null
}
golang en
t e-autocorrect 'keys en "teh "'
t b-latin 'keys en "hello world"'
t f-glide 'glide en world 14'
t g-backspace 'keys en " abc"; adb logcat -c; keys en "<BS><BS><BS>"'
t h-enter 'keys en "<ENTER>"; sleep 0.5; keys en "ok"'
golang uz
t d-uzbek 'keys uz " <o'"'"'><g'"'"'>il"'
golang ru
t c-cyrillic 'keys ru " <SHIFT>салом дун"; yo'
t a-back-hides 'back; sleep 1; adb shell dumpsys input_method | grep mInputShown > $EV/logs/'$TAG'-a-inputshown.txt'
adb shell dumpsys meminfo com.office.probe > $EV/logs/$TAG-meminfo.txt
