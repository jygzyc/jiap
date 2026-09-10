#!/usr/bin/env bash
# Verify adapted-file classification AFTER headers were stamped: strip the
# known header block from each file, diff the remainder against the upstream
# baseline, and report any file whose real state disagrees with the header it
# carries (e.g. edited after stamping, or misclassified).
#
# Usage: tools/verify-vendor-headers.sh <path-to-upstream-dexdec-1.0.2-checkout>
set -euo pipefail

UP_CHECKOUT="${1:?usage: verify-vendor-headers.sh <path-to-upstream-checkout>}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

HDR_END='Provenance and modification policy'

classify() { # classify <file> <upstream-file> -> prints pristine|modified|new
  local f="$1" up="$2"
  local lines=0 first=1
  while IFS= read -r line; do
    lines=$((lines + 1))
    case "$line" in
      "// "*) ;;
      *) echo "BAD-HEADER:$f:$line"; return 1;;
    esac
    if [ "$first" = 1 ]; then
      case "$line" in *"Adapted from"*) ;; *) echo "BAD-HEADER:$f:$line"; return 1;; esac
      first=0
    fi
    case "$line" in *"$HDR_END"*) break;; esac
  done < "$f"
  if [ ! -f "$up" ]; then echo "new:$f"; return 0; fi
  if diff -q <(tail -n +$((lines + 1)) "$f") "$up" >/dev/null 2>&1; then
    echo "pristine:$f"
  else
    echo "modified:$f"
  fi
}

verify_tree() { # verify_tree <local-dir> <up-dir> <label> <skip-glob...>
  local local_dir="$1" up_dir="$2" label="$3"; shift 3
  local total=0 hdr_pristine=0 hdr_modified=0 mismatch=0 real head_state
  while IFS= read -r f; do
    for skip in "$@"; do case "$f" in $skip) continue 2 ;; esac; done
    total=$((total + 1))
    state="$(classify "$ROOT/$local_dir/$f" "$up_dir/$f" || true)"
    case "$state" in pristine:*) real=pristine;; modified:*) real=modified;; new:*) real=new;; *) echo "$state"; continue;; esac
    if head -c 4096 "$ROOT/$local_dir/$f" | grep -q "MODIFIED for decx-native"; then
      head_state=modified; hdr_modified=$((hdr_modified + 1))
    else
      head_state=pristine; hdr_pristine=$((hdr_pristine + 1))
    fi
    if [ "$real" != "$head_state" ]; then
      mismatch=$((mismatch + 1))
      echo "MISMATCH [$label] $f: header=$head_state actual=$real"
    fi
  done < <(cd "$ROOT/$local_dir" && find . -name '*.rs' | sed 's|^\./||' | sort)
  echo "[$label] total=$total header-pristine=$hdr_pristine header-modified=$hdr_modified mismatches=$mismatch"
}

# dexdec core tree: skip the rusty_dex module and the four original shim
# modules (they have no upstream counterpart; rusty_dex has its own tree).
verify_tree crates/decx-engine/src "$UP_CHECKOUT/dexdec/src" dexdec-core \
  'rusty_dex/*' 'log.rs' 'rayon.rs' 'zstd.rs' 'crc32fast.rs'
# DEX parser module, against its own upstream subcrate.
verify_tree crates/decx-engine/src/rusty_dex "$UP_CHECKOUT/rusty-dex/src" rusty-dex

# multidex integration test (rusty-dex upstream).
f=crates/decx-engine/tests/multidex.rs
state="$(classify "$ROOT/$f" "$UP_CHECKOUT/rusty-dex/tests/multidex.rs" || true)"
case "$state" in modified:*) real=modified;; *) real=pristine;; esac
if head -c 4096 "$ROOT/$f" | grep -q "MODIFIED for decx-native"; then head_state=modified; else head_state=pristine; fi
if [ "$real" != "$head_state" ]; then
  echo "MISMATCH [multidex] tests/multidex.rs: header=$head_state actual=$real"
else
  echo "[multidex] ok ($real)"
fi
