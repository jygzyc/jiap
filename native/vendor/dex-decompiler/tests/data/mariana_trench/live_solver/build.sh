#!/usr/bin/env bash
# Rebuild the checked-in Mariana Trench live solver fixture.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
if [ -z "${JAVA_HOME:-}" ] && [ -x "/Applications/Android Studio.app/Contents/jbr/Contents/Home/bin/javac" ]; then
  export JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home"
fi
export PATH="${JAVA_HOME:+$JAVA_HOME/bin:}$PATH"
BUILD_TOOLS=""
while IFS= read -r dir; do
  [ -x "$dir/d8" ] && BUILD_TOOLS="$dir"
done < <(ls -1d "$SDK"/build-tools/* 2>/dev/null | sort -V)

if [ -z "$BUILD_TOOLS" ] || [ ! -x "$BUILD_TOOLS/d8" ]; then
  echo "error: Android SDK d8 not found" >&2
  exit 1
fi

WORK="$ROOT/build"
rm -rf "$WORK"
mkdir -p "$WORK/classes" "$WORK/dex"
javac --release 8 -g -d "$WORK/classes" "$ROOT/LiveFlow.java"
"$BUILD_TOOLS/d8" --min-api 26 --no-desugaring --output "$WORK/dex" \
  "$WORK/classes/mt/live/LiveFlow.class" \
  "$WORK/classes/mt/live/Origin.class"
cp "$WORK/dex/classes.dex" "$ROOT/classes.dex"
echo "wrote $ROOT/classes.dex"
