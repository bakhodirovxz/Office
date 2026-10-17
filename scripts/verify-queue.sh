#!/usr/bin/env bash
# Offline check (seconds) that upstream/<repo> == BASE tree + patch queue (PLAN section 5.2).
#   scripts/verify-queue.sh <repo|all>
# Rules: (a) the pristine commit's subtree equals BASE tree; (b) replaying patches/<repo>/NNNN-*.patch onto the BASE
# tree gives exactly HEAD:upstream/<repo>; (c) the working tree under upstream/<repo> is clean (no hand edits).
set -uo pipefail
. "$(dirname "$0")/env.sh"
cd "$OFFICE_ROOT"
ALL="wordcraft gridcraft deckcraft pdfcraft craft-libs"

fail() { echo "FAIL $r: $*" >&2; return 1; }

verify_one() {
  local r="$1" sha tree p n=0 idx pc re
  [ -f "patches/$r/BASE" ] || { fail "patches/$r/BASE missing"; return 1; }
  sha=$(sed -n 's/^sha=//p' "patches/$r/BASE"); tree=$(sed -n 's/^tree=//p' "patches/$r/BASE")
  git cat-file -e "$tree" 2>/dev/null || { fail "BASE tree $tree not in repo (pristine commit missing)"; return 1; }

  # (a) the last pristine commit for this repo and sha7 carries exactly the BASE tree
  re="(vendor\($r\): pristine ${sha:0:7}|vendor: pristine .*[ (]$r ${sha:0:7})"
  pc=$(git log -E --grep="$re" --format=%H -1 HEAD -- "upstream/$r")
  [ -n "$pc" ] || { fail "no 'pristine ${sha:0:7}' commit found for $r"; return 1; }
  [ "$(git rev-parse "$pc:upstream/$r")" = "$tree" ] || { fail "pristine commit $pc subtree != BASE tree"; return 1; }

  # (b) replay the queue in a throw-away index
  idx=$(mktemp -u)
  (
    export GIT_INDEX_FILE="$idx"
    git read-tree --prefix="upstream/$r/" "$tree" || exit 1
    for p in patches/$r/[0-9][0-9][0-9][0-9]-*.patch; do
      [ -e "$p" ] || continue
      git apply --cached --directory="upstream/$r" "$p" || { echo "FAIL $r: $p does not apply" >&2; exit 1; }
    done
    [ "$(git rev-parse "$(git write-tree):upstream/$r")" = "$(git rev-parse "HEAD:upstream/$r")" ] \
      || { echo "FAIL $r: tree != BASE+queue" >&2; exit 1; }
  ) || { rm -f "$idx"; return 1; }
  rm -f "$idx"
  for p in patches/$r/[0-9][0-9][0-9][0-9]-*.patch; do [ -e "$p" ] && n=$((n+1)); done

  # (c) no hand edits
  git diff --quiet HEAD -- "upstream/$r" && [ -z "$(git ls-files --others --exclude-standard "upstream/$r")" ] \
    || { fail "hand-edited (working tree differs from HEAD)"; return 1; }
  echo "$r: $n patches, tree OK"
}

target="${1:-all}"; rc=0
[ "$target" = all ] && set -- $ALL
for r in "$@"; do verify_one "$r" || rc=1; done
exit $rc
