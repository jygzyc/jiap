#!/usr/bin/env bash
# Rebuild decompiler_fixtures.apk + classes.dex from the Java sources in this folder.
# Requires an Android SDK (d8 / aapt2 / zipalign / apksigner) and a JDK (javac / keytool).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT"

find_sdk() {
  if [ -n "${ANDROID_HOME:-}" ] && [ -d "$ANDROID_HOME" ]; then
    echo "$ANDROID_HOME"
    return
  fi
  if [ -n "${ANDROID_SDK_ROOT:-}" ] && [ -d "$ANDROID_SDK_ROOT" ]; then
    echo "$ANDROID_SDK_ROOT"
    return
  fi
  if [ -d /Users/toto/Library/Android/sdk ]; then
    echo /Users/toto/Library/Android/sdk
    return
  fi
  echo "error: Android SDK not found (set ANDROID_HOME or ANDROID_SDK_ROOT)" >&2
  exit 1
}

find_java_home() {
  if [ -n "${JAVA_HOME:-}" ] && [ -x "$JAVA_HOME/bin/javac" ]; then
    if "$JAVA_HOME/bin/javac" -version >/dev/null 2>&1; then
      echo "$JAVA_HOME"
      return
    fi
  fi
  for candidate in \
    "/Applications/Android Studio.app/Contents/jbr/Contents/Home" \
    "$HOME/Applications/Android Studio.app/Contents/jbr/Contents/Home"
  do
    if [ -x "$candidate/bin/javac" ]; then
      echo "$candidate"
      return
    fi
  done
  if command -v javac >/dev/null 2>&1 && javac -version >/dev/null 2>&1; then
    local javac_bin
    javac_bin="$(command -v javac)"
    echo "$(cd "$(dirname "$javac_bin")/.." && pwd)"
    return
  fi
  echo "error: javac not found (install a JDK or Android Studio JBR)" >&2
  exit 1
}

SDK="$(find_sdk)"
export JAVA_HOME
JAVA_HOME="$(find_java_home)"
export PATH="$JAVA_HOME/bin:$PATH"

ANDROID_JAR="$SDK/platforms/android-36/android.jar"
if [ ! -f "$ANDROID_JAR" ]; then
  echo "error: missing $ANDROID_JAR" >&2
  exit 1
fi

BUILD_TOOLS=""
while IFS= read -r dir; do
  if [ -x "$dir/d8" ] && [ -x "$dir/aapt2" ] && [ -x "$dir/apksigner" ] && [ -x "$dir/zipalign" ]; then
    BUILD_TOOLS="$dir"
  fi
done < <(ls -1d "$SDK/build-tools"/* 2>/dev/null | sort -V)
if [ -z "$BUILD_TOOLS" ]; then
  echo "error: no usable build-tools under $SDK/build-tools" >&2
  exit 1
fi

echo "SDK         $SDK"
echo "JAVA_HOME   $JAVA_HOME"
echo "build-tools $BUILD_TOOLS"
echo "android.jar $ANDROID_JAR"

WORK="$ROOT/build"
rm -rf "$WORK"
mkdir -p "$WORK/classes" "$WORK/dex"

echo "==> javac"
find "$ROOT/java" -name '*.java' -print0 \
  | xargs -0 javac --release 8 -g -classpath "$ANDROID_JAR" -d "$WORK/classes"

echo "==> d8"
"$BUILD_TOOLS/d8" --min-api 24 --output "$WORK/dex" \
  $(find "$WORK/classes" -name '*.class')
if [ ! -f "$WORK/dex/classes.dex" ]; then
  echo "error: d8 did not produce classes.dex" >&2
  exit 1
fi

echo "==> aapt2 link"
"$BUILD_TOOLS/aapt2" link \
  --manifest "$ROOT/AndroidManifest.xml" \
  -I "$ANDROID_JAR" \
  --min-sdk-version 24 \
  --target-sdk-version 28 \
  --version-code 1 \
  --version-name 1.0 \
  -o "$WORK/base.zip"

echo "==> add classes.dex"
cp "$WORK/base.zip" "$WORK/unaligned.apk"
(
  cd "$WORK/dex"
  zip -q -u "$WORK/unaligned.apk" classes.dex
)

echo "==> zipalign"
"$BUILD_TOOLS/zipalign" -p -f 4 "$WORK/unaligned.apk" "$WORK/aligned.apk"

KS="$ROOT/debug.keystore"
if [ ! -f "$KS" ]; then
  echo "==> keytool (debug.keystore)"
  keytool -genkeypair -keystore "$KS" -storepass android -alias androiddebugkey \
    -keypass android -keyalg RSA -keysize 2048 -validity 10000 \
    -dname "CN=Android Debug,O=Androguard,C=US"
fi

echo "==> apksigner"
"$BUILD_TOOLS/apksigner" sign \
  --ks "$KS" \
  --ks-pass pass:android \
  --key-pass pass:android \
  --ks-key-alias androiddebugkey \
  --out "$WORK/signed.apk" \
  "$WORK/aligned.apk"

cp "$WORK/dex/classes.dex" "$ROOT/classes.dex"
cp "$WORK/signed.apk" "$ROOT/decompiler_fixtures.apk"

echo "wrote $ROOT/classes.dex ($(wc -c < "$ROOT/classes.dex") bytes)"
echo "wrote $ROOT/decompiler_fixtures.apk ($(wc -c < "$ROOT/decompiler_fixtures.apk") bytes)"
