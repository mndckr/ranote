#!/usr/bin/env bash
# Shared paths and pinned versions for the Android builds. Sourced, not run.
# Select the target with RSNOTE_ANDROID_TARGET=smoke|app (default: smoke).
# shellcheck disable=SC2034  # consumed by the scripts that source this file

ANDROID_ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)

# shellcheck source=../versions.env
# shellcheck disable=SC1091
source "$ANDROID_ROOT/versions.env"

# SDK location: honour ANDROID_HOME, else the conventional per-user path.
ANDROID_HOME=${ANDROID_HOME:-$HOME/Android/Sdk}
ANDROID_NDK_HOME=${ANDROID_NDK_HOME:-$ANDROID_HOME/ndk/$NDK_VERSION}
ANDROID_NDK_ROOT=${ANDROID_NDK_ROOT:-$ANDROID_NDK_HOME}

CACHE_DIR=${RSNOTE_ANDROID_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}/rsnote-android}
PIXIEWOOD_DIR=$CACHE_DIR/pixiewood
MESON_BIN=$CACHE_DIR/venv/bin/meson
SDKMANAGER=$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager
ADB=$ANDROID_HOME/platform-tools/adb

BUILD_DIR=$ANDROID_ROOT/build

die() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || die "missing '$1' (see misc/android/README.md)"
}

ANDROID_TARGET=${RSNOTE_ANDROID_TARGET:-smoke}
case "$ANDROID_TARGET" in
  smoke)
    APP_SRC_DIR=$ANDROID_ROOT/smoke
    APP_MANIFEST=$APP_SRC_DIR/rsnote-smoke.xml
    APP_APK=$BUILD_DIR/rsnote-smoke.apk
    APP_ID=org.rsnote.Smoke
    PACKAGE=org.rsnote.smoke
    LOG_TAG=rsnote-smoke
    ;;
  app)
    APP_SRC_DIR=$ANDROID_ROOT/app
    APP_MANIFEST=$APP_SRC_DIR/rsnote.xml
    APP_APK=$BUILD_DIR/rsnote-android.apk
    APP_ID=com.github.flxzt.rnote
    PACKAGE=com.github.flxzt.rnote
    LOG_TAG=rnote
    ;;
  *)
    die "unknown RSNOTE_ANDROID_TARGET '$ANDROID_TARGET' (expected smoke or app)"
    ;;
esac
ACTIVITY=org.gtk.android.ToplevelActivity

# Pixiewood and the pinned meson venv shadow system copies.
PATH=$CACHE_DIR/venv/bin:$PIXIEWOOD_DIR:$ANDROID_HOME/platform-tools:$PATH
export PATH
