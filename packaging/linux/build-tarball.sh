#!/bin/sh
# Packages a release binary into packaging/linux/output/super-download-manager-<ver>-linux-x86_64.tar.gz
# Usage: packaging/linux/build-tarball.sh [path/to/download-manager-ui]
# Build the binary first (on a Linux x86_64 host, ideally the oldest glibc you support):
#   cargo build --release --locked --manifest-path frontend/Cargo.toml
set -eu

root="$(cd "$(dirname "$0")/../.." && pwd)"
src="$root/packaging/linux"
binary="${1:-$root/frontend/target/release/download-manager-ui}"
[ -f "$binary" ] && [ -x "$binary" ] || { echo "error: $binary not found/executable; build it first" >&2; exit 1; }

# Refuse anything that is not a 64-bit little-endian x86-64 ELF (e.g. a macOS build).
magic="$(head -c 20 "$binary" | od -An -tx1 | tr -d ' \n')"
case "$magic" in
  7f454c46020101*3e00) ;;
  *) echo "error: $binary is not a Linux x86_64 ELF executable" >&2; exit 1 ;;
esac

version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/frontend/Cargo.toml" | head -n1)"
[ -n "$version" ] || { echo "error: cannot read version from frontend/Cargo.toml" >&2; exit 1; }
name="super-download-manager-$version-linux-x86_64"
out="$src/output"

stage="$(mktemp -d "${TMPDIR:-/tmp}/sdm-linux.XXXXXX")"
trap 'rm -rf "$stage"' EXIT
d="$stage/$name"
mkdir -p "$d/bin" "$d/share/applications" "$d/share/icons/hicolor/scalable/apps" "$out"
install -m 755 "$binary" "$d/bin/super-download-manager"
install -m 644 "$src/super-download-manager.desktop" "$d/share/applications/"
install -m 644 "$src/super-download-manager.svg" "$d/share/icons/hicolor/scalable/apps/"
install -m 755 "$src/install.sh" "$src/uninstall.sh" "$d/"
install -m 644 "$src/INSTALL.txt" "$root/LICENSE" "$d/"

if tar --version 2>/dev/null | grep -q GNU; then own="--owner=0 --group=0 --numeric-owner"; else own="--uid 0 --gid 0"; fi
# shellcheck disable=SC2086
tar -C "$stage" $own -czf "$out/$name.tar.gz" "$name"
(cd "$out" && { sha256sum "$name.tar.gz" 2>/dev/null || shasum -a 256 "$name.tar.gz"; } > "$name.tar.gz.sha256")
echo "$out/$name.tar.gz"
