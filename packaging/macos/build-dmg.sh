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
# A linker-signed Mach-O is not a valid signature for a complete app bundle.
# Seal the bundle after all its contents have been copied.
identity="${MACOS_SIGNING_IDENTITY:--}"
profile="${MACOS_NOTARY_PROFILE:-}"
if [[ -n "$profile" && "$identity" == "-" ]]; then
  echo "error: notarization requires MACOS_SIGNING_IDENTITY (Developer ID Application)" >&2
  exit 1
fi
if [[ "$identity" == "-" ]]; then
  codesign --force --sign - "$app"
else
  codesign --force --sign "$identity" --options runtime --timestamp "$app"
fi
codesign --verify --deep --strict --verbose=2 "$app"
ln -s /Applications "$stage/Applications"
dmg="$output/Super-Download-Manager-macOS-arm64.dmg"
hdiutil create -quiet -volname "Super Download Manager" -srcfolder "$stage" -ov -format UDZO "$dmg"
if [[ "$identity" != "-" ]]; then
  codesign --force --sign "$identity" --timestamp "$dmg"
  codesign --verify --verbose=2 "$dmg"
fi
if [[ -n "$profile" ]]; then
  xcrun notarytool submit "$dmg" --keychain-profile "$profile" --wait
  xcrun stapler staple "$dmg"
  xcrun stapler validate "$dmg"
else
  echo "Note: this disk image is not notarized and Gatekeeper may block it." >&2
fi
