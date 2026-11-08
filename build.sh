#!/usr/bin/env bash
# Builds the Rust engine + Android app and (optionally) installs it on a connected device.
#   ./build.sh            debug build
#   ./build.sh release    minified release build (signed with the debug key for personal installs)
#   ./build.sh install    debug build + adb install + launch
set -euo pipefail
cd "$(dirname "$0")"
export PATH="$HOME/.cargo/bin:$PATH"
export JAVA_HOME="${JAVA_HOME:-/usr/lib/jvm/java-21-openjdk}"
variant=Debug
[[ "${1:-}" == "release" ]] && variant=Release
./gradlew --offline "assemble$variant" -q
apk=$(ls app/build/outputs/apk/${variant,,}/*.apk | head -1)
echo "built: $apk"
if [[ "${1:-}" == "install" || "${2:-}" == "install" ]]; then
  adb install -r "$apk"
  adb shell am start -n com.riyadm.ribbit.revived/.MainActivity
fi
