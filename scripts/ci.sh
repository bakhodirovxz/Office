#!/usr/bin/env bash
# Local CI runner (PLAN section 6). The GitHub workflow calls exactly this script, one implementation for both.
#
#   scripts/ci.sh [--no-build] [--run <id>] <job> [args] [<job> [args] ...]
#   scripts/ci.sh [--no-build] quick|full|nightly
#
# Jobs (names as in PLAN 6.1):
#   queue                    verify-queue all; cargo xt register check; quarantine format check
#   lint                     fmt + clippy -D warnings (root), fmt/layers/assets (upstream trees), upstream clippy ratchet (fails on new warnings only)
#   gen-check                cargo xt gen-domain --check all; lockdiff x4
#   deny                     cargo-deny x5 (root + 4 domains), one config each
#   brand                    cargo xt brand-grep + logo-include check
#   i18n                     cargo xt i18n lint + roundtrip + check-stale
#   test <app|all>           upstream-workspace debug tests: nextest + quarantine, overlay ON; doctests
#   test-ours                root workspace tests + domain crates
#   check <domain|all>       cargo check desktop + wasm32 web
#   bind                     docs-bind (WP13, approach B binding layer): cargo check + test, native and wasm32
#   build <domain|all> <desktop|web|cli|all>   release builds via scripts/build.sh
#   sizes [--baseline] [--measure-base]        brotli/gzip of dist/<domain>-web vs web/budgets.toml
#   web-smoke                Playwright smoke over every dist
#   actionlint               lint .github/workflows/ci.yml
#   release-tests (nightly)  cargo test --release per app with the quarantine
#   fidelity, xlsx-bench (nightly), sync-dry-run (weekly)
# Domains: docs(wordcraft) sheets(gridcraft) slides(deckcraft) pdf(pdfcraft). Apps are the upstream names.
#
# Output: $OFFICE_TARGET/ci/<run>/<job>.json (+ <job>.log) and a markdown summary appended to
# $OFFICE_TARGET/ci/<run>/summary.md. Exit codes: 0 green, 1 a blocking failure, 2 an infrastructure error
# (missing tool or script, unreachable network). With several jobs: 1 if any job failed, else 2 if any infra error.
#
# --no-build (or CI_NO_BUILD=1): skip every step that compiles (clippy, tests, builds, cargo xt, ...) and list it as
# "skipped"; non-compiling steps (verify-queue, cargo deny, cargo fmt --check, actionlint, awk checks) still run.
# This is the dry-run mode used while another job owns the machine.
#
# Interfaces expected from WP3 (read, not defined here): domains/<d>/Cargo.toml (workspace) with packages <d>, <d>-web,
# <d>-cli; scripts/build.sh <d> desktop|web|cli --release; web dist in $OFFICE_TARGET/dist/<d>-web. Until domains/ exists
# the jobs fall back to the upstream workspace of the same app (a "stand-in", flagged in the output).
set -uo pipefail
. "$(dirname "${BASH_SOURCE[0]}")/env.sh"
. "$OFFICE_ROOT/scripts/lib/lock.sh"
cd "$OFFICE_ROOT" || exit 2

DOMAINS="docs sheets slides pdf"
APPS="wordcraft gridcraft deckcraft pdfcraft"
JOBS="queue lint gen-check deny brand i18n test test-ours check bind build sizes web-smoke actionlint release-tests fidelity xlsx-bench sync-dry-run"

NO_BUILD="${CI_NO_BUILD:-0}"
RUN_ID="${CI_RUN:-$(date +%Y%m%d-%H%M%S)}"

app_of() { case "$1" in docs) echo wordcraft;; sheets) echo gridcraft;; slides) echo deckcraft;; pdf) echo pdfcraft;; *) return 1;; esac; }
domain_of() { case "$1" in wordcraft) echo docs;; gridcraft) echo sheets;; deckcraft) echo slides;; pdfcraft) echo pdf;; *) return 1;; esac; }
# Workspace manifest of a domain: the generated domain workspace, else the upstream workspace as a stand-in.
domain_manifest() {
  if [ -f "domains/$1/Cargo.toml" ]; then echo "domains/$1/Cargo.toml"; else echo "upstream/$(app_of "$1")/Cargo.toml"; fi
}
domain_standin() { [ ! -f "domains/$1/Cargo.toml" ]; }
# Package names inside that workspace.
domain_pkg() { # <domain> <desktop|web|cli>
  local base; if domain_standin "$1"; then base="$(app_of "$1")"; else base="$1"; fi
  case "$2" in desktop) echo "$base";; web) echo "$base-web";; cli) echo "$base-cli";; esac
}
target_dir_domain() { if domain_standin "$1"; then echo "$OFFICE_TARGET/upstream-$(app_of "$1")"; else echo "$OFFICE_TARGET/domain-$1"; fi; }
overlay_dir() { echo "$OFFICE_OVERLAY_ROOT/$1"; }
xt_has() { grep -q "\"$1\"" xtask/src/main.rs 2>/dev/null; }
toml_get() { # <file> <section> <key>
  awk -v sec="[$2]" -v key="$3" '$0==sec{f=1;next} /^\[/{f=0} f && $1==key {sub(/^[^=]*=[ \t]*/,""); sub(/[ \t]*#.*$/,""); gsub(/"/,""); print; exit}' "$1"
}

