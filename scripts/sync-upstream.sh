#!/usr/bin/env bash
# Upstream sync (PLAN section 5.1). Moves the BASE of one or more upstream repos and rebases the patch queue.
#
#   scripts/sync-upstream.sh [--dry-run|--apply] [--survey] [--resolve [--resolver <name>]] [--to <sha>]
#                            [--gates quick|full] <repo...|all>
#
#   --dry-run   (default) steps 0-5 in scratch worktrees; the monorepo working tree, index and HEAD stay untouched.
#   --apply     the same, then (only if every selected gate passes and no patch is unresolved) commit the report
#               and fast-forward the monorepo to the scratch branch sync/<date>-<time>.
#   --resolve   on a conflict, stop and wait for a resolver (agent or human) working in the printed worktree:
#               edit, `git add`, and the script runs `git am --continue` itself and records the minutes.
#   --survey    dry-run only: do not stop at the first conflict; commit the conflict markers and carry on, so the
#               report lists every conflicting patch with file:line. Patches after the first conflict may cascade.
#   --fmt       regenerate the trailing `NNNN-<repo>-rustfmt.patch`: after the rebase, `rustfmt --edition <ed>` over the .rs
#               files the queue touches, committed as ONE last patch (mechanical formatting, regenerate on every sync).
#               That patch is never rebased: a queue that already has one regenerates it even without --fmt.
#   --to <sha>  new BASE (only with one repo). Default: the newest tag newer than BASE, else origin/HEAD.
#   --gates     quick = verify-queue + gen-check + lockdiff + deny + cargo check per domain (native + wasm) plus `cargo check --workspace --all-targets` of the upstream tree (tests, examples, benches compile);
#               full  = quick + `scripts/ci.sh build <affected domains>` and `scripts/ci.sh test <affected apps>` run on the scratch tree (own target root $OFFICE_TARGET/sync/full).
#   repos       wordcraft gridcraft deckcraft pdfcraft craft-libs | all. craft-fonts is pinned by patches/craft-fonts/BASE
#               and moved by hand (never patched).
#
# Outputs: $OFFICE_TARGET/sync/reports/<date>.md (+ <date>-rebase-log.tsv), cache branch office/queue-<new7> in each
# cache clone (the rebased queue). Exit: 0 success, 1 conflict or gate failure, 2 preflight/infrastructure error.
set -uo pipefail
. "$(dirname "${BASH_SOURCE[0]}")/env.sh"
. "$OFFICE_ROOT/scripts/lib/lock.sh"
. "$OFFICE_ROOT/scripts/lib/import.sh"
MAIN="$OFFICE_ROOT"
cd "$MAIN" || exit 2

ALL="wordcraft gridcraft deckcraft pdfcraft craft-libs"
MODE=dry GATES=quick RESOLVE=0 SURVEY=0 FMT=0 TO="" RESOLVER="-" REPOS=""
while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run) MODE=dry;; --apply) MODE=apply;; --resolve) RESOLVE=1;; --survey) SURVEY=1;; --fmt) FMT=1;;
    --resolver) RESOLVER="$2"; shift;; --to) TO="$2"; shift;; --gates) GATES="$2"; shift;;
    -h|--help) sed -n 2,24p "$0"; exit 0;;
    all) REPOS="$ALL";; -*) echo "unknown option $1" >&2; exit 2;;
    *) REPOS="$REPOS $1";;
  esac; shift
done
REPOS="${REPOS# }"; [ -n "$REPOS" ] || { echo "usage: sync-upstream.sh [--dry-run|--apply] [--resolve] [--to <sha>] [--gates quick|full] <repo...|all>" >&2; exit 2; }
for r in $REPOS; do case " $ALL " in *" $r "*) ;; *) echo "unknown repo $r" >&2; exit 2;; esac; done
[ -z "$TO" ] || [ "$(echo $REPOS | wc -w)" = 1 ] || { echo "--to needs exactly one repo" >&2; exit 2; }
[ "$MODE" = apply ] && [ "$SURVEY" = 1 ] && { echo "--survey is for --dry-run" >&2; exit 2; }
case "$GATES" in quick|full) ;; *) echo "--gates quick|full" >&2; exit 2;; esac

