#!/usr/bin/env bash
# Self-check of the 2-slot build lock (scripts/lib/lock.sh): two target dirs overlap, a third waits,
# the same target dir never overlaps, a dead owner's slot is reclaimed. No cargo, runs in ~10 s.
set -uo pipefail
. "$(dirname "$0")/lib/lock.sh"
export OFFICE_TARGET; OFFICE_TARGET=$(mktemp -d) OFFICE_LOCK_POLL=1
unset OFFICE_LOCK_HELD
ms() { date +%s%3N; }
job() { # job <name> <target> : records start/end ms in $OFFICE_TARGET/<name>
  ( export CARGO_TARGET_DIR="$2"; with_build_lock bash -c "echo \$(date +%s%3N) > '$OFFICE_TARGET/$1.s'; sleep 3; echo \$(date +%s%3N) > '$OFFICE_TARGET/$1.e'" ) >/dev/null 2>&1
}
st() { cat "$OFFICE_TARGET/$1.s"; }; en() { cat "$OFFICE_TARGET/$1.e"; }
fail=0; chk() { if eval "$2"; then echo "ok   $1"; else echo "FAIL $1"; fail=1; fi; }

job A /t/a & sleep 0.3; job B /t/b & sleep 0.3; job C /t/c & wait
chk "A and B overlap (two slots granted)" '[ "$(st B)" -lt "$(en A)" ]'
chk "C starts only after a slot frees (third waits)" '[ "$(st C)" -ge "$(en A)" ]'

job D /t/d & sleep 0.3; job E /t/d & wait
chk "same target dir never builds twice" '[ "$(st E)" -ge "$(en D)" ] || [ "$(st D)" -ge "$(en E)" ]'

mkdir "$(_lock_base).1"; echo 999999 > "$(_lock_base).1/pid"; echo /t/z > "$(_lock_base).1/target"
job F /t/z; chk "stale slot (dead pid) is reclaimed" '[ -f "$OFFICE_TARGET/F.e" ]'
rm -rf "$OFFICE_TARGET"; exit $fail
