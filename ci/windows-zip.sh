#!/usr/bin/env bash
# Portable package for the Windows-native single-session GUI.
set -euo pipefail

TARGET_DIR=${1:-target}
TAG_NAME=${TAG_NAME:-$(git -c core.abbrev=8 show -s --format=%cd --date=format:%Y%m%d-%H%M%S)}

zipdir="WezTerm-windows-${TAG_NAME}"
if [[ "${BUILD_REASON:-}" == "Schedule" ]]; then
  zipname="WezTerm-windows-nightly.zip"
else
  zipname="${zipdir}.zip"
fi

rm -rf "$zipdir" "$zipname"
mkdir -p "$zipdir/mesa"

release_dir="$TARGET_DIR/release"
cp "$release_dir/wezterm-gui.exe" \
  assets/windows/conhost/conpty.dll \
  assets/windows/conhost/OpenConsole.exe \
  assets/windows/angle/libEGL.dll \
  assets/windows/angle/libGLESv2.dll \
  "$zipdir/"
mkdir -p "$zipdir/mesa"
# Keep Mesa in its subdirectory; WGL searches this directory explicitly.
cp assets/windows/mesa/opengl32.dll "$zipdir/mesa/"

# PDBs are optional diagnostics and are not part of the portable runtime.
if command -v 7z >/dev/null 2>&1; then
  7z a -tzip "$zipname" "$zipdir"
elif command -v zip >/dev/null 2>&1; then
  zip -qr "$zipname" "$zipdir"
else
  echo "windows-zip.sh requires 7z or zip" >&2
  exit 1
fi