DATE=$(date +%F); RUN=$(date +%H%M%S)
SYNC="$OFFICE_TARGET/sync"; REPDIR="$SYNC/reports"; WORK="$SYNC/run-$DATE-$RUN"; mkdir -p "$REPDIR" "$WORK"
REPORT="$REPDIR/$DATE.md"; TSV="$REPDIR/$DATE-rebase-log.tsv"; : > "$REPORT"; : > "$TSV"
SCR="$SYNC/mono-$DATE-$RUN"; BRANCH="sync/$DATE-$RUN"
export GIT_COMMITTER_NAME="office-sync" GIT_COMMITTER_EMAIL="sync@office.invalid"
domain_of() { case "$1" in wordcraft) echo docs;; gridcraft) echo sheets;; deckcraft) echo slides;; pdfcraft) echo pdf;; *) echo "";; esac; }
say() { echo "$*" | tee -a "$REPORT"; }
secs() { date +%s.%N; }
elapsed() { awk -v a="$1" -v b="$(secs)" 'BEGIN{printf "%.2f", b-a}'; }
baseval() { sed -n "s/^$2=//p" "$MAIN/patches/$1/BASE"; }
t_start=$(secs)

# ---------------------------------------------------------------- step 0 preflight
# What a dry run must not change: the content of the paths it owns (HEAD may move when another agent commits elsewhere).
guard() { { git rev-parse HEAD:upstream HEAD:patches HEAD:domains HEAD:scripts; git status --porcelain -- upstream patches domains scripts docs/phase1/sync-reports; } | sha1sum | cut -c1-40; }
HEAD0=$(git rev-parse HEAD); GUARD0=$(guard)
if [ -n "$(git status --porcelain -- upstream patches domains scripts docs/phase1/sync-reports)" ]; then
  echo "preflight: upstream/ patches/ domains/ scripts/ are not clean; commit or stash first" >&2; exit 2; fi
OTHER_DIRTY=$(git status --porcelain | wc -l)
for t in git cargo sha1sum; do command -v $t >/dev/null || { echo "preflight: $t missing" >&2; exit 2; }; done
for r in $REPOS; do [ -d "$UPSTREAM_CACHE/$r/.git" ] || [ -f "$UPSTREAM_CACHE/$r/.git" ] || { echo "preflight: no cache clone $UPSTREAM_CACHE/$r" >&2; exit 2; }; done

say "# Upstream sync report $DATE ($MODE, gates $GATES, survey=$SURVEY resolve=$RESOLVE)"
say ""
say "Monorepo HEAD \`${HEAD0:0:7}\`; repos: $REPOS; paths outside upstream/patches/domains/scripts with uncommitted changes (not touched): $OTHER_DIRTY"
say ""

# ---------------------------------------------------------------- step 1 fetch
say "## 1 Fetch and distance"
say ""
say "| repo | BASE | NEW | upstream commits | NEW date | queue patches | files touched by queue | churn on those files (BASE..NEW) |"
say "|---|---|---|---|---|---|---|---|"
declare -A NEW OLD NCOMMITS
for r in $REPOS; do
  C="$UPSTREAM_CACHE/$r"
  git -C "$C" fetch --prune --tags -q origin 2>&1 | tail -2
  OLD[$r]=$(baseval $r sha)
  if [ -n "$TO" ]; then NEW[$r]=$(git -C "$C" rev-parse "$TO^{commit}")
  else
    # newest tag that contains BASE (so is newer than it); none -> origin/HEAD
    tag=$(git -C "$C" tag --contains "${OLD[$r]}" --sort=-creatordate | head -1)
    if [ -n "$tag" ] && [ "$(git -C "$C" rev-parse "$tag^{commit}")" != "${OLD[$r]}" ]; then NEW[$r]=$(git -C "$C" rev-parse "$tag^{commit}")
    else NEW[$r]=$(git -C "$C" rev-parse origin/HEAD); fi
  fi
  NCOMMITS[$r]=$(git -C "$C" rev-list --count "${OLD[$r]}..${NEW[$r]}")
  np=$(ls "$MAIN"/patches/$r/[0-9][0-9][0-9][0-9]-*.patch 2>/dev/null | wc -l)
  qfiles=$(cat "$MAIN"/patches/$r/[0-9][0-9][0-9][0-9]-*.patch 2>/dev/null | sed -n 's/^diff --git a\/\(.*\) b\/.*/\1/p' | sort -u)
  nq=$(echo "$qfiles" | grep -c . )
  if [ "$np" -gt 0 ]; then churn=$(git -C "$C" diff --shortstat "${OLD[$r]}" "${NEW[$r]}" -- $qfiles | sed 's/^ *//'); else churn="-"; fi
  [ -n "$churn" ] || churn="no change"
  echo "$qfiles" > "$WORK/$r.qfiles"
  say "| $r | \`${OLD[$r]:0:7}\` | \`${NEW[$r]:0:7}\` | ${NCOMMITS[$r]} | $(git -C "$C" log -1 --format=%cs "${NEW[$r]}") | $np | $([ "$np" -gt 0 ] && echo $nq || echo 0) | $churn |"
