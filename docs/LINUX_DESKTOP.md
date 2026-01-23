# Linux desktop integration (GNOME/Wayland)

GitGpui sets `app_id` to `gitgpui` so GNOME can associate the running window with a desktop entry.

To make GNOME show the correct app name/icon, run:

```sh
./scripts/install-linux.sh
```

Manual install (what the script does):

```sh
install -Dm644 assets/linux/gitgpui.desktop \
  ~/.local/share/applications/gitgpui.desktop

install -Dm644 assets/gitgpui_logo.svg \
  ~/.local/share/icons/hicolor/scalable/apps/gitgpui.svg

update-desktop-database ~/.local/share/applications >/dev/null 2>&1 || true
gtk-update-icon-cache ~/.local/share/icons/hicolor >/dev/null 2>&1 || true
```

Then restart GNOME Shell (or log out/in).