# ---------- per-job bookkeeping ----------
JOB="" JOB_LOG="" STEPS="" SKIPPED=0 DETAIL=""
RUN_DIR="$OFFICE_TARGET/ci/$RUN_ID"

job_begin() { JOB="$1"; STEPS=""; SKIPPED=0; DETAIL=""; mkdir -p "$RUN_DIR"; JOB_LOG="$RUN_DIR/$1.log"; : >"$JOB_LOG"; echo "=== job $1 ${2:-}"; }
note() { echo "$*" | tee -a "$JOB_LOG"; DETAIL="${DETAIL:+$DETAIL; }$*"; }
# step <name> <cmd...>: run, log, record rc. Returns the command's rc.
step() {
  local name="$1" rc; shift
  echo "--- $name: $*" | tee -a "$JOB_LOG"
  local t0=$SECONDS
  "$@" 2>&1 | tee -a "$JOB_LOG"; rc=${PIPESTATUS[0]}
  echo "--- $name: exit $rc in $((SECONDS - t0))s" | tee -a "$JOB_LOG"
  STEPS="${STEPS:+$STEPS,}\"$name:$rc\""; return "$rc"
}
# bstep <name> <cmd...>: a step that compiles. Skipped under --no-build; otherwise under the build lock and RAM gate.
bstep() {
  local name="$1"
  if [ "$NO_BUILD" = 1 ]; then
    shift; echo "--- $name: SKIPPED (no-build): $*" | tee -a "$JOB_LOG"
    STEPS="${STEPS:+$STEPS,}\"$name:skipped\""; SKIPPED=$((SKIPPED+1)); return 0
  fi
  ram_gate "${CI_RAM_GB:-3}"; with_build_lock step "$@"
}
job_end() { # <rc> [status override]
  local rc="$1" status="${2:-}" t1; t1=$(date +%s)
  if [ -z "$status" ]; then case "$rc" in 0) status=ok;; 1) status=fail;; *) status=infra;; esac; fi
  [ "$rc" = 0 ] && [ "$SKIPPED" -gt 0 ] && status="ok-with-skips"
  printf '{"job":"%s","status":"%s","exit":%s,"seconds":%s,"skipped_steps":%s,"steps":[%s],"detail":"%s"}\n' \
    "$JOB" "$status" "$rc" "$((t1 - JOB_T0))" "$SKIPPED" "$STEPS" "$(echo "$DETAIL" | tr -d '"\\' | tr '\n' ' ')" >"$RUN_DIR/$JOB.json"
  printf -- '- `%s`: **%s** (exit %s, %ss%s)%s\n' "$JOB" "$status" "$rc" "$((t1 - JOB_T0))" \
    "$([ "$SKIPPED" -gt 0 ] && echo ", $SKIPPED step(s) skipped")" "$([ -n "$DETAIL" ] && echo " - $DETAIL")" >>"$RUN_DIR/summary.md"
  echo "=== $JOB: $status (exit $rc)"
  return "$rc"
}
JOB_T0=0
run_job() { # <job> <fn> [args...]
  local j="$1" fn="$2" rc; shift 2
  JOB_T0=$(date +%s); job_begin "$j" "$*"; "$fn" "$@"; rc=$?
  [ "$rc" -gt 2 ] && rc=2   # tool exit codes above 1 (missing file, crash) are infrastructure errors
  job_end "$rc"
}

# ---------- jobs ----------
job_queue() {
  local rc=0
  step verify-queue scripts/verify-queue.sh all || rc=1
  if xt_has register; then bstep register-check cargo xt register check || rc=1; fi
  check_quarantine_format || rc=1
  return $rc
}

