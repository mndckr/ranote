#!/usr/bin/env bash
# Collect device, GPU, app-version, memory, frame and log data from the
# attached device into misc/android/build/device-check-<timestamp>.txt.
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=env.sh
# shellcheck disable=SC1091
source "$SCRIPT_DIR/env.sh"

[ -x "$ADB" ] || die "adb not found at $ADB (run 'just android-setup')"
"$ADB" get-state >/dev/null 2>&1 || die "no device or emulator attached"

mkdir -p "$BUILD_DIR"
out=${1:-$BUILD_DIR/device-check-$(date +%Y%m%d-%H%M%S).txt}
logcat_filter=(AndroidRuntime:E Gtk:V Gdk:V GLib:V rsnote-smoke:V '*:S')

{
  echo "# collected $(date --iso-8601=seconds)"
  echo "# adb $(command -v "$ADB")"
  echo
  echo "## device"
  "$ADB" shell getprop ro.product.manufacturer
  "$ADB" shell getprop ro.product.model
  "$ADB" shell getprop ro.product.device
  echo "Android $("$ADB" shell getprop ro.build.version.release) (SDK $("$ADB" shell getprop ro.build.version.sdk))"
  "$ADB" shell getprop ro.product.cpu.abi
  echo
  echo "## gpu"
  "$ADB" shell dumpsys SurfaceFlinger 2>/dev/null | grep -iE 'gles|gpu|renderer' | head -5 || true
  echo
  echo "## app"
  "$ADB" shell dumpsys package "$PACKAGE" |
    grep -E 'versionName|versionCode|firstInstallTime|lastUpdateTime' || true
  echo
  echo "## memory"
  "$ADB" shell dumpsys meminfo "$PACKAGE" | head -30 || true
  echo
  echo "## frame stats (needs interaction before collecting)"
  "$ADB" shell dumpsys gfxinfo "$PACKAGE" framestats | head -60 || true
  echo
  echo "## logcat (last 300 lines)"
  "$ADB" logcat -d -v threadtime "${logcat_filter[@]}" | tail -300 || true
} | tee "$out"

printf '\nWrote %s\n' "$out"