done
fonts_new=$(git -C "$UPSTREAM_CACHE/craft-fonts" rev-parse origin/HEAD 2>/dev/null); fonts_pin=$(baseval craft-fonts sha)
say ""; say "craft-fonts: pinned \`${fonts_pin:0:7}\`, origin/HEAD \`${fonts_new:0:7}\` ($([ "$fonts_new" = "$fonts_pin" ] && echo unchanged || echo MOVED, not synced by this script))"
say ""

# ---------------------------------------------------------------- step 2 signals
say "## 2 Signals (files that change our assumptions)"
say ""
lockver() { git -C "$UPSTREAM_CACHE/$1" show "$2:Cargo.lock" 2>/dev/null | awk -v p="$3" '$0=="name = \""p"\""{getline; print $3; exit}' | tr -d '"'; }
rustver() { git -C "$UPSTREAM_CACHE/$1" show "$2:Cargo.toml" 2>/dev/null | sed -n 's/^rust-version *= *"\(.*\)"/\1/p' | head -1; }
for r in $REPOS; do
  C="$UPSTREAM_CACHE/$r"; o=${OLD[$r]}; n=${NEW[$r]}; flags=""
  rv0=$(rustver $r $o); rv1=$(rustver $r $n)
  [ "$rv0" != "$rv1" ] && flags="$flags rust-version $rv0 -> $rv1 (ours: $(rustc --version | awk '{print $2}'));"
  for pk in wasm-bindgen egui; do a=$(lockver $r $o $pk); b=$(lockver $r $n $pk); [ "$a" != "$b" ] && flags="$flags $pk $a -> $b;"; done
  git -C "$C" diff -U0 $o $n -- Cargo.toml | grep -qE '^[+-]\[(patch|profile|workspace\.lints)|^@@.*\[(patch|profile|workspace\.lints)' && flags="$flags [patch]/[profile]/[workspace.lints] changed;"
  newapps=$(git -C "$C" diff --name-status $o $n -- 'apps/*/Cargo.toml' 'crates/*/Cargo.toml' | awk '$1=="A"{print $2}' | tr '\n' ' ')
  [ -n "$newapps" ] && flags="$flags new manifests: $newapps"
  sigstat=$(git -C "$C" diff --stat $o $n -- docs/decisions.md docs/gaps.md docs/roadmap.md ':(glob)**/*parity*.md' CLAUDE.md AGENTS.md Cargo.toml .cargo/config.toml 'rust-toolchain*' | tail -1 | sed 's/^ *//')
  say "- **$r**: ${flags:-no flagged change}. Signal files: ${sigstat:-unchanged}"
done
say ""

