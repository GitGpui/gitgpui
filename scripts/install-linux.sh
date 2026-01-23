#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/install-linux.sh [--release|--debug] [--prefix PATH] [--no-build]

Installs:
  - binary to <prefix>/bin/gitgpui-app
  - desktop entry to ~/.local/share/applications/gitgpui.desktop
  - icon to ~/.local/share/icons/hicolor/scalable/apps/gitgpui.svg

Defaults:
  --release, --prefix ~/.local, build if needed
EOF
}

mode="release"
prefix="${HOME}/.local"
build=1

while [[ $# -gt 0 ]]; do
  case "$1" in
    --release) mode="release"; shift ;;
    --debug) mode="debug"; shift ;;
    --prefix) prefix="$2"; shift 2 ;;
    --no-build) build=0; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown arg: $1" >&2; usage; exit 2 ;;
  esac
done

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
bin_src="${repo_root}/target/${mode}/gitgpui-app"

if [[ $build -eq 1 && ! -x "$bin_src" ]]; then
  (cd "$repo_root" && cargo build -p gitgpui-app --${mode})
fi

if [[ ! -x "$bin_src" ]]; then
  echo "Binary not found or not executable: $bin_src" >&2
  echo "Build first or omit --no-build." >&2
  exit 1
fi

bindir="${prefix}/bin"
appdir="${XDG_DATA_HOME:-${HOME}/.local/share}/applications"
icondir="${XDG_DATA_HOME:-${HOME}/.local/share}/icons/hicolor/scalable/apps"

install -Dm755 "$bin_src" "${bindir}/gitgpui-app"

# Install desktop file with absolute Exec path so it works even if ~/.local/bin isn't on PATH.
tmp_desktop="$(mktemp)"
trap 'rm -f "$tmp_desktop"' EXIT
sed "s|^Exec=.*$|Exec=${bindir}/gitgpui-app|g" \
  "${repo_root}/assets/linux/gitgpui.desktop" >"$tmp_desktop"
install -Dm644 "$tmp_desktop" "${appdir}/gitgpui.desktop"

install -Dm644 "${repo_root}/assets/gitgpui_logo.svg" \
  "${icondir}/gitgpui.svg"

command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$appdir" >/dev/null 2>&1 || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache "${XDG_DATA_HOME:-${HOME}/.local/share}/icons/hicolor" >/dev/null 2>&1 || true

echo "Installed GitGpui:"
echo "  ${bindir}/gitgpui-app"
echo "  ${appdir}/gitgpui.desktop"
echo "  ${icondir}/gitgpui.svg"
echo "If GNOME still shows a generic icon, log out/in (or restart GNOME Shell)."
