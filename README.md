# Omarss

A fast, keyboard-friendly desktop RSS/Atom reader for Linux and Windows. `SPEC.md` is the source of truth.

**Status:** M1 (skeleton). The UI shows built-in demo data; real feeds arrive in M2.

## Download

Every merge to `main` publishes installers on the [Releases page](https://github.com/ChrisWaldenDev/omarss/releases): `.AppImage`, `.deb` and `.rpm` for Linux, and `-setup.exe` or `.msi` for Windows. The Windows installers are not code-signed yet, so SmartScreen may warn the first time you run them.

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
| Database, backups, logs | `~/.local/share/omarss/` (honours `XDG_DATA_HOME`) | `%LOCALAPPDATA%\omarss\` |

## Troubleshooting

- **Blank or black window on Linux** (WebKitGTK with some GPU drivers): start the app with `WEBKIT_DISABLE_DMABUF_RENDERER=1`.
