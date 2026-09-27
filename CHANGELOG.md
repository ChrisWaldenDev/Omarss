# Changelog

## Unreleased

- Every merge to main publishes a numbered GitHub Release (e.g. `v0.1.7`) with Linux (`.AppImage`, `.deb`, `.rpm`) and Windows (`-setup.exe`, `.msi`) installers, after CI passes.
- Windows installers embed the WebView2 bootstrapper; packages carry a category, description, licence and homepage.
- Fixed `cargo test` crashing on Windows (`STATUS_ENTRYPOINT_NOT_FOUND`).
- The startup log reports the bundle version.

## 0.1.0: M1 Skeleton (2026-09-27)

- Tauri 2 app with a Svelte 5 + TypeScript + Vite frontend, targeting Linux and Windows.
- Embedded SQLite (rusqlite, bundled, WAL, FTS5) with numbered migrations embedded in the binary. The schema version is kept in `PRAGMA user_version`, each migration runs in a transaction, and existing databases are backed up before migrating. Migration `0001_init` creates the full §5 schema and the Local account.
- Typed IPC: TypeScript types and command bindings are generated from Rust with tauri-specta. Backend errors reach the UI as `{ kind, message }`.
- Settings stored in the `settings` table (currently the theme).
- Three-pane layout (sidebar, article list, reader) showing built-in demo data: smart views (All, Unread, Starred, Today) with counts, collapsible folders, a warning icon on failing feeds, an unread/all filter, and sort order.
- Light, dark and system themes, persisted across restarts, with a matching native window theme.
- Links open in the default browser; the WebView can't navigate away from the app, and no plugin APIs are exposed to it.
- Daily-rotated log files (7 kept) in the data directory.
- CI (GitHub Actions): format, lint, type check and frontend tests; Rust fmt, clippy, tests and a release build on Ubuntu 22.04 and Windows.
- Placeholder app icon (`src-tauri/icons/icon.svg`, rendered with `tauri icon`).
