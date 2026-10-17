# Import helpers (PLAN section 1.4). Source after scripts/env.sh. Run from the monorepo root.

# import_pristine <repo> <sha>: copy the tree objects of <sha> from the cache clone into this repo's
# object store (no upstream history), then place them at upstream/<repo> with exact modes and blob ids.
import_pristine() {
  local C="$UPSTREAM_CACHE/$1" tree
  tree=$(git -C "$C" rev-parse "$2^{tree}") || return 1
  { echo "$tree"; git -C "$C" ls-tree -r -t "$tree" | awk '{print $3}'; } \
    | git -C "$C" pack-objects -q --stdout | git unpack-objects -q
  git rm -r -q --cached --ignore-unmatch "upstream/$1"; rm -rf "upstream/$1"
  git read-tree --prefix="upstream/$1/" -u "$tree"
}

# write_base <repo> <sha>: write patches/<repo>/BASE (PLAN section 1.5)
write_base() {
  local C="$UPSTREAM_CACHE/$1" sha
  sha=$(git -C "$C" rev-parse "$2^{commit}")
  mkdir -p "patches/$1"
  {
    echo "repo=$1"
    echo "url=$(git -C "$C" remote get-url origin)"
    echo "sha=$sha"
    echo "tree=$(git -C "$C" rev-parse "$sha^{tree}")"
    echo "date=$(git -C "$C" log -1 --format=%cI "$sha")"
    echo "describe=$(git -C "$C" describe --tags --always "$sha" 2>/dev/null)"
    echo "synced=${3:-$(date +%F)}"
  } > "patches/$1/BASE"
}
