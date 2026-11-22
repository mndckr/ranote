#!/usr/bin/env bash
# Collect device, GPU, app-version, memory, frame, crash and log data from the
# attached device into misc/android/build/device-check-<timestamp>.txt.
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=env.sh
# shellcheck disable=SC1091
source "$SCRIPT_DIR/env.sh"

[ -x "$ADB" ] || die "adb not found at $ADB (run 'just android-setup')"
"$ADB" get-state >/dev/null 2>&1 || die "no device or emulator attached"
"$ADB" shell pm path "$PACKAGE" >/dev/null 2>&1 ||
  die "$PACKAGE is not installed (install it first, e.g. 'just android-app-install')"

mkdir -p "$BUILD_DIR"
out=${1:-$BUILD_DIR/device-check-$(date +%Y%m%d-%H%M%S).txt}
logcat_filter=(AndroidRuntime:E Gtk:V Gdk:V GLib:V $LOG_TAG:V '*:S')
aapt2=$ANDROID_HOME/build-tools/$BUILD_TOOLS_VERSION/aapt2

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
    grep -E 'versionName|versionCode|firstInstallTime|lastUpdateTime|minSdk|targetSdk' || true
  if [ -f "$APP_APK" ]; then
    echo "APK $(sha256sum "$APP_APK")"
    if [ -x "$aapt2" ]; then
      "$aapt2" dump badging "$APP_APK" 2>/dev/null |
        grep -E '^package:' || true
    fi
  fi
  echo
  echo "## memory"
  "$ADB" shell dumpsys meminfo "$PACKAGE" | head -30 || true
  echo
  echo "## frame stats (interact before collecting; totals reset on app restart)"
  "$ADB" shell dumpsys gfxinfo "$PACKAGE" | head -40 || true
  echo
  echo "## frame stats (recent frame rows)"
  "$ADB" shell dumpsys gfxinfo "$PACKAGE" framestats | head -60 || true
  echo
  echo "## crashes"
  "$ADB" logcat -d -b crash -t 100 || true
  echo
  echo "## logcat (last 300 lines)"
  "$ADB" logcat -d -v threadtime "${logcat_filter[@]}" | tail -300 || true
} | tee "$out"

printf '\nWrote %s\n' "$out"
