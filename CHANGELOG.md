# Changelog

## 0.3.0: M3 Daily-driver polish (unreleased)

- **Keyboard shortcuts.** Every §6.7 shortcut except `t`, `f` and `/` (tags, full text and search arrive in M4/M5): `j`/`k`, `n`/`p`, `J`/`K`, `o`/`Enter`, `v`, `m`, `s`, `A`, `r`/`R`, `g a`/`g u`/`g s`, `Space`/`Shift+Space`, `Ctrl+,`, `?` and `Esc`. `?` shows a cheatsheet, and every shortcut can be rebound in Settings → Keyboard (with clash detection). Shortcuts are off while typing.
- **Mark all as read** for the current view, optionally only articles older than a day or a week, from the list header or `Shift+A`, with a 10-second **Undo**.
- **OPML import and export.** Nested outlines become folders (deeper levels are flattened as `Parent / Child`), duplicates and invalid addresses are skipped and counted, and custom titles round-trip. Imported feeds are fetched straight away with the usual refresh progress.
- **Retention.** Articles older than 90 days (configurable: 30/90/180/365 days or forever) are deleted daily and after refreshes that bring new articles. Starred and tagged articles and each feed's newest 50 are always kept. The planner is tuned with `PRAGMA optimize`, and the database is compacted weekly when idle.
- **Image proxy and cache.** Remote images load through `omarss-img://` without cookies or referrer, are cached on disk (500 MB by default, least recently used evicted first) for offline reading, and known trackers are refused. "Load remote images" can be Always (with list thumbnails), Only in opened articles, or Never (click to load).
- **List thumbnails** from Media RSS or an article's first image.
- **Sanitisation hardening.** YouTube and Vimeo embeds become click-to-load players (`youtube-nocookie.com`). 1×1 tracking pixels and a built-in list of tracker hosts are removed, and `utm_*` parameters are stripped from links (setting, on by default).
- **Broken feeds** view listing feeds that keep failing, with their last error, **Try again** and **Edit address**. Feed addresses are now editable, and feeds can override the User-Agent (advanced).
- **Settings** (General, Reading, Refresh, Appearance, Keyboard, Privacy & network, Storage, About): two-pane layout, mark-as-read on open / after a delay / on scroll, reader font, size, line width and spacing, accent colour, interface scale (80–150 %), `custom.css`, manual HTTP/SOCKS5 proxy, pause refresh on metered connections (Windows), storage usage with "Clear image cache" and "Compact database". Sidebar and list widths can be dragged and are remembered.
- Share menu in the reader (copy link, copy as Markdown, email), and right-click menus on articles.

## 0.2.0: M2 Local feeds (unreleased)

- **Subscribe to real feeds.** Paste a feed or website address: Omarss discovers the site's feeds (`<link rel="alternate">`, then common paths), lets you pick one if there are several, and previews the title and latest five articles before you choose a name and folder.
- **Fetching and parsing.** RSS 0.9x/1.0/2.0, Atom and JSON Feed, with character-set detection, repair of common XML breakage, lenient dates, relative-URL resolution, enclosures, and GUID fallbacks. Article HTML is sanitised when stored.
- **Background refresh.** Every 30 minutes by default (or per feed), on start-up, and on demand: all feeds, a folder or one feed. Conditional GET, permanent-redirect tracking, 410 auto-pause, `Retry-After`, publisher hints, exponential backoff, at most 8 fetches at once and 2 per host, and no counting errors while offline.
- **Reading.** Real article list with paging and a virtualised view, a reader with star, read/unread and open-in-browser, mark-as-read on open, and unread/starred/today counts.
- **Organising.** Folders (create, rename, delete), drag-and-drop ordering, and an Edit feed dialog (name, folder, refresh interval, pause). Unsubscribing keeps starred articles under "Deleted feeds".
- **Favicons** fetched and cached as 32×32 icons, with a letter avatar as fallback. Feeds that keep failing show a warning with the last error.
- The demo data from M1 is gone.

## Since 0.1.0 (released as 0.1.x builds)

- Renamed to **Omarss** in the window title, UI, installers and release names, matching the other oma apps. Commands, packages and data folders keep the lowercase `omarss`, so existing installs upgrade in place and keep their data.
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
