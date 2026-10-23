# Build lock and RAM gate (PLAN section 1.6). Source after scripts/env.sh.
# Honours E:/office-target/PAUSE (Phase 0 convention): while the file exists, jobs wait.

# Two slots (OFFICE_LOCK_SLOTS, default 2; 31.8 GB RAM): $OFFICE_TARGET/BUILD.lock.1 and .2. Two different cargo target
# dirs may build at once; one target dir never builds twice (a slot's `target` file holds its CARGO_TARGET_DIR).
_lock_base() { echo "${OFFICE_LOCK_DIR:-${OFFICE_TARGET:-E:/office-target/p1}}/BUILD.lock"; }   # OFFICE_LOCK_DIR: a build in its own OFFICE_TARGET still shares the global slots
_lock_key() { echo "${CARGO_TARGET_DIR:-${OFFICE_TARGET:-E:/office-target/p1}}"; }

# Stale = the owner pid is gone (kill -0), or no pid file after 2 min (the owner died between mkdir and writing it).
_lock_stale() {
  local d="$1" pid
  pid=$(cat "$d/pid" 2>/dev/null)
  if [ -n "$pid" ]; then kill -0 "$pid" 2>/dev/null || return 0; return 1; fi
  [ -n "$(find "$d" -maxdepth 0 -mmin +2 2>/dev/null)" ]
}

# _lock_take <key>: print the slot dir taken; return 1 if every slot is busy or the key already builds.
_lock_take() {
  local key="$1" n d o other
  for n in $(seq 1 "${OFFICE_LOCK_SLOTS:-2}"); do
    d="$(_lock_base).$n"
    [ -d "$d" ] && _lock_stale "$d" && { echo "removing stale build lock $d" >&2; rm -rf "$d"; }
    mkdir "$d" 2>/dev/null || continue
    echo "$$" > "$d/pid"; echo "$key" > "$d/target"
    for o in $(seq 1 "${OFFICE_LOCK_SLOTS:-2}"); do
      other="$(_lock_base).$o"; [ "$other" = "$d" ] && continue
      # both racers may see each other and back off; the random sleep in the caller breaks the tie
      if [ "$(cat "$other/target" 2>/dev/null)" = "$key" ]; then rm -rf "$d"; return 1; fi
    done
    echo "$d"; return 0
  done
  return 1
}

wait_unpaused() {
  while [ -e "E:/office-target/PAUSE" ]; do echo "paused (E:/office-target/PAUSE)"; sleep 30; done
}

# with_build_lock <cmd...>: at most OFFICE_LOCK_SLOTS rustc-heavy jobs, never two on the same target dir.
with_build_lock() {
  # nested call (ci.sh holds the lock and runs build.sh): the outer holder already serialises
  [ -n "${OFFICE_LOCK_HELD:-}" ] && { "$@"; return; }
  local slot; mkdir -p "$(dirname "$(_lock_base)")"
  wait_unpaused
  until slot=$(_lock_take "$(_lock_key)"); do
    echo "waiting for a build slot ($(_lock_key))"; sleep $(( ${OFFICE_LOCK_POLL:-10} + RANDOM % 3 ))
  done
  # shellcheck disable=SC2064
  trap "rm -rf '$slot'" EXIT INT TERM
  export OFFICE_LOCK_HELD=1; "$@"; local rc=$?; unset OFFICE_LOCK_HELD
  rm -rf "$slot"; trap - EXIT INT TERM
  return $rc
}

# ram_gate <GB>: wait until free physical memory is at least that many GB.
ram_gate() {
  local need_kb=$(( ${1:?GB} * 1024 * 1024 )) free_kb
  while :; do
    free_kb=$(powershell -NoProfile -Command "(Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory" | tr -d '\r')
    [ "${free_kb:-0}" -ge "$need_kb" ] && return 0
    echo "waiting for RAM: free ${free_kb} KB < ${need_kb} KB"; sleep 20
  done
}
