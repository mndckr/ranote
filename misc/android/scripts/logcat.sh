#!/usr/bin/env bash
# Follow the selected app's logcat stream. Extra arguments go to adb logcat.
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=env.sh
# shellcheck disable=SC1091
source "$SCRIPT_DIR/env.sh"

[ -x "$ADB" ] || die "adb not found at $ADB (run 'just android-app-setup')"
exec "$ADB" logcat -v threadtime \
  AndroidRuntime:E \
  Gtk:V Gdk:V GLib:V $LOG_TAG:V \
  '*:S' "$@"
