# Omarss

A fast, keyboard-friendly desktop RSS/Atom reader for Linux and Windows. `SPEC.md` is the source of truth.

**Status:** M2 (local feeds). You can subscribe to feeds and websites, organise them into folders, and read, star and refresh them. Keyboard shortcuts, OPML, search, full text, notifications and sync follow in later milestones.

## Download

Every merge to `main` publishes installers on the [Releases page](https://github.com/ChrisWaldenDev/omarss/releases).

### Linux

- **AppImage** (`.AppImage`, works on most distros): download it, then make it executable and run it:

  ```sh
  chmod +x Omarss_*_amd64.AppImage
  ./Omarss_*_amd64.AppImage
  ```

  To integrate it with your desktop launcher, move it somewhere permanent (e.g. `~/.local/bin/`) and create a `.desktop` file at `~/.local/share/applications/omarss.desktop`:

  ```ini
  [Desktop Entry]
  Name=Omarss
  Exec=/home/you/.local/bin/Omarss.AppImage
  Icon=/home/you/.local/share/omarss/icon.png
  Type=Application
  Categories=Network;News;
  ```

  Then run `update-desktop-database ~/.local/share/applications` (if available) so it shows up in app launchers.

- **`.deb`** (Debian, Ubuntu and derivatives): `sudo apt install ./Omarss_*_amd64.deb`
- **`.rpm`** (Fedora, openSUSE and derivatives): `sudo dnf install ./Omarss-*.x86_64.rpm` (or `rpm -i` / `zypper install`)

### Windows

- **`-setup.exe`**: run it and follow the installer.
- **`.msi`**: double-click it, or run `msiexec /i Omarss_*_x64_en-US.msi` from a terminal.

The Windows installers are not code-signed yet, so SmartScreen may warn the first time you run them.

## Development

Prerequisites: stable Rust, Node.js 22.12 or newer with npm, and the [Tauri system dependencies](https://v2.tauri.app/start/prerequisites/) (on Linux, `webkit2gtk-4.1` and friends).

```sh
npm install
npm run tauri dev      # run the app with hot reload
npm run tauri build    # release build and installers
```

Checks (all run in CI):

```sh
npm test                 # Vitest
npm run check            # svelte-check (types)
npm run lint             # ESLint
npm run format:check     # Prettier
cd src-tauri && cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check
```

`src/lib/types.ts` is generated from the Rust command and type definitions. Never edit it by hand; it is rewritten on every debug launch, or run `npm run gen:bindings`.

## Where data lives

| | Linux | Windows |
|---|---|---|
| Database, favicons, backups, logs | `~/.local/share/omarss/` (honours `XDG_DATA_HOME`) | `%LOCALAPPDATA%\omarss\` |

## Troubleshooting

- **Blank or black window on Linux** (WebKitGTK with some GPU drivers): start the app with `WEBKIT_DISABLE_DMABUF_RENDERER=1`.
