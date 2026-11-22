#!/usr/bin/env bash
# Install the pinned Android SDK/NDK, meson and pixiewood, then configure the
# selected build (RSNOTE_ANDROID_TARGET=smoke|app). Safe to re-run: it resets the
# generated meson build directories.
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=env.sh
# shellcheck disable=SC1091
source "$SCRIPT_DIR/env.sh"

require_command python3
require_command git
require_command curl
require_command unzip
require_command ninja
require_command java

if [ "$ANDROID_TARGET" = "app" ]; then
  # cargo resolves GTK via pkg-config; msgfmt builds the .mo files (see app/meson.build).
  require_command pkg-config
  require_command msgfmt
  require_command cargo
  require_command rustup
  for target in $RUST_TARGETS; do
    rustup target add "$target" >/dev/null
  done
fi

# --- Host checks -------------------------------------------------------------

java_major=$(java -version 2>&1 | awk -F'[".]' '/version/ {print $2}')
[ -n "$java_major" ] || die "could not parse 'java -version'"
[ "$java_major" -ge 17 ] || die "JDK 17+ required, found Java $java_major"
if [ "$java_major" -ne "$JDK_VERSION" ]; then
  printf 'warning: pinned JDK is %s, found %s (accepted, but pin %s for reproducibility)\n' \
    "$JDK_VERSION" "$java_major" "$JDK_VERSION" >&2
fi

# --- Pinned meson ------------------------------------------------------------

if [ ! -x "$MESON_BIN" ] || [ "$("$MESON_BIN" --version)" != "$MESON_VERSION" ]; then
  printf 'Installing meson %s into %s\n' "$MESON_VERSION" "$CACHE_DIR/venv"
  rm -rf "$CACHE_DIR/venv"
  python3 -m venv "$CACHE_DIR/venv"
  "$CACHE_DIR/venv/bin/pip" install --quiet --disable-pip-version-check \
    "meson==$MESON_VERSION"
fi
[ "$("$MESON_BIN" --version)" = "$MESON_VERSION" ] ||
  die "meson $MESON_VERSION not active"

# --- Android SDK -------------------------------------------------------------

if [ ! -x "$SDKMANAGER" ]; then
  mkdir -p "$ANDROID_HOME/cmdline-tools" "$CACHE_DIR"
  zip=$CACHE_DIR/cmdline-tools-$CMDLINE_TOOLS_VERSION.zip
  if [ ! -f "$zip" ]; then
    printf 'Downloading Android command-line tools %s\n' "$CMDLINE_TOOLS_VERSION"
    curl --fail --location --output "$zip" \
      "https://dl.google.com/android/repository/commandlinetools-linux-${CMDLINE_TOOLS_VERSION}_latest.zip"
  fi
  echo "$CMDLINE_TOOLS_SHA256  $zip" | sha256sum --check --status ||
    die "command-line tools checksum mismatch: $zip"
  tmp=$(mktemp -d "$CACHE_DIR/cmdline-tools.XXXXXX")
  unzip -q "$zip" -d "$tmp"
  rm -rf "$ANDROID_HOME/cmdline-tools/latest"
  mv "$tmp/cmdline-tools" "$ANDROID_HOME/cmdline-tools/latest"
  rmdir "$tmp"
fi

if [ ! -x "$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-strip" ] ||
   [ ! -d "$ANDROID_HOME/platforms/android-$ANDROID_API" ] ||
   [ ! -d "$ANDROID_HOME/build-tools/$BUILD_TOOLS_VERSION" ]; then
  printf 'Accepting SDK licenses and installing API %s, build-tools %s, NDK %s\n' \
    "$ANDROID_API" "$BUILD_TOOLS_VERSION" "$NDK_VERSION"
  yes | "$SDKMANAGER" --sdk_root="$ANDROID_HOME" --licenses >/dev/null || true
  "$SDKMANAGER" --sdk_root="$ANDROID_HOME" \
    "platform-tools" \
    "platforms;android-$ANDROID_API" \
    "build-tools;$BUILD_TOOLS_VERSION" \
    "ndk;$NDK_VERSION"
fi
[ -x "$ADB" ] || die "adb not found at $ADB after SDK install"

# --- Pinned pixiewood --------------------------------------------------------

if [ ! -d "$PIXIEWOOD_DIR/.git" ]; then
  git clone --quiet https://github.com/sp1ritCS/gtk-android-builder.git "$PIXIEWOOD_DIR"
fi
git -C "$PIXIEWOOD_DIR" fetch --quiet origin
git -C "$PIXIEWOOD_DIR" checkout --quiet "$PIXIEWOOD_COMMIT"
[ "$(git -C "$PIXIEWOOD_DIR" rev-parse HEAD)" = "$PIXIEWOOD_COMMIT" ] ||
  die "pixiewood is not at $PIXIEWOOD_COMMIT"
"$PIXIEWOOD_DIR/pixiewood" --version >/dev/null 2>&1 ||
  die "pixiewood cannot load; install its Perl/AppStream dependencies (see README)"

# --- Pin drift check ---------------------------------------------------------

manifest=$APP_MANIFEST
grep -q "revision=\"$GTK_COMMIT\"" "$manifest" ||
  die "$manifest does not pin GTK $GTK_VERSION ($GTK_COMMIT)"
grep -q "revision=\"$LIBADWAITA_COMMIT\"" "$manifest" ||
  die "$manifest does not pin libadwaita $LIBADWAITA_VERSION ($LIBADWAITA_COMMIT)"
grep -q "revision=\"$GLIB_COMMIT\"" "$manifest" ||
  die "$manifest does not pin GLib $GLIB_VERSION ($GLIB_COMMIT)"

# --- Configure ---------------------------------------------------------------

printf 'Configuring %s for aarch64 and x86_64\n' "$APP_ID"
cd "$APP_SRC_DIR"
rm -rf _pixiewood
"$PIXIEWOOD_DIR/pixiewood" prepare \
  --sdk "$ANDROID_HOME" \
  --toolchain "$ANDROID_NDK_HOME" \
  --meson "$MESON_BIN" \
  "$APP_MANIFEST"

if [ "$ANDROID_TARGET" = "app" ]; then
  NEXT_RECIPE="just android-app-build"
else
  NEXT_RECIPE="just android-build"
fi

cat <<EOF

Android toolchain ready ($ANDROID_TARGET):
  GTK             $GTK_VERSION ($GTK_COMMIT)
  libadwaita      $LIBADWAITA_VERSION ($LIBADWAITA_COMMIT)
  GLib            $GLIB_VERSION ($GLIB_COMMIT)
  NDK             $NDK_VERSION
  SDK platform    android-$ANDROID_API / build-tools $BUILD_TOOLS_VERSION
  Gradle / AGP    $GRADLE_VERSION / $AGP_VERSION (JDK $JDK_VERSION)
  pixiewood       $PIXIEWOOD_COMMIT
  meson           $MESON_VERSION

Next: ${NEXT_RECIPE}
EOF