# Format check of ci/quarantine.toml until `cargo xt quarantine check` exists (WP6b).
check_quarantine_format() {
  local f=ci/quarantine.toml out
  [ -f "$f" ] || { note "quarantine: $f missing"; return 2; }
  out=$(awk '
    function fin(   k, miss) {
      if (!n) return
      for (k in req) if (!(k in v) || v[k]=="") { printf "entry %d (%s): empty or missing %s\n", n, v["name"], k; bad++ }
      if (v["class"] != "" && !(v["class"] in cls)) { printf "entry %d (%s): unknown class %s\n", n, v["name"], v["class"]; bad++ }
      if (v["class"] != "" && v["app"] != "" && !(v["app"] in apps)) { printf "entry %d: unknown app %s\n", n, v["app"]; bad++ }
      delete v
    }
    BEGIN { split("app crate binary name class env reason evidence since review", r, " "); for (i in r) req[r[i]]=1
            split("env upstream-bug flaky flaky-crash brand-overlay i18n-incomplete debug-only", c, " "); for (i in c) cls[c[i]]=1
            split("wordcraft gridcraft deckcraft pdfcraft", a, " "); for (i in a) apps[a[i]]=1 }
    /^\[\[test\]\]/ { fin(); n++; next }
    /^[a-z_]+[ \t]*=/ && n { key=$1; val=$0; sub(/^[^=]*=[ \t]*/, "", val); gsub(/^"|"[ \t]*$/, "", val); v[key]=val }
    END { fin(); printf "%d entries, %d problems\n", n, bad+0; exit (bad ? 1 : 0) }' "$f") ; local rc=$?
  step quarantine-format echo "$out"; [ $rc -eq 0 ] || note "quarantine format check failed"
  return $rc
}

# cargo fmt --check without the diff flood: the full diff goes to the log, the terminal gets the file list.
fmt_check() { # <name> [manifest]
  local name="$1" out rc; local -a m=(); [ -n "${2:-}" ] && m=(--manifest-path "$2")
  echo "--- fmt-$name: cargo fmt --all --check ${m[*]:-}" | tee -a "$JOB_LOG"
  out=$(cargo fmt --all --check "${m[@]}" -- --color never 2>&1); rc=$?
  echo "$out" >>"$JOB_LOG"; STEPS="${STEPS:+$STEPS,}\"fmt-$name:$rc\""
  if [ $rc -ne 0 ]; then
    echo "$out" | sed -n 's/^Diff in //p' | sed 's/:[0-9]*:\r*$//; s#.*[\\/]office[\\/]##; s#\\#/#g' | sort | uniq -c | sed 's/^/    not fmt-clean: /'
    note "fmt-$name: $(echo "$out" | grep -c '^Diff in') diff hunk(s)"
  fi
  return $rc
}

job_lint() {
  local rc=0 app
  fmt_check root || rc=1
  bstep clippy-root env CARGO_TARGET_DIR="$OFFICE_TARGET/root" cargo clippy --workspace --all-targets -j 6 -- -D warnings || rc=1
  for app in $APPS; do
    [ -f "upstream/$app/Cargo.toml" ] || continue
    fmt_check "$app" "upstream/$app/Cargo.toml" || rc=1
    if grep -q '^layers\b\|"layers"' "upstream/$app/xtask/src/main.rs" 2>/dev/null; then
      bstep "layers-$app" env CARGO_TARGET_DIR="$OFFICE_TARGET/upstream-$app" cargo run --manifest-path "upstream/$app/Cargo.toml" -p xtask -- layers || rc=1
    fi
    if [ "$app" = pdfcraft ] || [ "$app" = wordcraft ]; then
      bstep "assets-$app" env CARGO_TARGET_DIR="$OFFICE_TARGET/upstream-$app" cargo run --manifest-path "upstream/$app/Cargo.toml" -p xtask -- assets || rc=1
    fi
    upstream_clippy_ratchet "$app" || rc=1   # ratchet: fails only on a warning that is not in ci/clippy-baseline/<app>.txt
  done
  return $rc
}
# Upstream clippy ratchet (PLAN 6.1): upstream's own warnings are baselined in ci/clippy-baseline/<app>.txt (one warning per line:
# path<TAB>message<TAB>lint, no line numbers so that unrelated edits do not move them). The job FAILS on a warning that is not in the
# baseline (a new one, or one more of the same kind), and only notes baselined warnings that disappeared (shrink the file then).
# A newer clippy than upstream's CI may add lints we did not cause: refresh the baseline in that case, with a STATUS line.
# Refresh: scripts/ci.sh lint with CI_CLIPPY_REFRESH=1 rewrites the files from the current output (lint names need --message-format json,
# see docs/phase1/wp6-final.md; a refresh keeps the lint column of rows that already exist).
clippy_keys() { # stdin: cargo clippy --message-format short output; stdout: sorted "path<TAB>message"
  tr -d '\r' | sed -n -E 's#\\#/#g; s#^([^ :][^:]*):[0-9]+:[0-9]+: warning: (.*)$#\1\t\2#p' | sed -E 's/: help: .*$//' | LC_ALL=C sort
}
upstream_clippy_ratchet() {
  local app="$1" base=ci/clippy-baseline/$1.txt out cur basek new gone crc
  if [ "$NO_BUILD" = 1 ]; then note "clippy-ratchet-$app: skipped (no-build)"; SKIPPED=$((SKIPPED+1)); return 0; fi
  ram_gate "${CI_RAM_GB:-3}"
  out=$(mktemp); cur=$(mktemp); basek=$(mktemp)
  with_build_lock env CARGO_TARGET_DIR="$OFFICE_TARGET/upstream-$app" cargo clippy --workspace --all-targets -j 6 \
        --manifest-path "upstream/$app/Cargo.toml" --message-format short >"$out" 2>&1; crc=$?
  cat "$out" >>"$JOB_LOG"
  if [ "$crc" -ne 0 ]; then note "upstream clippy $app: cargo clippy exit $crc (does not compile; see $JOB_LOG)"; rm -f "$out" "$cur" "$basek"; return 1; fi
  clippy_keys <"$out" >"$cur"
  if [ ! -f "$base" ]; then
    note "upstream clippy $app: $(wc -l <"$cur" | tr -d ' ') warnings, NO baseline file $base"; rm -f "$out" "$cur" "$basek"; return 1
  fi
  grep -v '^#' "$base" | cut -f1,2 | LC_ALL=C sort >"$basek"
  new=$(LC_ALL=C comm -23 "$cur" "$basek"); gone=$(LC_ALL=C comm -13 "$cur" "$basek")
  rm -f "$out"
  if [ "${CI_CLIPPY_REFRESH:-0}" = 1 ]; then
    { echo "# upstream clippy baseline for $app (path<TAB>message<TAB>lint<TAB>origin); see scripts/ci.sh clippy_keys"
      local T=$'\t' p m
      while IFS=$'\t' read -r p m; do printf '%s\t%s\t%s\n' "$p" "$m" "$(grep -F "$p$T$m$T" "$base" | head -1 | cut -f3-)"; done <"$cur"; } >"$base.new" && mv "$base.new" "$base"
    note "upstream clippy $app: baseline refreshed ($(wc -l <"$cur" | tr -d ' ') warnings)"; rm -f "$cur" "$basek"; return 0
  fi
  if [ -n "$new" ]; then
    note "FAIL upstream clippy $app: $(echo "$new" | wc -l | tr -d ' ') new warning(s) not in $base"; echo "$new" | sed 's/^/    NEW: /' | tee -a "$JOB_LOG"
    rm -f "$cur" "$basek"; return 1
  fi
  note "upstream clippy $app: $(wc -l <"$cur" | tr -d ' ') warnings, all baselined${gone:+; $(echo "$gone" | wc -l | tr -d ' ') baselined warning(s) gone, shrink $base}"
  rm -f "$cur" "$basek"; return 0
}

job_gen_check() {
  if [ ! -d domains ]; then note "stub: domains/ absent (WP3 pending), nothing to check"; return 0; fi
  local rc=0 d
  if xt_has gen-domain; then bstep gen-domain cargo xt gen-domain --check all || rc=1
  else note "stub: cargo xt gen-domain not implemented yet"; fi
  if xt_has lockdiff; then
    for d in $DOMAINS; do
      bstep "lockdiff-$d" cargo xt lockdiff "domains/$d/Cargo.lock" "upstream/$(app_of "$d")/Cargo.lock" || rc=1
    done
  fi
  return $rc
}

# One deny run. Prints "ok <name>" or "FAIL <name>"; rc 0 ok, 1 findings, 2 infrastructure.
deny_one() { # <name> <manifest> <config>
  local name="$1" manifest="$2" cfg="$3" out rc
  [ -f "$manifest" ] || { echo "INFRA $name: $manifest missing"; return 2; }
  out=$(cargo deny --manifest-path "$manifest" --config "$cfg" check --hide-inclusion-graph \
        -A unmatched-skip -A license-not-encountered -A advisory-not-detected 2>&1); rc=$?
  echo "$out" >>"$JOB_LOG"
  if [ $rc -eq 0 ]; then echo "ok $name"; return 0; fi
  if [ "$(echo "$out" | grep -c '^error\[')" -gt 0 ]; then   # not grep -q: SIGPIPE + pipefail
    echo "FAIL $name: $(echo "$out" | grep -c '^error\[') error(s); $(echo "$out" | tail -1)"; echo "$out" | grep '^error\[' | sort | uniq -c | sort -rn | head -8 | sed 's/^/    /'
    return 1
  fi
  echo "INFRA $name: cargo deny exit $rc: $(echo "$out" | tail -2 | tr '\n' ' ')"; return 2
}
job_deny() {
  local rc=0 r d line
  command -v cargo-deny >/dev/null 2>&1 || { note "cargo-deny not found on PATH"; return 2; }
  mkdir -p "$OFFICE_CACHE/advisory-db"
  line=$(deny_one root Cargo.toml deny.toml); r=$?; echo "$line" | tee -a "$JOB_LOG"; [ $r -gt $rc ] && rc=$r
  for d in $DOMAINS; do
    domain_standin "$d" && echo "(domain $d: stand-in upstream/$(app_of "$d") until WP3 generates domains/$d)" | tee -a "$JOB_LOG"
    line=$(deny_one "$d" "$(domain_manifest "$d")" "ci/deny/$d.toml"); r=$?; echo "$line" | tee -a "$JOB_LOG"
    [ $r -gt $rc ] && rc=$r
  done
  [ $rc -ne 0 ] && DETAIL="deny findings, see $JOB_LOG"
  return $rc
}

job_brand() {
  local rc=0
  # Source scan: the working name only in brand/brand.toml (rule W), no brand-mark includes outside cfg(not(office_overlay)) (rule M).
  bstep brand-grep cargo xt brand-grep || rc=1
  # Binary scan (needs the release builds): no upstream URLs, no brand-mark bytes; token counts are reported.
  if ls "$OFFICE_TARGET"/docs/release/docs.exe "$OFFICE_TARGET"/sheets/release/sheets.exe "$OFFICE_TARGET"/slides/release/slides.exe "$OFFICE_TARGET"/pdf/release/pdf.exe >/dev/null 2>&1      || ls "$OFFICE_TARGET"/dist/*-web/*.wasm >/dev/null 2>&1; then
    bstep brand-scan-bin cargo xt brand-scan-bin || rc=1
  else
    note "brand-scan-bin: no release binaries yet (scripts/build.sh <d> all --release)"
  fi
  return $rc
}

job_i18n() {
  local rc=0
  if xt_has i18n; then
    bstep i18n-lint cargo xt i18n lint || rc=1
    bstep i18n-roundtrip cargo xt i18n roundtrip || rc=1   # every canonical catalog re-exports to upstream's own ru catalog
    bstep i18n-check-stale cargo xt i18n check-stale || rc=1   # an upstream catalog changed without a catalog merge
  else note "stub until WP9: cargo xt i18n lint not implemented"; fi
  return $rc
}

apps_arg() { # all|<app>|<domain> -> app list
  case "${1:-all}" in all) echo "$APPS";; wordcraft|gridcraft|deckcraft|pdfcraft) echo "$1";; docs|sheets|slides|pdf) app_of "$1";; *) return 1;; esac
}
domains_arg() {
  case "${1:-all}" in all) echo "$DOMAINS";; docs|sheets|slides|pdf) echo "$1";; wordcraft|gridcraft|deckcraft|pdfcraft) domain_of "$1";; *) return 1;; esac
}

# Overlay environment for the upstream test builds (same as scripts/build.sh overlay_prepare). OFFICE_NO_OVERLAY=1 builds
# pristine-like (overlay off) and also drops the quarantine entries of class brand-overlay / i18n-incomplete.
test_overlay() { # <app>
  local app="$1" d; d=$(domain_of "$app")
  if [ "${OFFICE_NO_OVERLAY:-0}" = 1 ]; then unset OFFICE_OVERLAY_DIR OFFICE_PRODUCT_VERSION OFFICE_PRODUCT_NAME; return 0; fi
  [ "$NO_BUILD" = 1 ] && { export OFFICE_OVERLAY_DIR="$(overlay_dir "$d")"; return 0; }
  (unset CARGO_TARGET_DIR RUSTC_WRAPPER; cargo xt overlay "$d"     && cargo xt i18n export "$app" --mode "${OFFICE_I18N_MODE:-review}" --out "$OFFICE_OVERLAY_ROOT/_i18n") || return 2
  export OFFICE_OVERLAY_DIR="$(overlay_dir "$d")"
  local f; for f in "$OFFICE_OVERLAY_ROOT/_i18n/$app/"*; do cmp -s "$f" "$OFFICE_OVERLAY_DIR/$(basename "$f")" || cp "$f" "$OFFICE_OVERLAY_DIR/" || return 2; done
  export OFFICE_PRODUCT_NAME=$(sed -n 's/^PRODUCT_NAME=//p' "$OFFICE_OVERLAY_DIR/overlay.env" | tr -d '
')
  export OFFICE_PRODUCT_VERSION=$(sed -n 's/^VERSION=//p' "$OFFICE_OVERLAY_DIR/overlay.env" | tr -d '
')
}

# deckcraft's fonts build.rs prints rerun-if-changed for fonts/latin-manifest.txt, which our craft-fonts pin does not have: a missing
# file is always "changed", so the build script and the 11 crates above it rebuilt on EVERY cargo invocation (20 min per call).
# The test builds use a copy of the pin with an empty latin-manifest.txt (same embedded fonts; the file only lists Latin extras).
test_fonts() { # <app>
  [ "$1" = deckcraft ] || return 0
  local src="$OFFICE_ROOT/upstream/craft-fonts" dst="$OFFICE_CACHE/craft-fonts-ci"
  [ -f "$src/fonts/latin-manifest.txt" ] && return 0
  [ "$NO_BUILD" = 1 ] && return 0
  if ! cmp -s "$src/fonts/manifest.txt" "$dst/fonts/manifest.txt" 2>/dev/null; then
    rm -rf "$dst"; mkdir -p "$dst"; cp -r "$src/fonts" "$dst/fonts"; : > "$dst/fonts/latin-manifest.txt"
  fi
  export CRAFT_FONTS_DIR="$dst"
}

# Debug tests of one upstream workspace (patched tree) with the overlay on; quarantined tests run in a separate pass
# (cargo xt quarantine apply) which reports stale / still-failing / missing entries. nextest -j is TEST threads.
test_app() { # <app> <debug|release>
  local app="$1" mode="$2" m=upstream/$1/Cargo.toml tdir="$OFFICE_TARGET/upstream-$1" rc=0 rel="" filt="all()"
  [ "$mode" = release ] && rel="--release"
  if [ "$NO_BUILD" != 1 ] && [ ! -f ci/nextest.toml ]; then note "ci/nextest.toml missing"; return 2; fi
  test_overlay "$app" || return 2
  test_fonts "$app"
  # the xtask runs in the root target dir; the upstream target dir is passed to nextest explicitly (--target-dir)
  # Run the built xtask directly, not through `cargo run`: cargo run exports CARGO_MANIFEST_DIR/CARGO_PKG_*/CARGO_BIN_NAME,
  # which would leak into the nested nextest build and rebuild the upstream crates that read them (70 min for pdfcraft).
  xtq() { (unset CARGO_TARGET_DIR CARGO_BUILD_JOBS; cargo build -q -p xtask && "$OFFICE_TARGET/core/debug/xtask.exe" "$@"); }
  export UP_TARGET="$tdir"
  # nextest keeps its store (junit, archives) in <workspace>/target/nextest, i.e. inside upstream/: point it at the target dir
  export NEXTEST_CFG="$tdir/nextest-ci.toml"; mkdir -p "$tdir"
  { cat ci/nextest.toml; printf '
[store]
dir = "%s/nextest"
' "$tdir"; } > "$NEXTEST_CFG"
  bstep "quarantine-check" xtq quarantine check || return 1
  if [ "$NO_BUILD" != 1 ]; then filt=$(xtq quarantine filter "$app") || return 2; fi
  # the quarantined pass first: it builds the test binaries, the blocking pass below reuses them
  bstep "quarantine-$app" xtq quarantine apply "$app" || rc=1
  bstep "nextest-$app" cargo nextest run --workspace --profile ci --config-file "$NEXTEST_CFG" --manifest-path "$m" --target-dir "$tdir"     --build-jobs 4 --test-threads 4 $rel -E "$filt" || rc=1
  if [ "$mode" = debug ]; then
    bstep "doctests-$app" cargo test --doc --workspace --manifest-path "$m" -j 4 || rc=1
  fi
  unset CARGO_TARGET_DIR CARGO_BUILD_JOBS
  export CRAFT_FONTS_DIR="$OFFICE_ROOT/upstream/craft-fonts"   # undo test_fonts
  return $rc
}
job_test() { local rc=0 a; for a in $(apps_arg "${1:-all}") ; do test_app "$a" debug || rc=$?; done; return $rc; }
job_release_tests() { local rc=0 a; for a in $(apps_arg "${1:-all}"); do test_app "$a" release || rc=$?; done; return $rc; }

job_test_ours() {
  local rc=0 d filt="all()"
  bstep root-tests env CARGO_TARGET_DIR="$OFFICE_TARGET/root" cargo nextest run --workspace --profile ci --config-file ci/nextest.toml --build-jobs 4 --test-threads 4 || rc=1
  for d in $DOMAINS; do
    [ -f "domains/$d/Cargo.toml" ] || continue
    # pdfcraft's main.rs tests egui_wgpu_* and winit_carries_* read vendor/ and Cargo.lock relative to CARGO_MANIFEST_DIR/../.. (the upstream
    # workspace layout); in the domain wrapper those paths do not exist. They run, and pass, in the upstream suite (ci.sh test pdfcraft).
    [ "$d" = pdf ] && filt='not (binary_id(=pdf-desktop::bin/pdf) & (test(=tests::egui_wgpu_carries_the_surface_size_fit) | test(=tests::egui_wgpu_returns_a_surface_it_cannot_configure_as_an_error) | test(=tests::winit_carries_the_windows_11_monitor_scale_fix)))' || filt="all()"
    bstep "domain-tests-$d" env CARGO_TARGET_DIR="$(target_dir_domain "$d")" cargo nextest run --workspace --profile ci --config-file ci/nextest.toml --build-jobs 4 --test-threads 4 --manifest-path "domains/$d/Cargo.toml" -E "$filt" || rc=1
  done
  return $rc
}

job_check() {
  local rc=0 d m t
  for d in $(domains_arg "${1:-all}"); do
    m="$(domain_manifest "$d")"; t="$(target_dir_domain "$d")"
    domain_standin "$d" && note "check $d: stand-in $m (WP3 pending)"
    bstep "check-desktop-$d" env CARGO_TARGET_DIR="$t" cargo check -j 6 --manifest-path "$m" -p "$(domain_pkg "$d" desktop)" || rc=1
    bstep "check-web-$d" env CARGO_TARGET_DIR="$t" cargo check -j 6 --manifest-path "$m" -p "$(domain_pkg "$d" web)" --target wasm32-unknown-unknown || rc=1
  done
  return $rc
}

# WP13: keep approach B's binding layer (domains/docs/crates/docs-bind) compiling and tested against the current wordcraft snapshot.
# Native: check + test. wasm32: check + build (the cdylib the WebView shell loads) + the tests compiled but not run (no wasm
# test runner; its two tests are plain #[test]). The wasm steps run without sccache (it cannot spawn rustc there on Windows).
job_bind() {
  local rc=0 m=domains/docs/Cargo.toml t; t="$(target_dir_domain docs)"
  [ -f domains/docs/crates/docs-bind/Cargo.toml ] || { note "docs-bind: domains/docs/crates/docs-bind missing"; return 2; }
  bstep bind-check-native env CARGO_TARGET_DIR="$t" cargo check --locked -j 6 --manifest-path "$m" -p docs-bind || rc=1
  bstep bind-test-native env CARGO_TARGET_DIR="$t" cargo test --locked -j 6 --manifest-path "$m" -p docs-bind || rc=1
  bstep bind-check-wasm32 env -u RUSTC_WRAPPER CARGO_TARGET_DIR="$t" cargo check --locked -j 6 --manifest-path "$m" -p docs-bind --target wasm32-unknown-unknown || rc=1
  bstep bind-build-wasm32 env -u RUSTC_WRAPPER CARGO_TARGET_DIR="$t" cargo build --locked -j 6 --manifest-path "$m" -p docs-bind --target wasm32-unknown-unknown || rc=1
  bstep bind-test-wasm32-compile env -u RUSTC_WRAPPER CARGO_TARGET_DIR="$t" cargo test --locked -j 6 --manifest-path "$m" -p docs-bind --target wasm32-unknown-unknown --no-run || rc=1
  return $rc
}

job_build() {
  local rc=0 d k kinds="${2:-all}"
  [ "$kinds" = all ] && kinds="desktop web cli"
  [ "$NO_BUILD" != 1 ] && [ ! -x scripts/build.sh ] && { note "scripts/build.sh missing (WP3)"; return 2; }
  for d in $(domains_arg "${1:-all}"); do
    for k in $kinds; do bstep "build-$d-$k" scripts/build.sh "$d" "$k" --release || rc=1; done
  done
  return $rc
}

# ---------- sizes ----------
bytes() { wc -c <"$1" | tr -d ' '; }
br_bytes() { brotli -q 11 --lgwin=24 -c "$1" | wc -c | tr -d ' '; }
gz_bytes() { gzip -9 -c "$1" | wc -c | tr -d ' '; }
job_sizes() {
  local B=web/budgets.toml baseline=0 measure=0 rc=0 a d dist f raw gz br jsbr base dmax js_max budget head jhead
  for a in "$@"; do case "$a" in --baseline) baseline=1;; --measure-base) measure=1;; esac; done
  [ -f "$B" ] || { note "$B missing"; return 2; }
  if [ "$baseline" = 0 ] && [ "$NO_BUILD" = 1 ]; then
    baseline=1; note "no-build: no dist is built, showing the Phase 0 baseline against the budgets (--baseline)"
  fi
  [ "$baseline" = 1 ] || command -v brotli >/dev/null 2>&1 || { note "brotli CLI not found on PATH"; return 2; }
  printf '%-7s %-9s %12s %12s %12s %12s %12s %s\n' domain "(src)" raw gzip brotli budget headroom js_br/max | tee -a "$JOB_LOG"
  for d in $DOMAINS; do
    base=$(toml_get "$B" "$d" base_br); dmax=$(toml_get "$B" "$d" delta_br_max); js_max=$(toml_get "$B" "$d" js_br_max)
    budget=$((base + dmax))
    if [ "$baseline" = 1 ]; then
      raw=$(toml_get "$B" "$d" base_raw); gz=$(toml_get "$B" "$d" base_gzip); br=$base; jsbr=$(toml_get "$B" "$d" js_br); src=phase0
    else
      dist="${CI_DIST_ROOT:-$OFFICE_TARGET/dist}/$d-web"
      [ -d "$dist" ] || { echo "INFRA $d: $dist missing (run build $d web)" | tee -a "$JOB_LOG"; rc=2; continue; }
      raw=0; gz=0; br=0; jsbr=0
      for f in "$dist"/*.wasm; do [ -e "$f" ] || continue; raw=$((raw+$(bytes "$f"))); gz=$((gz+$(gz_bytes "$f"))); br=$((br+$(br_bytes "$f"))); done
      for f in "$dist"/*.js;   do [ -e "$f" ] || continue; jsbr=$((jsbr+$(br_bytes "$f"))); done
      [ "$raw" -gt 0 ] || { echo "INFRA $d: no .wasm in $dist" | tee -a "$JOB_LOG"; rc=2; continue; }
      src=dist
    fi
    head=$((budget - br)); jhead=$((js_max - jsbr))
    printf '%-7s %-9s %12s %12s %12s %12s %12s %s/%s\n' "$d" "$src" "$raw" "$gz" "$br" "$budget" "$head" "$jsbr" "$js_max" | tee -a "$JOB_LOG"
    if [ "$baseline" = 0 ]; then
      [ "$head" -lt 0 ] && { echo "FAIL $d: wasm brotli $br exceeds budget $budget (delta $((br-base)) > $dmax)" | tee -a "$JOB_LOG"; [ $rc -lt 1 ] && rc=1; }
      [ "$jhead" -lt 0 ] && { echo "FAIL $d: js brotli $jsbr exceeds $js_max" | tee -a "$JOB_LOG"; [ $rc -lt 1 ] && rc=1; }
      [ "$measure" = 1 ] && echo "MEASURED-BASE $d: set base_raw=$raw base_gzip=$gz base_br=$br js_br=$jsbr in $B (pristine dist only)" | tee -a "$JOB_LOG"
    fi
  done
  [ "$baseline" = 1 ] && DETAIL="${DETAIL:+$DETAIL; }baseline table only"
  return $rc
}

job_web_smoke() {
  local cfg=ci/web-smoke/playwright.config.ts
  if [ "$NO_BUILD" = 1 ]; then note "skipped (no-build): needs built dists"; SKIPPED=$((SKIPPED+1)); return 0; fi
  [ -f "$cfg" ] || { note "$cfg missing (web-smoke spec not written yet)"; return 2; }
  step playwright env DIST_ROOT="${CI_DIST_ROOT:-$OFFICE_TARGET/dist}" npx playwright test --config "$cfg"
}

job_actionlint() {
  command -v actionlint >/dev/null 2>&1 || { note "actionlint not found on PATH (E:/office-tools/bin)"; return 2; }
  step actionlint actionlint .github/workflows/ci.yml
}

job_fidelity() {
  if [ ! -d corpus ]; then note "skipped: corpus/ not delivered"; return 0; fi
  [ -x scripts/fidelity.sh ] || { note "scripts/fidelity.sh missing (WP11)"; return 2; }
  bstep fidelity scripts/fidelity.sh
}
job_xlsx_bench() {
  [ -x scripts/xlsx-bench.sh ] || { note "scripts/xlsx-bench.sh missing (WP4/WP5 harness not wired)"; return 2; }
  bstep xlsx-bench scripts/xlsx-bench.sh
}
job_sync_dry_run() {
  [ -x scripts/sync-upstream.sh ] || { note "scripts/sync-upstream.sh missing (WP5)"; return 2; }
  bstep sync-dry-run scripts/sync-upstream.sh --dry-run --gates quick all
}

# ---------- profiles ----------
# Domains affected by the diff (PLAN 6.1): upstream/<app>, patches/<app>, domains/<d> -> that app; brand/, i18n/,
# assets/, crates/, xtask/ -> all. CI_DIFF_BASE (default HEAD~1) selects the range; working-tree edits count too.
affected_domains() {
  local files out="" d a
  files=$(git diff --name-only "${CI_DIFF_BASE:-HEAD~1}" 2>/dev/null; git ls-files --others --exclude-standard 2>/dev/null) || { echo "$DOMAINS"; return; }
  [ -n "$files" ] || { echo "$DOMAINS"; return; }
  if echo "$files" | grep -Eq '^(brand|i18n|assets|crates|xtask)/'; then echo "$DOMAINS"; return; fi
  for d in $DOMAINS; do
    a=$(app_of "$d")
    echo "$files" | grep -Eq "^(upstream/$a/|patches/$a/|domains/$d/)" && out="$out $d"
  done
  echo $out
}

profile_jobs() { # prints "job args" lines
  local p="$1" d a aff
  printf '%s\n' queue lint gen-check deny brand i18n
  case "$p" in
    quick) for d in $(affected_domains); do echo "check $d"; echo "test $(app_of "$d")"; [ "$d" = docs ] && echo bind; done;;
    full|nightly)
      echo "test-ours"
      for a in $APPS; do echo "test $a"; done
      for d in $DOMAINS; do echo "check $d"; done
      echo bind
      for d in $DOMAINS; do echo "build $d all"; done
      echo sizes; echo web-smoke; echo actionlint
      if [ "$p" = nightly ]; then echo release-tests; echo fidelity; echo xlsx-bench; fi;;
  esac
}

dispatch() { # <job> [args]
  local j="$1"; shift
  case "$j" in
    queue) run_job queue job_queue;;
    lint) run_job lint job_lint;;
    gen-check) run_job gen-check job_gen_check;;
    deny) run_job deny job_deny;;
    brand) run_job brand job_brand;;
    i18n) run_job i18n job_i18n;;
    test) run_job "test${1:+ $1}" job_test "$@";;
    test-ours) run_job test-ours job_test_ours;;
    check) run_job "check${1:+ $1}" job_check "$@";;
    bind) run_job bind job_bind;;
    build) run_job "build${1:+ $1}" job_build "$@";;
    sizes) run_job sizes job_sizes "$@";;
    web-smoke) run_job web-smoke job_web_smoke;;
    actionlint) run_job actionlint job_actionlint;;
    release-tests) run_job release-tests job_release_tests "$@";;
    fidelity) run_job fidelity job_fidelity;;
    xlsx-bench) run_job xlsx-bench job_xlsx_bench;;
    sync-dry-run) run_job sync-dry-run job_sync_dry_run;;
    *) echo "unknown job: $j" >&2; return 2;;
  esac
}

main() {
  local -a args=(); local a
  while [ $# -gt 0 ]; do
    case "$1" in
      --no-build) NO_BUILD=1;;
      --run) RUN_ID="$2"; shift;;
      -h|--help) sed -n '2,38p' "$0"; return 0;;
      *) args+=("$1");;
    esac; shift
  done
  RUN_DIR="$OFFICE_TARGET/ci/$RUN_ID"; mkdir -p "$RUN_DIR"
  [ ${#args[@]} -gt 0 ] || { echo "usage: scripts/ci.sh [--no-build] <job...>|quick|full|nightly (see header)" >&2; return 2; }
  printf '\n## ci.sh %s (run %s%s)\n' "${args[*]}" "$RUN_ID" "$([ "$NO_BUILD" = 1 ] && echo ", no-build")" >>"$RUN_DIR/summary.md"
  local rc=0 r any_fail=0 any_infra=0 line
  local -a queue=()
  case "${args[0]}" in
    quick|full|nightly)
      while IFS= read -r line; do queue+=("$line"); done < <(profile_jobs "${args[0]}");;
    *)  # group words: a known job name starts a new job, other words are its arguments
      local cur=""
      for a in "${args[@]}"; do
        if [[ " $JOBS " == *" $a "* ]]; then [ -n "$cur" ] && queue+=("$cur"); cur="$a"; else cur="${cur:+$cur }$a"; fi
      done
      [ -n "$cur" ] && queue+=("$cur");;
  esac
  for line in "${queue[@]}"; do
    # shellcheck disable=SC2086
    dispatch $line; r=$?
    [ "$r" = 1 ] && any_fail=1; [ "$r" -ge 2 ] && any_infra=1
  done
  echo "summary: $RUN_DIR/summary.md"
  if [ $any_fail = 1 ]; then return 1; elif [ $any_infra = 1 ]; then return 2; fi
  return 0
}
main "$@"
exit $?
