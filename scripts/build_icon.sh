#!/bin/bash
set -euo pipefail
project_root="$(cd "$(dirname "$0")/.." && pwd)"
output="${1:-$project_root/build/AppIcon.icns}"
source_image="$project_root/macos/Assets/AppIcon.png"
mkdir -p "$(dirname "$output")"
staging_dir="$(mktemp -d "${TMPDIR:-/tmp}/vela-icon.XXXXXX")"
trap 'rm -rf "$staging_dir"' EXIT
iconset="$staging_dir/AppIcon.iconset"
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" "$source_image" --out "$iconset/icon_${size}x${size}.png" >/dev/null
  retina_size=$((size * 2))
  sips -z "$retina_size" "$retina_size" "$source_image" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$output"
echo "Иконка: $output"
