#!/usr/bin/env bash
# Cross-compile GTK/libadwaita + the selected app, then package the APK.
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=env.sh
# shellcheck disable=SC1091
source "$SCRIPT_DIR/env.sh"

[ -d "$APP_SRC_DIR/.pixiewood" ] || die "run the matching android setup recipe first (see README)"
[ -x "$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-strip" ] ||
  die "NDK $NDK_VERSION not found at $ANDROID_NDK_HOME (run 'just android-setup')"

cd "$APP_SRC_DIR"
"$PIXIEWOOD_DIR/pixiewood" generate
if [ "$ANDROID_TARGET" = "app" ]; then
  # Pixiewood can only emit MIME-type filters; .rnote files arrive as
  # application/octet-stream, which needs a path-pattern filter (ticket #14).
  python3 "$SCRIPT_DIR/patch-manifest.py" \
    "$APP_SRC_DIR/.pixiewood/android/app/src/main/AndroidManifest.xml"
fi
"$PIXIEWOOD_DIR/pixiewood" build "$@"

apk_dir=$APP_SRC_DIR/.pixiewood/android/app/build/outputs/apk/debug
shopt -s nullglob
apks=("$apk_dir"/*.apk)
[ ${#apks[@]} -gt 0 ] || die "no APK produced under $apk_dir"

# Prefer the universal APK so one file works on tablets and emulators.
apk=${apks[0]}
for candidate in "${apks[@]}"; do
  [[ $candidate == *universal* ]] && apk=$candidate
done

mkdir -p "$BUILD_DIR"
cp "$apk" "$APP_APK"
printf 'APK: %s (%s)\n' "$APP_APK" "$(du -h "$APP_APK" | cut -f1)"
if [ "$ANDROID_TARGET" = "app" ]; then
  next_install='just android-app-install'
else
  next_install='just android-install'
fi
printf 'Next: %s\n' "$next_install"
