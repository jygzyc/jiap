#!/usr/bin/env bash
# Stamp provenance headers onto the adapted decx-engine sources.
#
# Usage:
#   tools/stamp-vendor-headers.sh <path-to-upstream-checkout>
#
# <path-to-upstream-checkout> must be an extraction of asLody/dexdec tag
# v1.0.2 (or a newer tag the vendored trees are being synced to), i.e. a
# directory containing dexdec/ and rusty-dex/ subcrates:
#
#   curl -sSL https://github.com/asLody/dexdec/archive/refs/tags/v1.0.2.tar.gz | tar xz
#   tools/stamp-vendor-headers.sh ./dexdec-1.0.2
#
# Files identical to upstream get a "byte-identical" header; differing files
# get a "MODIFIED" header (Apache-2.0 §4(b) prominent notice). Idempotent:
# existing headers are detected and skipped, so the script is safe to re-run
# after an upstream sync to refresh the classification.
#
# Layout note: decx-engine consolidates the former crates/dexdec,
# crates/rusty-dex and crates/shims/* — the dexdec tree is stamped from
# upstream dexdec/src, the rusty_dex module from upstream rusty-dex/src;
# the root shim modules (log/rayon/zstd/crc32fast.rs) are original
# decx-native code and never stamped.
#
# IMPORTANT: if you sync from a NEW upstream tag, update the version, commit,
# and URL baked into the header strings below and in crates/decx-engine/VENDORED.md.
set -euo pipefail

UP_CHECKOUT="${1:?usage: stamp-vendor-headers.sh <path-to-upstream-dexdec-checkout>}"
[ -d "$UP_CHECKOUT/dexdec/src" ] && [ -d "$UP_CHECKOUT/rusty-dex/src" ] \
  || { echo "not a dexdec workspace checkout: $UP_CHECKOUT" >&2; exit 1; }

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
UP_DEXDEC="$(cd "$UP_CHECKOUT/dexdec/src" && pwd)"
UP_RUSTY="$(cd "$UP_CHECKOUT/rusty-dex/src" && pwd)"

DEXDEC_PRISTINE_HDR='// decx-engine: in-house DEX decompiler engine (decx-native).
// Adapted from asLody/dexdec v1.0.2 (Apache-2.0): https://github.com/asLody/dexdec
// This file is byte-identical to upstream apart from this header.
// Provenance and modification policy: crates/decx-engine/VENDORED.md'
DEXDEC_MODIFIED_HDR='// decx-engine: in-house DEX decompiler engine (decx-native).
// Adapted from asLody/dexdec v1.0.2 (Apache-2.0): https://github.com/asLody/dexdec
// MODIFIED for decx-native: dependency shims as root modules, batch/caching
// APIs, streaming iterator signatures, rustc-compat fixes, extra tests.
// Provenance and modification policy: crates/decx-engine/VENDORED.md'
RUSTY_PRISTINE_HDR='// decx-engine rusty_dex module: DEX parser (decx-native).
// Adapted from rusty-rs/rusty-dex 0.2.0 (Apache-2.0): https://github.com/rusty-rs/rusty-dex
// Baseline: the copy bundled in the asLody/dexdec v1.0.2 workspace.
// This file is byte-identical to upstream apart from this header.
// Provenance and modification policy: crates/decx-engine/VENDORED.md'
RUSTY_MODIFIED_HDR='// decx-engine rusty_dex module: DEX parser (decx-native).
// Adapted from rusty-rs/rusty-dex 0.2.0 (Apache-2.0): https://github.com/rusty-rs/rusty-dex
// Baseline: the copy bundled in the asLody/dexdec v1.0.2 workspace.
// MODIFIED for decx-native: zip I/O rerouted to decx-apk; thiserror /
// regex / lazy_static / byteorder replaced by hand-written std code;
// crate:: paths rebased for the decx-engine module fold-in.
// Provenance and modification policy: crates/decx-engine/VENDORED.md'

stamp_file() { # stamp_file <path-relative-to-$ROOT> <upstream-file> <pristine-hdr> <modified-hdr>
  local path="$1" up="$2" pristine="$3" modified="$4" hdr
  if head -c 4096 "$ROOT/$path" | grep -q "Adapted from"; then
    return 0 # already stamped; classification refresh needs a de-stamp first
  fi
  if [ -f "$up" ] && diff -q "$up" "$ROOT/$path" >/dev/null 2>&1; then
    hdr="$pristine"
  else
    hdr="$modified"
    MOD_COUNT=$((MOD_COUNT + 1))
  fi
  printf '%s\n' "$hdr" | cat - "$ROOT/$path" > "$ROOT/$path.tmp" && mv "$ROOT/$path.tmp" "$ROOT/$path"
  TOTAL=$((TOTAL + 1))
}

cd "$ROOT"

TOTAL=0; MOD_COUNT=0
while IFS= read -r f; do
  case "$f" in rusty_dex/*|log.rs|rayon.rs|zstd.rs|crc32fast.rs) continue ;; esac
  stamp_file "crates/decx-engine/src/$f" "$UP_DEXDEC/$f" "$DEXDEC_PRISTINE_HDR" "$DEXDEC_MODIFIED_HDR"
done < <(cd crates/decx-engine/src && find . -name '*.rs' | sed 's|^\./||' | sort)
echo "dexdec-core: stamped $TOTAL files ($MOD_COUNT modified-class headers)"

TOTAL=0; MOD_COUNT=0
while IFS= read -r f; do
  stamp_file "crates/decx-engine/src/rusty_dex/$f" "$UP_RUSTY/$f" "$RUSTY_PRISTINE_HDR" "$RUSTY_MODIFIED_HDR"
done < <(cd crates/decx-engine/src/rusty_dex && find . -name '*.rs' | sed 's|^\./||' | sort)
echo "rusty-dex: stamped $TOTAL files ($MOD_COUNT modified-class headers)"

# tests/multidex.rs is vendored verbatim (modified locally only by the
# fold-in crate-reference rewrite); stamp it as pristine when unmodified.
t=crates/decx-engine/tests/multidex.rs
if [ -f "$t" ] && ! head -c 4096 "$t" | grep -q "Adapted from"; then
  if diff -q "$UP_CHECKOUT/rusty-dex/tests/multidex.rs" "$t" >/dev/null 2>&1; then
    printf '%s\n' "$RUSTY_PRISTINE_HDR" | cat - "$t" > "$t.tmp" && mv "$t.tmp" "$t"
  else
    printf '%s\n' "$RUSTY_MODIFIED_HDR" | cat - "$t" > "$t.tmp" && mv "$t.tmp" "$t"
  fi
fi

echo "note: after stamping, refresh the modified-file lists in"
echo "      crates/decx-engine/VENDORED.md from the diff output."
