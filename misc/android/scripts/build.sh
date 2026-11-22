#!/usr/bin/env bash
# Cross-compile GTK/libadwaita + the smoke app, then package the APK.
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=env.sh
# shellcheck disable=SC1091
source "$SCRIPT_DIR/env.sh"

[ -d "$SMOKE_DIR/_pixiewood" ] || die "run 'just android-setup' first"
[ -x "$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-strip" ] ||
  die "NDK $NDK_VERSION not found at $ANDROID_NDK_HOME (run 'just android-setup')"

cd "$SMOKE_DIR"
"$PIXIEWOOD_DIR/pixiewood" generate
"$PIXIEWOOD_DIR/pixiewood" build "$@"

apk_dir=$SMOKE_DIR/_pixiewood/android/app/build/outputs/apk/debug
shopt -s nullglob
apks=("$apk_dir"/*.apk)
[ ${#apks[@]} -gt 0 ] || die "no APK produced under $apk_dir"

# Prefer the universal APK so one file works on tablets and emulators.
apk=${apks[0]}
for candidate in "${apks[@]}"; do
  [[ $candidate == *universal* ]] && apk=$candidate
done

mkdir -p "$BUILD_DIR"
cp "$apk" "$APK"
printf 'APK: %s (%s)\n' "$APK" "$(du -h "$APK" | cut -f1)"
printf 'Next: just android-install\n'
