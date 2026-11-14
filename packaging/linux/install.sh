#!/bin/sh
# Installs Super Download Manager from an extracted release tarball.
# Usage: ./install.sh [--prefix DIR]    (default: ~/.local, no root needed)
set -eu

prefix="${HOME:?HOME is not set}/.local"
if [ "${1:-}" = "--prefix" ]; then
  [ $# -eq 2 ] || { echo "usage: $0 [--prefix DIR]" >&2; exit 2; }
  prefix="$2"
elif [ $# -ne 0 ]; then
  echo "usage: $0 [--prefix DIR]" >&2; exit 2
fi
# The prefix is written into the .desktop Exec line, so keep it to safe characters.
case "$prefix" in
  /*) ;;
  *) echo "error: prefix must be an absolute path" >&2; exit 2 ;;
esac
case "$prefix" in
  *[!A-Za-z0-9_./+-]*) echo "error: prefix may only contain letters, digits and _ . / + -" >&2; exit 2 ;;
esac

here="$(cd "$(dirname "$0")" && pwd)"
dest="${DESTDIR:-}$prefix"
for f in bin/super-download-manager share/applications/super-download-manager.desktop share/icons/hicolor/scalable/apps/super-download-manager.svg; do
  [ -f "$here/$f" ] || { echo "error: $f missing; run this from the extracted tarball" >&2; exit 1; }
done

install -d "$dest/bin" "$dest/share/applications" "$dest/share/icons/hicolor/scalable/apps"
install -m 755 "$here/bin/super-download-manager" "$dest/bin/super-download-manager"
install -m 644 "$here/share/icons/hicolor/scalable/apps/super-download-manager.svg" "$dest/share/icons/hicolor/scalable/apps/"
# Use an absolute Exec path so the launcher works even if $prefix/bin is not on PATH.
sed "s|^Exec=.*|Exec=$prefix/bin/super-download-manager|" "$here/share/applications/super-download-manager.desktop" \
  > "$dest/share/applications/super-download-manager.desktop"
chmod 644 "$dest/share/applications/super-download-manager.desktop"

# Best effort: refresh caches so the launcher shows up right away.
if [ -z "${DESTDIR:-}" ]; then
  command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$dest/share/applications" >/dev/null 2>&1 || true
  command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q -t "$dest/share/icons/hicolor" >/dev/null 2>&1 || true
fi

echo "Installed to $prefix. Run: $prefix/bin/super-download-manager (or use the application menu)."
case ":$PATH:" in *":$prefix/bin:"*) ;; *) echo "Note: $prefix/bin is not on your PATH." ;; esac
