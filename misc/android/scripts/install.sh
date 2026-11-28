#!/usr/bin/env bash
# Install the built APK on the connected device/emulator and launch it.
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=env.sh
# shellcheck disable=SC1091
source "$SCRIPT_DIR/env.sh"

[ -x "$ADB" ] || die "adb not found at $ADB (run 'just android-app-setup')"
[ -f "$APP_APK" ] || die "no APK at $APP_APK (run 'just android-app-build')"
"$ADB" devices | awk 'NR > 1 && $2 == "device" {found = 1} END {exit !found}' ||
  die "no device or emulator attached (adb devices)"

"$ADB" install -r "$APP_APK"
"$ADB" shell am force-stop "$PACKAGE"
"$ADB" shell am start -n "$PACKAGE/$ACTIVITY"
next_logcat='just android-app-logcat'
printf 'Launched %s. Follow logs with: %s\n' "$PACKAGE" "$next_logcat"
