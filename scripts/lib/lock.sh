# Build lock and RAM gate (PLAN section 1.6). Source after scripts/env.sh.
# Honours E:/office-target/PAUSE (Phase 0 convention): while the file exists, jobs wait.

_lock_dir() { echo "${OFFICE_TARGET:-E:/office-target/p1}/BUILD.lock"; }

wait_unpaused() {
  while [ -e "E:/office-target/PAUSE" ]; do echo "paused (E:/office-target/PAUSE)"; sleep 30; done
}

# with_build_lock <cmd...>: one rustc-heavy job at a time (mkdir is atomic).
with_build_lock() {
  # nested call (ci.sh holds the lock and runs build.sh): the outer holder already serialises
  [ -n "${OFFICE_LOCK_HELD:-}" ] && { "$@"; return; }
  local lock; lock="$(_lock_dir)"; mkdir -p "$(dirname "$lock")"
  wait_unpaused
  until mkdir "$lock" 2>/dev/null; do echo "waiting for build lock"; sleep 10; done
  # shellcheck disable=SC2064
  trap "rmdir '$lock' 2>/dev/null" EXIT INT TERM
  export OFFICE_LOCK_HELD=1; "$@"; local rc=$?; unset OFFICE_LOCK_HELD
  rmdir "$lock" 2>/dev/null; trap - EXIT INT TERM
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
