#!/usr/bin/env bash
set -euo pipefail

prefix="${HOME}/.local"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --prefix) prefix="$2"; shift 2 ;;
    -h|--help)
      echo "Usage: scripts/uninstall-linux.sh [--prefix PATH]"
      exit 0
      ;;
    *) echo "Unknown arg: $1" >&2; exit 2 ;;
  esac
done

bindir="${prefix}/bin"
appdir="${XDG_DATA_HOME:-${HOME}/.local/share}/applications"
icondir="${XDG_DATA_HOME:-${HOME}/.local/share}/icons/hicolor/scalable/apps"

rm -f "${bindir}/gitgpui-app"
rm -f "${appdir}/gitgpui.desktop"
rm -f "${icondir}/gitgpui.svg"

command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$appdir" >/dev/null 2>&1 || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache "${XDG_DATA_HOME:-${HOME}/.local/share}/icons/hicolor" >/dev/null 2>&1 || true

echo "Uninstalled GitGpui desktop integration from ${prefix} and ~/.local/share."

