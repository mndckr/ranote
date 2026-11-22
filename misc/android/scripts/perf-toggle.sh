#!/usr/bin/env bash
# Turn the debug-only rnote performance instrumentation on or off (ticket #12).
# Usage: perf-toggle.sh on|off
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=env.sh
# shellcheck disable=SC1091
source "$SCRIPT_DIR/env.sh"

[ "$ANDROID_TARGET" = "app" ] ||
  die "perf instrumentation exists only in the app build (RANOTE_ANDROID_TARGET=app)"
[ -x "$ADB" ] || die "adb not found at $ADB (run 'just android-app-setup')"
"$ADB" get-state >/dev/null 2>&1 || die "no device or emulator attached"
"$ADB" shell pm path "$PACKAGE" >/dev/null 2>&1 ||
  die "$PACKAGE is not installed (run 'just android-app-install')"

action=${1:-}
case "$action" in
  on | off) ;;
  *) die "usage: $0 on|off" ;;
esac

# The app checks perf.on in the XDG data dirs and the app cache dir. GTK sets
# XDG_DATA_DIRS/XDG_DATA_HOME from Java, so write to the internal share dir and
# cover the plausible cache dirs too.
for dir in files/share/rnote cache/rnote files/.cache/rnote .cache/rnote files/cache/rnote; do
  if [ "$action" = "on" ]; then
    "$ADB" shell "run-as $PACKAGE sh -c 'mkdir -p $dir && touch $dir/perf.on'"
  else
    "$ADB" shell "run-as $PACKAGE rm -f $dir/perf.on"
  fi
done

if [ "$action" = "on" ]; then
  printf 'Perf instrumentation on for %s. Restart the app, exercise the document,\n' "$PACKAGE"
  printf 'then run: just android-app-device-check\n'
else
  printf 'Perf instrumentation off for %s.\n' "$PACKAGE"
fi
