#!/usr/bin/env bash
# Build rnote-ui for one Android ABI and install it as a shared library.
#
# GTK's Android RuntimeApplication dlopen()s the app and calls an exported
# `main`, so this is `cargo rustc` with -shared instead of a normal binary.
# gtk-rs resolves GLib/GTK via pkg-config; the NDK has no pkg-config, so this
# wrapper points it at meson's uninstalled .pc files from the pixiewood build.
# That is the whole gtk-rs Android workaround; no gtk-rs fork or patch.
set -euo pipefail

if [ "$#" -ne 8 ]; then
  echo "usage: cargo-build.sh <repo-root> <build-root> <rust-target> <profile> <output> <cc> <cxx> <cargo>" >&2
  exit 2
fi

repo_root=$1
build_root=$2
rust_target=$3
profile=$4
output=$5
# Meson passes a build-root-relative path but this script cd's to the repo
# root, and pixiewood builds both ABIs in parallel: absolutise so they cannot
# race on the same file.
output=$(realpath -m "$output")
cc=$6
cxx=$7
cargo=$8

profile_args=()
if [ "$profile" = "release" ]; then
  profile_args=(--release)
fi

# --- pkg-config over meson's uninstalled files -------------------------------
# Meson writes <name>-uninstalled.pc; pkg-config (and system-deps) ask for
# <name>.pc. Symlink the stripped names into one directory and search only it,
# so host GTK cannot leak into the cross build (PKG_CONFIG_LIBDIR).
pc_dir=$build_root/android-pkgconfig
rm -rf "$pc_dir"
mkdir -p "$pc_dir"
shopt -s nullglob
for pc in "$build_root"/meson-uninstalled/*.pc; do
  name=$(basename "$pc")
  name=${name%-uninstalled.pc}.pc
  ln -s "$pc" "$pc_dir/$name"
done
shopt -u nullglob

# --- NDK toolchain -----------------------------------------------------------
# `cc` is the versioned NDK clang (aarch64-linux-android31-clang), so it already
# knows its sysroot and target. llvm-ar/ranlib live next to it.
toolchain_bin=$(dirname "$cc")
ar=$toolchain_bin/llvm-ar
[ -x "$ar" ] || {
  echo "error: llvm-ar not found next to the NDK clang at $ar" >&2
  exit 1
}

# --- cargo -------------------------------------------------------------------
cd "$repo_root"

# env(1) is needed because cargo/cc target variables contain dashes.
env \
  "CC_$rust_target=$cc" \
  "CXX_$rust_target=$cxx" \
  "AR_$rust_target=$ar" \
  "CARGO_TARGET_$(echo "$rust_target" | tr 'a-z-' 'A-Z_')_LINKER=$cc" \
  PKG_CONFIG_ALLOW_CROSS=1 \
  PKG_CONFIG_LIBDIR="$pc_dir" \
  PKG_CONFIG_PATH="$pc_dir" \
  "$cargo" rustc \
  --locked \
  --manifest-path "$repo_root/Cargo.toml" \
  --target-dir "$build_root/cargo-target" \
  --target "$rust_target" \
  -p rnote \
  --bin rnote \
  "${profile_args[@]}" \
  -- \
  -C link-arg=-shared \
  -C link-arg=-Wl,--export-dynamic \
  -C link-arg=-Wl,-z,max-page-size=16384 \
  -C link-arg=-llog

install -m 0755 "$build_root/cargo-target/$rust_target/$profile/rnote" "$output"

# GTK's RuntimeApplication resolves `main` with dlsym(). Fail the build rather
# than crash on device if a linker change drops the export.
llvm_nm=$toolchain_bin/llvm-nm
if [ -x "$llvm_nm" ]; then
  # grep without -q: with pipefail, -q exits early and SIGPIPEs llvm-nm.
  "$llvm_nm" -D "$output" | grep -E ' [TW] main$' >/dev/null ||
    {
      echo "error: $output does not export main for GTK's Android entrypoint" >&2
      exit 1
    }
fi

# The debug profile carries ~500 MB of DWARF per ABI, and Gradle cannot strip
# this library ("Unable to strip ... packaging them as they are"). Strip the
# packaged copy; the unstripped binary stays in cargo-target for symbolizing.
"$toolchain_bin/llvm-strip" --strip-unneeded "$output"
