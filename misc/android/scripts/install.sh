#!/usr/bin/env bash
# Install the built APK on the connected device/emulator and launch it.
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=env.sh
# shellcheck disable=SC1091
source "$SCRIPT_DIR/env.sh"

[ -x "$ADB" ] || die "adb not found at $ADB (run 'just android-setup')"
[ -f "$APK" ] || die "no APK at $APK (run 'just android-build')"
"$ADB" devices | awk 'NR > 1 && $2 == "device" {found = 1} END {exit !found}' ||
  die "no device or emulator attached (adb devices)"

"$ADB" install -r "$APK"
"$ADB" shell am force-stop "$PACKAGE"
"$ADB" shell am start -n "$PACKAGE/$ACTIVITY"
printf 'Launched %s. Follow logs with: just android-logcat\n' "$PACKAGE"