# ---------------------------------------------------------------- step 3 rebase
say "## 3 Rebase of the patch queues"
say ""
say "Cost per patch (PLAN 5.4): wall seconds, conflicted files with the first marker line, conflict hunks, interdiff lines (\`git range-diff\`, hunk and index headers excluded)."
say ""
declare -A UNRES REPO_SECS REPO_NP RESULT_CNT
UNRES_ANY=0
rm_worktree() { git -C "$1" worktree remove --force "$2" >/dev/null 2>&1; rm -rf "$2"; git -C "$1" worktree prune; }
for r in $REPOS; do
  C="$UPSTREAM_CACHE/$r"; o=${OLD[$r]}; n=${NEW[$r]}; n7=${n:0:7}
  WN="$SYNC/wt-$r-$n7"; WO="$SYNC/wt-$r-old${o:0:7}"; rm_worktree "$C" "$WN"; rm_worktree "$C" "$WO"
  patches=$(ls "$MAIN"/patches/$r/[0-9][0-9][0-9][0-9]-*.patch 2>/dev/null | grep -v -- '-rustfmt[.]patch$'); np=$(echo "$patches" | grep -c .)
  oldfmt=$(ls "$MAIN"/patches/$r/[0-9][0-9][0-9][0-9]-*-rustfmt.patch 2>/dev/null | head -1)   # the trailing fmt patch is regenerated, never rebased
  [ -n "$patches" ] || np=0
  REPO_NP[$r]=$np; UNRES[$r]=0
  git -C "$C" worktree add -q --detach "$WN" "$n" || { say "FATAL: worktree for $r failed"; exit 2; }
  git -C "$WN" config core.autocrlf false
  if [ "$np" -eq 0 ]; then say "- **$r**: no patches; snapshot moves \`${o:0:7}\` -> \`$n7\` only."; continue; fi
  git -C "$C" worktree add -q --detach "$WO" "$o"; git -C "$WO" config core.autocrlf false
  for p in $patches; do git -C "$WO" am -q "$p" >/dev/null 2>&1 || { say "FATAL: old queue does not apply to BASE ($p)"; exit 2; }; done
  oldq=$(git -C "$WO" rev-parse HEAD)
  : > "$WORK/$r.rows"; r_t0=$(secs); stop=0
  for p in $patches; do
    pn=$(basename "$p"); num=${pn:0:4}; t0=$(secs)
    if [ $stop = 1 ]; then echo -e "$pn\tnot-run\t-\t0\t0.00" >> "$WORK/$r.rows"; continue; fi
    out=$(git -C "$WN" am -3 "$p" 2>&1); rc=$?
    if [ $rc -eq 0 ]; then
      res=clean; echo "$out" | grep -q "Falling back" && res=3way
      echo -e "$pn\t$res\t-\t0\t$(elapsed $t0)" >> "$WORK/$r.rows"; continue
    fi
    unm=$(git -C "$WN" diff --name-only --diff-filter=U)
    locs=""; hunks=0
    for f in $unm; do
      first=$(grep -n '^<<<<<<< ' "$WN/$f" | head -1 | cut -d: -f1); cnt=$(grep -c '^<<<<<<< ' "$WN/$f")
      locs="$locs${locs:+,}$f:${first:-?}"; hunks=$((hunks+cnt))
    done
    say "CONFLICT at $num in $r: ${locs:-patch does not apply (no unmerged file)}"
    if [ $RESOLVE = 1 ]; then
      echo ">>> RESOLVE NEEDED: worktree $WN  patch $pn  files: $unm"
      echo ">>> edit, 'git -C $WN add <files>'; the script then runs 'git am --continue' (no conflict markers may remain)"
      while :; do
        [ -z "$(git -C "$WN" ls-files -u)" ] && ! git -C "$WN" diff --cached | grep -q '^+<<<<<<< ' && break
        sleep 15
      done
      git -C "$WN" -c core.editor=true am --continue >/dev/null 2>&1 || git -C "$WN" am --skip >/dev/null 2>&1
      echo -e "$pn\tresolved\t$locs\t$hunks\t$(elapsed $t0)" >> "$WORK/$r.rows"
    elif [ $SURVEY = 1 ]; then
      if [ -n "$unm" ]; then git -C "$WN" add -A >/dev/null 2>&1; git -C "$WN" -c core.editor=true am --continue >/dev/null 2>&1
      else git -C "$WN" am --skip >/dev/null 2>&1; fi
      echo -e "$pn\tconflict\t${locs:-no-merge}\t$hunks\t$(elapsed $t0)" >> "$WORK/$r.rows"; UNRES[$r]=$((${UNRES[$r]}+1))
    else
      git -C "$WN" am --abort >/dev/null 2>&1
      echo -e "$pn\tconflict\t${locs:-no-merge}\t$hunks\t$(elapsed $t0)" >> "$WORK/$r.rows"; UNRES[$r]=$((${UNRES[$r]}+1)); stop=1
    fi
  done
  REPO_SECS[$r]=$(elapsed $r_t0)
  [ "${UNRES[$r]}" -gt 0 ] && UNRES_ANY=1
  # interdiff lines per patch
  git -C "$C" range-diff --no-color "$o..$oldq" "$n..$(git -C "$WN" rev-parse HEAD)" 2>/dev/null | awk '
    /^ *[0-9]+: +[0-9a-f]+ [=!<>] +[0-9-]+: / { n=$1; sub(":","",n); cur=n+0; cnt[cur]=0; next }
    cur && /^    [+-]/ { s=substr($0,6); if (s !~ /^(@@|index |diff --git|--- |\+\+\+ |new file|deleted file|Metadata)/) cnt[cur]++ }
    END { for (k in cnt) print k "\t" cnt[k] }' | sort -n > "$WORK/$r.interdiff"
  # keep the queue rustfmt-clean (CI lint) with ONE trailing patch: rustfmt on every .rs file the queue touches.
  # rustfmt output depends on the new upstream context, so the patch is regenerated on every sync, never rebased.
  fmtname=""; fmtdone=0
  if { [ $FMT = 1 ] || [ -n "$oldfmt" ]; } && [ "${UNRES[$r]}" -eq 0 ]; then
    ed=$(awk -F'"' '/^edition *=/{print $2; exit}' "$WN/Cargo.toml"); ed=${ed:-2021}
    fmtname=$(basename "${oldfmt:-}"); [ -n "$oldfmt" ] || fmtname=$(printf '%04d-%s-rustfmt.patch' $(( 10#$(ls "$MAIN"/patches/$r/[0-9][0-9][0-9][0-9]-*.patch 2>/dev/null | xargs -r -n1 basename | tail -1 | cut -c1-4 | grep . || echo 0) + 1 )) "$r")
    qfiles=$(git -C "$WN" diff --name-only "$n..HEAD" | grep '[.]rs$' | while read -r f; do [ -f "$WN/$f" ] && echo "$f"; done)
    if [ -n "$qfiles" ] && ( cd "$WN" && rustfmt --edition $ed $qfiles ) >"$WORK/$r.fmt.log" 2>&1; then
      if ! git -C "$WN" diff --quiet; then
        git -C "$WN" commit -q -a -F - <<EOM
$r: rustfmt the files the patch queue touches (mechanical formatting, regenerate on every sync)

rustfmt --edition $ed over every .rs file touched by the queue, so cargo fmt --all --check is clean in CI lint. No behaviour change.
Always the last patch of the queue: scripts/sync-upstream.sh regenerates it (--fmt, or implied when the queue has one), never rebases it.
EOM
        fmtdone=1
      fi
    else
      say "WARN: rustfmt failed for $r (see $WORK/$r.fmt.log); queue kept unformatted"
    fi
    say "- $r: rustfmt (edition $ed) trailing patch ${fmtname}: $([ $fmtdone = 1 ] && echo "regenerated, $(git -C "$WN" show --stat --format= HEAD | tail -1 | sed 's/^ *//')" || echo "not needed, files are already clean${oldfmt:+ (the old patch is dropped: remove its REGISTER row)}")"
  fi
  # regenerate the queue
  rm -rf "$WORK/$r.patches"; mkdir -p "$WORK/$r.patches"
  nh=$(git -C "$WN" rev-parse HEAD)
  git -C "$WN" format-patch -q --zero-commit --binary --no-signature -o "$WORK/$r.patches" "$n..$nh"
  # keep the registered file names (format-patch numbers from 0001): patch k takes the name of old patch k; the fmt patch keeps its own
  mapfile -t oldnames < <(echo "$patches" | xargs -r -n1 basename)
  mapfile -t newfiles < <(ls "$WORK/$r.patches"/[0-9][0-9][0-9][0-9]-*.patch)
  for ((k=0; k<${#newfiles[@]}; k++)); do
    if [ $k -lt $np ]; then tgt=${oldnames[$k]}; else tgt=$fmtname; fi
    [ "$(basename "${newfiles[$k]}")" = "$tgt" ] || mv "${newfiles[$k]}" "$WORK/$r.patches/.tmp-$k"
  done
  for ((k=0; k<${#newfiles[@]}; k++)); do
    [ -e "$WORK/$r.patches/.tmp-$k" ] || continue
    if [ $k -lt $np ]; then tgt=${oldnames[$k]}; else tgt=$fmtname; fi
    mv "$WORK/$r.patches/.tmp-$k" "$WORK/$r.patches/$tgt"
  done
  [ $fmtdone = 1 ] && echo -e "$fmtname	regenerated	-	0	0.00" >> "$WORK/$r.rows" && REPO_NP[$r]=$((np+1))
  git -C "$WN" branch -f "office/queue-$n7" "$nh" 2>/dev/null || git -C "$C" branch -f "office/queue-$n7" "$nh"
  echo "$nh" > "$WORK/$r.newq"
  say ""; say "**$r**: \`${o:0:7}\` -> \`$n7\`, ${NCOMMITS[$r]} upstream commits, cache branch \`office/queue-$n7\`, total ${REPO_SECS[$r]} s"
  say ""; say "| patch | result | conflicted files (first marker line) | hunks | seconds | interdiff lines |"; say "|---|---|---|---|---|---|"
  i=0; while IFS=$'\t' read -r pn res locs hunks sec; do
    i=$((i+1)); il=$(awk -F'\t' -v k=$i '$1==k{print $2}' "$WORK/$r.interdiff"); [ -n "$il" ] || il=0
    say "| ${pn:0:4} | $res | ${locs//,/<br>} | $hunks | $sec | $il |"
    min=$(awk -v s="$sec" 'BEGIN{printf "%.2f", s/60}')
    echo -e "$DATE\t$r\t${o:0:7}\t$n7\t${NCOMMITS[$r]}\t${pn:0:4}\t$res\t$locs\t$hunks\t$min\t$il\t-\t-\t$([ "$res" = resolved ] && echo "$RESOLVER" || echo script)" >> "$TSV"
    RESULT_CNT[$res]=$(( ${RESULT_CNT[$res]:-0} + 1 ))
  done < "$WORK/$r.rows"
  say ""
done
say "Totals: clean ${RESULT_CNT[clean]:-0}, 3-way ${RESULT_CNT[3way]:-0}, resolved ${RESULT_CNT[resolved]:-0}, conflict ${RESULT_CNT[conflict]:-0}, not-run ${RESULT_CNT[not-run]:-0}"
say ""

# ---------------------------------------------------------------- step 4 materialise in scratch
say "## 4 Materialise in scratch (\`$SCR\`, branch \`$BRANCH\`)"
say ""
READY=""
for r in $REPOS; do [ "${UNRES[$r]:-0}" -eq 0 ] && READY="$READY $r"; done; READY="${READY# }"
GATE_FAIL=0; declare -A GATE
if [ -z "$READY" ]; then say "No repo is free of unresolved conflicts; nothing to materialise, gates skipped."; GATE_FAIL=1
else
  git branch -D "$BRANCH" >/dev/null 2>&1; git worktree remove --force "$SCR" >/dev/null 2>&1; rm -rf "$SCR"
  git worktree add -q -b "$BRANCH" "$SCR" "$HEAD0" || { say "FATAL: scratch worktree"; exit 2; }
  cd "$SCR" || exit 2
  . "$SCR/scripts/lib/import.sh"
  export CARGO_TARGET_DIR="$SYNC/target-xt"
  for r in $READY; do
    n=${NEW[$r]}; n7=${n:0:7}; np=${REPO_NP[$r]}
    import_pristine $r $n || { say "FATAL: import_pristine $r"; exit 2; }
    if [ "$np" -eq 0 ]; then
      write_base $r $n $DATE; git add patches/$r/BASE
      git commit -q -m "vendor($r): pristine $n7" -- upstream/$r patches/$r/BASE
    else
      git commit -q -m "vendor($r): pristine $n7" -- upstream/$r
      import_pristine $r "$(cat "$WORK/$r.newq")" || { say "FATAL: import queue $r"; exit 2; }
      git rm -q -f patches/$r/[0-9][0-9][0-9][0-9]-*.patch; cp "$WORK/$r.patches"/*.patch patches/$r/
      write_base $r $n $DATE; git add patches/$r
      git commit -q -m "patches($r): apply queue ($np patches) on $n7" -- upstream/$r patches/$r
    fi
    say "- $r: commits P' (pristine $n7)$([ "$np" -gt 0 ] && echo " and Q' (queue of $np)"), BASE tree $(baseval $r tree | cut -c1-12)"
  done
  # lock / domain regeneration
  for r in $READY; do d=$(domain_of $r); [ -n "$d" ] || continue
    ( cargo xt gen-domain $d ) >"$WORK/gen-$d.log" 2>&1 || { say "FATAL: gen-domain $d (see $WORK/gen-$d.log)"; GATE[gen-$d]=FAIL; GATE_FAIL=1; continue; }
    cp "upstream/$r/Cargo.lock" "domains/$d/Cargo.lock"
    cargo metadata --format-version 1 --manifest-path "domains/$d/Cargo.toml" >/dev/null 2>"$WORK/seed-$d.log" || say "WARN: lock seeding for $d failed, see $WORK/seed-$d.log"
  done
  chg=$(git status --porcelain -- domains | wc -l)
  say "- domain regeneration: $chg changed files under domains/ ($(git status --porcelain -- domains | awk '{print $2}' | sed 's#/[^/]*$##' | sort -u | tr '\n' ' '))"
  say ""
  # register + rebase log
  for r in $READY; do n7=${NEW[$r]:0:7}
    awk -v repo=$r -v d="$DATE" -v n7=$n7 'FNR==NR{ split($0,t,"\t"); if(t[2]==repo){ res[repo "/" t[6]]=t[7]; mn[repo "/" t[6]]=t[10] } next }
      /^\| / { n=split($0,c,"|"); id=c[2]; gsub(/ /,"",id)
               if (id in res) { c[11]=" " d " @ " n7 ": " res[id] ", " mn[id] " min "; s=c[1]; for(i=2;i<=n;i++) s=s "|" c[i]; print s; next } }
      {print}' "$TSV" patches/REGISTER.md > "$WORK/REGISTER.new" && cp "$WORK/REGISTER.new" patches/REGISTER.md
  done
  git diff --stat -- patches/REGISTER.md | tail -1 | sed 's/^/- register: /' | tee -a "$REPORT"
  [ -s patches/rebase-log.tsv ] || printf 'date\trepo\tfrom_sha\tto_sha\tupstream_commits\tpatch\tresult\tconflicted_files\tconflict_hunks\tminutes\tresolution_lines\trebuild_s\tretest\tresolver\n' > patches/rebase-log.tsv
  for r in $READY; do awk -F'\t' -v repo=$r '$2==repo' "$TSV" >> patches/rebase-log.tsv; done

  # ------------------------------------------------------------ step 5 gates
  say ""; say "## 5 Gates ($GATES)"; say ""
  gate() { # <name> <PASS|FAIL|INFRA> <detail>
    GATE[$1]=$2; say "- $1: **$2** $3"; [ "$2" = PASS ] || GATE_FAIL=1; }
  wait_no_rustc() { while tasklist //FI "IMAGENAME eq rustc.exe" 2>/dev/null | grep -qi rustc.exe; do echo "another rustc.exe is running; waiting"; sleep 20; done; }

  out=$(scripts/verify-queue.sh $READY 2>&1); rc=$?
  gate verify-queue $([ $rc = 0 ] && echo PASS || echo FAIL) "$(echo "$out" | tr '\n' ';')"
  out=$(cargo xt register check 2>&1); rc=$?; gate register $([ $rc = 0 ] && echo PASS || echo FAIL) "$out"
  # i18n gate: the canonical catalogs (i18n/*.tsv) must be current with the upstream catalogs just imported. A dry run fails
  # when an upstream catalog changed (keys added, removed, English or ru changed); --apply merges (cargo xt i18n extract keeps
  # every uz/ru value and status; a changed English is carried as draft:carried, never reviewed) and then re-checks.
  out=$(cargo xt i18n check-stale 2>&1); rc=$?; merged=""
  if [ $rc != 0 ] && [ "$MODE" = apply ]; then
    say "- i18n catalogs are stale: merging with \`cargo xt i18n extract\`:"; say '```'; say "$(echo "$out" | grep -v '^run ')"; say '```'
    if cargo xt i18n extract >"$WORK/i18n-extract.log" 2>&1; then out=$(cargo xt i18n check-stale 2>&1); rc=$?; merged="merged by --apply, "; fi
  fi
  gate i18n-stale $([ $rc = 0 ] && echo PASS || echo FAIL) "${merged}$(echo "$out" | head -1)"
  [ $rc = 0 ] || { say '```'; say "$out"; say '```'; say "  (a dry run does not merge: \`scripts/sync-upstream.sh --apply\` runs the merge, or run \`cargo xt i18n extract\` yourself)"; }
  out=$(cargo xt i18n roundtrip 2>&1); rc=$?; gate i18n-roundtrip $([ $rc = 0 ] && echo PASS || echo FAIL) "$(echo "$out" | tail -1)"
  out=$(cargo xt i18n lint 2>&1); rc=$?; gate i18n-lint $([ $rc = 0 ] && echo PASS || echo FAIL) "$(echo "$out" | head -1)"
  for r in $READY; do d=$(domain_of $r); [ -n "$d" ] || continue
    out=$(cargo xt gen-domain --check $d 2>&1); rc=$?; gate gen-check-$d $([ $rc = 0 ] && echo PASS || echo FAIL) "$out"
    out=$(cargo xt lockdiff domains/$d/Cargo.lock upstream/$r/Cargo.lock 2>&1); rc=$?; gate lockdiff-$d $([ $rc = 0 ] && echo PASS || echo FAIL) "$(echo "$out" | tr '\n' ';')"
    mkdir -p "$OFFICE_CACHE/advisory-db"
    out=$(cargo deny --manifest-path domains/$d/Cargo.toml --config ci/deny/$d.toml check --hide-inclusion-graph -A unmatched-skip -A license-not-encountered -A advisory-not-detected 2>&1); rc=$?
    ne=$(echo "$out" | grep -c '^error\[')
    if [ $rc = 0 ]; then gate deny-$d PASS ""; elif [ "$ne" -gt 0 ]; then gate deny-$d FAIL "$ne error(s): $(echo "$out" | grep '^error\[' | sort | uniq -c | head -4 | tr '\n' ';')"
    else gate deny-$d INFRA "exit $rc: $(echo "$out" | tail -2 | tr '\n' ' ')"; fi
  done
  check_domain() { # <domain>: native + wasm cargo check, separate target dir
    local d=$1 t0 rc1 rc2; export CARGO_TARGET_DIR="$SYNC/target-$d"
    [ "$d" = slides ] && export CRAFT_FONTS_DIR="$MAIN/upstream/craft-fonts" || unset CRAFT_FONTS_DIR
    wait_no_rustc; ram_gate 3; t0=$(secs)
    cargo check --locked -j 4 --manifest-path domains/$d/Cargo.toml -p $d-desktop -p $d-cli >"$WORK/check-$d-native.log" 2>&1; rc1=$?
    # tests too: a new upstream test can stop compiling against our patches (gridcraft 0103, E0615), which a plain check misses
    cargo check --locked -j 4 --workspace --all-targets --manifest-path upstream/$r/Cargo.toml >>"$WORK/check-$d-native.log" 2>&1 || rc1=1
    wait_no_rustc; ram_gate 3
    ( unset RUSTC_WRAPPER; cargo check --locked -j 4 --manifest-path domains/$d/Cargo.toml -p $d-web --target wasm32-unknown-unknown ) >"$WORK/check-$d-wasm.log" 2>&1; rc2=$?
    echo "$rc1 $rc2 $(elapsed $t0)" > "$WORK/check-$d.rc"
  }
  for r in $READY; do d=$(domain_of $r); [ -n "$d" ] || continue
    with_build_lock check_domain $d; read -r rc1 rc2 sec < "$WORK/check-$d.rc"
    gate check-$d $([ "$rc1$rc2" = 00 ] && echo PASS || echo FAIL) "native rc=$rc1, wasm32 rc=$rc2, ${sec} s (logs $WORK/check-$d-*.log)"
    echo "$r check_s=$sec" >> "$WORK/gate-secs"
  done
  if [ "$GATES" = full ]; then
    # Reuse the CI jobs on the scratch tree (env.sh makes the scratch checkout the root). Own target root: the main tree's
    # dists and warm caches stay untouched; it is persistent, so only the first full run is cold.
    fdoms=""; fapps=""; for r in $READY; do d=$(domain_of $r); [ -n "$d" ] && { fdoms="$fdoms $d"; fapps="$fapps $r"; }; done
    for x in $fdoms $fapps; do
      case " $fdoms " in *" $x "*) job=build;; *) job=test;; esac
      out=$(env -u CARGO_TARGET_DIR OFFICE_TARGET="$SYNC/full" CI_RUN="sync-$DATE-$RUN" scripts/ci.sh $job $x 2>&1); rc=$?
      echo "$out" > "$WORK/full-$job-$x.log"
      gate full-$job-$x $([ $rc = 0 ] && echo PASS || echo FAIL) "rc=$rc ($(echo "$out" | grep '^summary:' | head -1); log $WORK/full-$job-$x.log)"
    done
  fi
  export CARGO_TARGET_DIR="$SYNC/target-xt"
fi

# ---------------------------------------------------------------- step 6 apply / hold
cd "$MAIN" || exit 2
say ""; say "## 6 Outcome"; say ""
NPATCH=$(awk -F'\t' '{n++} END{print n+0}' "$TSV")
SUMMARY="$(echo $REPOS | wc -w) repos, $NPATCH patches (clean ${RESULT_CNT[clean]:-0}, 3-way ${RESULT_CNT[3way]:-0}, resolved ${RESULT_CNT[resolved]:-0}), gates $GATES"
if [ $UNRES_ANY = 0 ] && [ $GATE_FAIL = 0 ]; then OK=1; say "**DRY RUN OK: $SUMMARY PASS**"; else OK=0
  say "**DRY RUN INCOMPLETE: $SUMMARY; unresolved conflicts: $(for r in $REPOS; do echo -n "$r=${UNRES[$r]:-0} "; done); gate failures: $GATE_FAIL**"; fi
total=$(elapsed $t_start); say ""; say "Script wall time: ${total} s"
if [ "$MODE" = apply ]; then
  if [ $OK = 1 ]; then
    mkdir -p "$SCR/docs/phase1/sync-reports"; cp "$REPORT" "$SCR/docs/phase1/sync-reports/$DATE.md"
    ( cd "$SCR" && git add docs/phase1/sync-reports patches domains i18n && git commit -q -m "sync($(echo $READY | tr ' ' ',')): rebase queues, regenerate domains and locks ($DATE)" )
    # HEAD may have moved (other commits elsewhere): replay the scratch commits on it, then fast-forward
    if [ "$(git rev-parse HEAD)" != "$HEAD0" ]; then ( cd "$SCR" && git rebase -q "$(git -C "$MAIN" rev-parse HEAD)" ) >/dev/null 2>&1 || ( cd "$SCR" && git rebase --abort ) >/dev/null 2>&1; fi
    if git merge --ff-only -q "$BRANCH"; then say "APPLIED: fast-forwarded to $(git rev-parse --short HEAD)"; else say "APPLY FAILED: HEAD moved or merge refused; branch $BRANCH kept"; OK=0; fi
  else say "NOT APPLIED: the pin stays (STATUS gets an issue line); scratch branch $BRANCH kept at $SCR"; fi
fi
if [ "$MODE" = dry ] || [ $OK = 0 ]; then
  if [ "$MODE" = dry ]; then git worktree remove --force "$SCR" >/dev/null 2>&1; rm -rf "$SCR"; git worktree prune; git branch -D "$BRANCH" >/dev/null 2>&1; fi
fi
if [ "$MODE" = dry ] && [ "$(guard)" != "$GUARD0" ]; then echo "BUG: the dry run changed the monorepo" >&2; exit 1; fi
for r in $REPOS; do rm_worktree "$UPSTREAM_CACHE/$r" "$SYNC/wt-$r-old${OLD[$r]:0:7}" ; done
[ $OK = 1 ] && exit 0 || exit 1
