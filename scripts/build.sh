#!/bin/bash
set -euo pipefail
project_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$project_root"
command -v cargo >/dev/null || { echo 'Rust не найден. Установите Rust или добавьте cargo в PATH.' >&2; exit 1; }
build_dir="${VELA_BUILD_DIR:-$project_root/build}"
cargo_target="${CARGO_TARGET_DIR:-$project_root/core/target}"
mkdir -p "$build_dir/cache"
export MACOSX_DEPLOYMENT_TARGET=14.0
cargo build --locked --release --manifest-path core/Cargo.toml
staging_dir="$(mktemp -d "${TMPDIR:-/tmp}/vela-build.XXXXXX")"
trap 'rm -rf "$staging_dir"' EXIT
app_path="$staging_dir/Vela.app"
mkdir -p "$app_path/Contents/MacOS" "$app_path/Contents/Resources"
cp "$cargo_target/release/vela-core" "$app_path/Contents/MacOS/vela-core"
cp macos/Info.plist "$app_path/Contents/Info.plist"
bash scripts/build_icon.sh "$build_dir/AppIcon.icns"
cp "$build_dir/AppIcon.icns" "$app_path/Contents/Resources/AppIcon.icns"
swiftc -parse-as-library -O -swift-version 6 -target "$(uname -m)-apple-macos14.0" -module-cache-path "$build_dir/cache" \
  macos/VelaApp.swift -o "$app_path/Contents/MacOS/Vela" \
  -framework SwiftUI -framework AppKit -framework UniformTypeIdentifiers
cp LICENSE "$app_path/Contents/Resources/LICENSE"
cp THIRD_PARTY.md "$app_path/Contents/Resources/THIRD_PARTY.md"
mkdir -p "$app_path/Contents/Resources/ThirdParty"
for component in vendor/*; do
  component_name="$(basename "$component")"
  mkdir -p "$app_path/Contents/Resources/ThirdParty/$component_name"
  for notice in "$component"/LICENSE* "$component"/COPYING* "$component"/NOTICE*; do
    if [ -f "$notice" ]; then cp "$notice" "$app_path/Contents/Resources/ThirdParty/$component_name/"; fi
  done
done
for attribute in com.apple.FinderInfo com.apple.ResourceFork; do
  xattr -dr "$attribute" "$app_path" 2>/dev/null || true
done
codesign --force --sign - "$app_path/Contents/MacOS/vela-core"
# Seal in a temporary directory: File Provider can attach Finder metadata to
# application bundles in Documents while codesign is running.
for attribute in com.apple.FinderInfo com.apple.ResourceFork; do
  xattr -dr "$attribute" "$app_path" 2>/dev/null || true
done
codesign --force --sign - "$app_path"
codesign --verify --deep --strict "$app_path"
ditto -c -k --keepParent --norsrc --noextattr "$app_path" "$build_dir/Vela-preview.zip"
# Vela.app is a generated build product; persistent libraries live beside it.
rm -rf "$project_root/Vela.app"
ditto --norsrc --noextattr "$app_path" "$project_root/Vela.app"
echo "Готово: $project_root/Vela.app"
echo "Проверенный архив: $build_dir/Vela-preview.zip"
