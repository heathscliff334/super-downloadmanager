#!/bin/bash
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
binary="$root/frontend/target/release/download-manager-ui"
output="$root/packaging/macos/output"
if [[ ! -x "$binary" ]]; then
  echo "Build the release binary first: cargo build --release --manifest-path frontend/Cargo.toml" >&2
  exit 1
fi
stage="$(mktemp -d "${TMPDIR:-/tmp}/super-download-manager.XXXXXX")"
trap 'rm -rf "$stage"' EXIT
app="$stage/Super Download Manager.app"
mkdir -p "$app/Contents/MacOS" "$output"
cp "$binary" "$app/Contents/MacOS/SuperDownloadManager"
cp "$root/packaging/macos/Info.plist" "$app/Contents/Info.plist"
ln -s /Applications "$stage/Applications"
hdiutil create -quiet -volname "Super Download Manager" -srcfolder "$stage" -ov -format UDZO "$output/Super-Download-Manager-macOS-arm64.dmg"
