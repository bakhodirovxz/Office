#!/usr/bin/env bash
# curl-only server flow (no browser). Run from spikes/docserver with fake_hrm (8790) and
# docserver (8780) already running. Exit 0 = all checks passed.
set -euo pipefail
DS=http://127.0.0.1:8780
HRM=http://127.0.0.1:8790
FIX=fixtures/sample.docx
fail() { echo "FAIL: $*"; exit 1; }
sha() { sha256sum "$1" | cut -d' ' -f1; }
j() { python -I -c "import sys,json; d=json.load(sys.stdin); print(eval(sys.argv[1]))" "$1"; }

[ "$(curl -s $DS/healthz)" = ok ] || fail healthz
curl -s $HRM/config/sample.docx -o /tmp/cfg.json 2>/dev/null || curl -s $HRM/config/sample.docx -o "${TMPDIR:-.}/cfg.json"
CFG="${TMPDIR:-.}/cfg.json"; [ -f /tmp/cfg.json ] && CFG=/tmp/cfg.json
[ "$(j "len(d['token'].split('.'))" < $CFG)" = 3 ] || fail "token not 3-part"
curl -s $HRM/files/sample.docx -o "$CFG.docx"
[ "$(sha "$CFG.docx")" = "$(sha $FIX)" ] || fail "hrm file sha mismatch"

before=$(curl -s $HRM/received | wc -l)
SESS=$(python -I -c "import json,sys; print(json.dumps({'config': json.load(open(sys.argv[1]))}))" $CFG | curl -s -w '\n%{http_code}' -H 'content-type: application/json' --data-binary @- $DS/api/session)
code=$(echo "$SESS" | tail -1); body=$(echo "$SESS" | head -1)
[ "$code" = 200 ] || fail "session $code $body"
SID=$(echo "$body" | j "d['sid']")
curl -s $DS/api/session/$SID/document -o "$CFG.out"
[ "$(sha "$CFG.out")" = "$(sha $FIX)" ] || fail "document sha mismatch"
RES=$(curl -s -H 'X-File-Name: sample.docx' --data-binary @$FIX $DS/api/session/$SID/save)
echo "save -> $RES"
[ "$(echo "$RES" | j "d['callback']")" = ok ] || fail "callback not ok"
after=$(curl -s $HRM/received | wc -l)
[ "$after" -eq $((before + 1)) ] || fail "received.jsonl did not grow"
LINE=$(curl -s $HRM/received | tail -1)
echo "received -> $LINE"
[ "$(echo "$LINE" | j "d['status']")" = 6 ] || fail "status != 6"
[ "$(echo "$LINE" | j "d['sha256']")" = "$(sha $FIX)" ] || fail "received sha mismatch"
# tampered file token -> 401
code=$(curl -s -o /dev/null -w '%{http_code}' "$DS/files/$SID/1?t=abc.def.ghi")
[ "$code" = 401 ] || fail "tampered t gave $code"
echo "SMOKE OK"
