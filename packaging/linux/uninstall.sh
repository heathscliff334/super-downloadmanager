#!/bin/sh
# Removes exactly the three files install.sh installed. Usage: ./uninstall.sh [--prefix DIR]
set -eu
prefix="${HOME:?HOME is not set}/.local"
if [ "${1:-}" = "--prefix" ]; then
  [ $# -eq 2 ] || { echo "usage: $0 [--prefix DIR]" >&2; exit 2; }
  prefix="$2"
elif [ $# -ne 0 ]; then
  echo "usage: $0 [--prefix DIR]" >&2; exit 2
fi
case "$prefix" in /*) ;; *) echo "error: prefix must be an absolute path" >&2; exit 2 ;; esac
rm -f "$prefix/bin/super-download-manager" \
  "$prefix/share/applications/super-download-manager.desktop" \
  "$prefix/share/icons/hicolor/scalable/apps/super-download-manager.svg"
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$prefix/share/applications" >/dev/null 2>&1 || true
echo "Removed Super Download Manager from $prefix. Downloaded files were not touched."
