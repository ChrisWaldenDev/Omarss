# Omarss: Port to iced (Linux only)

> Status: draft for owner review, written 2026-09-30 from the repo at `95f86ae` (M3 merged, 0.3.0).
> Audience: the coding agent doing the port, and the owner reviewing it.
> This document is a **change spec on top of `SPEC.md`**. Where they disagree, this document wins
> for the port; phase P0 below folds the changes into `SPEC.md` itself so there is one source of
> truth again.

---

## How to use this document

Paste this to start a session:

> Read `CLAUDE.md`, `SPEC.md`, `docs/DECISIONS.md` and `docs/ICED_PORT_SPEC.md` in full.
> We're porting Omarss from Tauri + Svelte to iced and dropping Windows. Do phase **P\<n\>** of
> `docs/ICED_PORT_SPEC.md` §15 only, then stop and summarise as `CLAUDE.md` says.

One phase per session, one PR per phase, same as milestones. The phases are ordered so that
**every PR merged to `main` still ships a working app** (every merge publishes a release).

Before P0, the owner should answer the questions in §14. Each has a default. If the owner hasn't
answered, use the default and log it in `docs/DECISIONS.md`.

---

## 1. Goal

1. **Linux only.** Remove Windows support: code, CI, packaging, docs and spec. macOS stays a
   non-goal and still shouldn't be broken on purpose.
2. **Pure Rust.** Replace the Tauri shell, the WebView and the Svelte/TypeScript frontend with
   an [iced](https://github.com/iced-rs/iced) GUI. No Node, npm, Vite, WebKitGTK, or generated
   TypeScript bindings are left in the repo.
3. **No change for users' data.** Same data and config folders, same SQLite file, same schema
   (no migration needed for the port), same `settings` rows, same shortcut overrides. An existing
   0.3 install upgrades in place and keeps everything.
4. **Feature parity with M1–M3** (the checklist in §13) before the Tauri app is deleted.
   M4 onwards are then built on iced.

### 1.1 Non-goals of the port

- No new features. The regressions iced forces on us are listed in §14 for the owner to accept.
- No schema or settings-format redesign beyond the removals in §10.
- No attempt to keep the Windows code "just in case".

---

## 2. What exists today (inventory)

Read this before touching anything. Line counts are approximate.

### 2.1 Backend (`src-tauri/src`, ~11.4k lines incl. tests): mostly keep

| Area | Files | Tauri-coupled? | Fate |
|---|---|---|---|
| Storage | `store/*` (pool, migrations, repos), `migrations/*.sql` | `tauri::async_runtime` in `store/mod.rs` only | **Keep**, swap runtime calls for tokio |
| Feed parsing | `feed/*`, `opml.rs` | No | **Keep as is** |
| Fetching | `fetch/*` (HTTP client, discovery, favicons) | No | **Keep as is** |
| Content | `content.rs` (ammonia sanitiser, excerpts, hashes), `content/rewrite.rs` (embeds, trackers, display rewriting) | No | **Keep** ingest side; display side changes (§8) |
| Services | `services/{feeds,articles,images,maintenance,opml,settings}.rs` | `tauri::async_runtime::spawn*`, `tauri::Theme` | **Keep**, minor edits |
| Scheduler | `scheduler.rs` | `AppHandle<R>` + `tauri_specta::Event::emit` | **Keep logic**, replace emitting with the event channel (§5.2) |
| Models / errors | `models.rs`, `error.rs` | `specta::Type` derives, `From<tauri::Error>` | **Keep types**, drop specta/tauri bits |
| Events | `events.rs` | tauri-specta events | **Replace** with a plain enum (§5.2) |
| Commands | `commands/*.rs` (29 commands) | Entirely | **Replace** with a `Backend` facade (§5.1) |
| Glue | `lib.rs`, `main.rs`, `bindings.rs`, `protocol.rs` (`omarss-img://`), `navigation.rs` (WebView guard), `paths.rs` (Tauri path resolver) | Entirely | **Delete or rewrite** (§5, §9) |
| Windows-only | `network.rs` (metered detection via WinRT), `windows-app-manifest.xml`, `build.rs` manifest embedding, `windows` crate | n/a | **Delete** (§10) |
| Tests | `tests/*.rs` + 38 fixture feeds, ~133 tests | `tests/scheduler.rs` uses `tauri::test::mock_app` | **Keep all**; scheduler tests read the event channel instead |

### 2.2 Frontend (`src/`, ~8.1k lines): rewrite in Rust

| Area | Files | What it does |
|---|---|---|
| Shell | `App.svelte`, `main.ts`, `app.css` | Grid layout (3-pane / 2-pane), splitters, global keydown, loading/error screens, CSS theme tokens (light + dark) |
| State | `stores/articles.svelte.ts` | View, filter, sort, keyset-paged list, selection cursor, open article, stale-response dropping, optimistic read/star with rollback, mark-read modes (on open / after delay / on scroll), mark-all + undo, "show new articles" |
| | `stores/sidebar.svelte.ts` | Sidebar data, collapsed folders, debounced reload |
| | `stores/refresh.svelte.ts` | Refresh progress/offline/last-finished |
| | `stores/settings.svelte.ts` | Optimistic settings update with rollback |
| | `stores/dialogs.svelte.ts`, `stores/toasts.svelte.ts`, `stores/clock.svelte.ts` | Single modal, toasts with actions + timeouts, minute clock |
| | `stores/app.svelte.ts`, `startup.ts` | Event wiring; subscribe before first load |
| Logic | `keyboard.ts` | Action table, default bindings, overrides, conflict detection, key-event → binding string, two-key sequences with 1.5 s timeout |
| | `actions.ts` | What each action/menu item does; errors → toasts |
| | `views.ts`, `sidebarOrder.ts`, `virtual.ts`, `format.ts`, `share.ts`, `embeds.ts`, `theme.ts`, `readerScroll.ts`, `addFeed.svelte.ts`, `shortcutGroups.ts` | Broken-feed rules, next-unread-feed, view titles, drag-and-drop reorder maths, list windowing maths, relative time / dates / sizes / letter avatars, Markdown & mailto links, click-to-load embeds and images, accent colour + contrast, reader scrolling, add-feed state machine |
| i18n | `i18n/en.json` (~290 keys), `i18n/index.ts` | `t(key, params)` with `{name}` placeholders |
| Components | `Sidebar`, `ArticleList`, `ReaderPane`, `VirtualList`, `Splitter`, `ContextMenu`, `Toasts`, `Dialog(s)`, `AddFeedDialog`, `EditFeedDialog`, `FolderDialog`, `ConfirmDialog`, `ShortcutsDialog`, `SettingsDialog` + 8 settings sections, `BrokenFeeds`, `FeedIcon`, `Icon`, `Keys`, `ThemeSwitcher` | The UI |
| Tests | 15 Vitest files, ~75 tests | Listed in §12 with where each goes |

### 2.3 Build, CI, release

- `package.json`/npm drive everything, including `tauri build`.
- `ci.yml`: docs-only detection; a frontend job (prettier, eslint, svelte-check, vitest); an app
  job on **ubuntu-22.04 and windows-latest** (fmt, clippy, test, stale-bindings check, `tauri build --no-bundle`).
- `release.yml`: version = `<major>.<minor>` from `src-tauri/tauri.conf.json` + a patch number
  counted by walking main's first-parent history (see `docs/DECISIONS.md`, last entry). Builds
  `.deb`/`.rpm`/`.AppImage` on Ubuntu 22.04 and NSIS/MSI on Windows with the Tauri bundler, then
  publishes a GitHub Release with `SHA256SUMS.txt`.
- `.claude/skills/run-omarss/` drives the app through the **WebKit remote inspector** (DOM
  clicks, `eval`, IPC `invoke`). None of that survives the port (§11.4).

---

## 3. Target architecture

### 3.1 Repository layout

A Cargo workspace at the repo root, two crates, no Node:

```
omarss/
├─ Cargo.toml                 # [workspace], shared [workspace.package] version = "0.4.0"
├─ Cargo.lock
├─ SPEC.md  CLAUDE.md  README.md  CHANGELOG.md  LICENSE
├─ docs/DECISIONS.md  docs/ICED_PORT_SPEC.md
├─ crates/
│  ├─ omarss-core/            # lib `omarss_core`: everything that isn't UI. No iced dependency.
│  │  ├─ Cargo.toml
│  │  ├─ migrations/          # moved from src-tauri/migrations
│  │  └─ src/
│  │     ├─ lib.rs            # pub mod …; pub use backend::Backend;
│  │     ├─ backend.rs        # the facade replacing commands/ (§5.1)
│  │     ├─ events.rs         # AppEvent enum + channel (§5.2)
│  │     ├─ paths.rs          # dirs-based (§9.1)
│  │     ├─ document.rs       # article content IR (§8)
│  │     ├─ store/ services/ fetch/ feed/ content/ opml.rs scheduler.rs models.rs error.rs clock.rs logging.rs
│  │     └─ tests/            # moved as is, fixtures included
│  └─ omarss/                 # bin `omarss`: the iced app
│     ├─ Cargo.toml           # [package.metadata.deb] / [package.metadata.generate-rpm]
│     ├─ assets/              # icon.svg + PNGs, UI icons (SVG), i18n/en.json, omarss.desktop, metainfo
│     └─ src/
│        ├─ main.rs           # runtime, logging, Backend::start, iced::application(...)
│        ├─ app.rs            # App state, Message, update, view, subscription
│        ├─ state/            # articles.rs sidebar.rs refresh.rs settings.rs dialogs.rs toasts.rs add_feed.rs
│        ├─ keyboard.rs       # port of keyboard.ts (§7.9)
│        ├─ actions.rs        # port of actions.ts
│        ├─ view/             # sidebar.rs list.rs reader.rs broken_feeds.rs dialogs/ settings/ toasts.rs menu.rs
│        ├─ widget/           # virtual_list.rs splitter.rs content.rs (IR → widgets) feed_icon.rs icon.rs
│        ├─ theme.rs          # tokens, light/dark palettes, accent, styles (§7.10)
│        ├─ i18n.rs  format.rs  share.rs  views.rs  sidebar_order.rs
│        └─ platform.rs       # open links, file dialogs, clipboard helpers (§9)
└─ packaging/
   ├─ appimage/               # AppImage recipe (§11.3)
   └─ arch/PKGBUILD           # SPEC §2 (can stay a stub until M8)
```

Why two crates: `omarss-core` builds and tests without iced/wgpu (fast CI, fast `cargo test -p
omarss-core`), and the crate boundary replaces the old IPC boundary, keeping "the UI never
touches the DB, disk or network directly" (SPEC §4.1) enforceable by the compiler: the UI crate
must not depend on `rusqlite` or `reqwest`.

During the transition (P2–P8) the Tauri app keeps living in `src-tauri/` as a **workspace member
that depends on `omarss-core`**, so `main` stays releasable. The iced crate's binary is called
`omarss-next` until the switch-over (P9), when it becomes `omarss` and `src-tauri/`, `src/`, and
every Node file are deleted.

### 3.2 Dependencies

`omarss-core` keeps today's dependencies minus `tauri*`, `specta*`, `tauri-specta`,
`specta-typescript`, `windows`, and adds:

| Crate | For |
|---|---|
| `tokio` (add `fs` only if needed) | Already there; now also used directly for `spawn`/`spawn_blocking` |
| `dirs` (or `directories`) | Base dirs replacing Tauri's path resolver (§9.1) |

`omarss` (the app):

| Crate | For | Notes |
|---|---|---|
| `iced` **0.14** | GUI | Features: `tokio`, `image`, `svg`, `advanced` (custom widgets), `wgpu` + `tiny-skia` (defaults; tiny-skia is the software fallback). Pin the minor version. If 0.15 is out when you start, read its changelog and record in DECISIONS which one you pick and why. |
| `rfd` | OPML open/save dialogs | `default-features = false`, `features = ["xdg-portal", "tokio"]`. Same reasoning as today's DECISIONS entry: the GTK dialog doesn't open under Hyprland, the portal does. |
| `open` | Open links in the default browser / mail client | What `tauri-plugin-opener` used underneath. Use the detached variant so it never blocks. |
| `omarss-core` | Everything else | |
| `serde_json` | Loading `en.json` | |
| `tracing` | Logging | Setup stays in core's `logging.rs` |

Clipboard: iced's built-in `clipboard::write` task. Don't add `arboard` unless that fails on
Wayland.

Check every API named in this document against the pinned iced version's docs before relying
on it. iced renames things between minors. Names here are from 0.13/0.14 and may be off.

---

## 4. What stays exactly the same

- The SQLite schema, migrations, `PRAGMA` setup, pool size, pre-migration backups, the
  "newer schema refused" rule.
- Every service's behaviour and every DECISIONS entry about fetching, parsing, dedupe, retention,
  scheduling, offline detection, favicons, image cache limits and LRU, OPML rules, settings
  storage (one row per field, camelCase keys, `state.` prefix, fall back per field).
- File locations (§9.1): `~/.local/share/omarss/` (DB `omarss.sqlite`, `icons/`, `images/`,
  `backups/`, `logs/`) and `~/.config/omarss/`, honouring `XDG_*`.
- The log setup (daily, 7 kept). The default filter changes from `warn,omarss_lib=info` to
  `warn,omarss_core=info,omarss=info` because the crate names change.
- All `en.json` strings (moved, not rewritten), all default shortcuts and the shortcut override
  format in the `shortcuts` setting.
- Sanitising at ingest, and storing only sanitised HTML. The stored HTML stays the canonical
  content so M4's FTS backfill and M5's extraction work the same.

---

## 5. Core changes

### 5.1 `Backend` facade replaces `commands/`

Create `omarss_core::Backend`, a cheap-to-clone struct holding today's managed state (`Store`,
`SettingsService`, `HttpClient`, `FeedService`, `ArticleService`, `ImageCache`,
`MaintenanceService`, `OpmlService`, `Scheduler`, `AppPaths`, version string).

- `Backend::start(version: &str) -> AppResult<(Backend, EventReceiver)>` does what the Tauri
  `setup` closure does today, in the same order: resolve paths, init logging, open the store,
  load settings, build the HTTP client (falling back to no proxy if the saved one is invalid),
  build the services, start the scheduler and maintenance. It must be called from inside a tokio
  runtime.
- **One async method per current command**, same names, same arguments (minus `State`/`AppHandle`),
  same return types, and **the same side effects**. The side effects are the part that's easy to
  lose:
  - `subscribe_feed`: `scheduler.reschedule()` and emit `ArticlesChanged { new_count: 0 }`.
  - `update_feed`: `scheduler.reschedule()`.
  - `update_settings`: validate and apply the proxy **before** saving and roll it back if saving
    fails; after saving, apply the image-cache limit and reschedule if the interval changed.
    Theme and UI scale are applied by the UI from the returned `Settings`, not here.
  - `mark_all_read` / `undo_mark_all_read`: unchanged (token held in memory).
  - `compact_database`: `cleanup()` then `compact()`.
- File-picker commands split in two: the **UI** opens the dialog (§9.2) and then calls
  `backend.import_opml_file(path) -> AppResult<ImportSummary>` (keeps the 20 MB limit and
  reschedules when feeds were added) or `backend.export_opml_file(path) -> AppResult<()>`.
- `get_app_info` returns version, platform, homepage, data and config dirs. `metered_supported`
  goes (§10).
- `get_custom_css` is replaced according to the answer to Q4 in §14 (default:
  `load_theme_overrides()`, §7.10).
- `open_external` moves to the UI's `platform.rs` (§9.3) but keeps the scheme allowlist
  (`http`, `https`, `mailto`) as a pure function in core, with its existing test.
- New: `backend.load_image(url) -> Option<CachedImage>` (wraps `ImageCache::load`) and
  `backend.icon_path(file_name) -> Option<PathBuf>`, which keeps `protocol.rs`'s
  `is_icon_file_name` check so nothing outside `icons/` can be read.
- New: `backend.article_document(id) -> AppResult<ArticleView>` (§8).

By the end of the port (P9), delete `commands/`, `bindings.rs`, `protocol.rs`,
`navigation.rs`'s Tauri plugin, and every `specta::Type` derive and `#[specta::specta]`
attribute. Until then the derives live behind a `specta` cargo feature of core that only the
transitional Tauri shell enables (P2). Keep the `serde` derives: settings are stored as JSON and
tests use them.

### 5.2 Events without Tauri

```rust
pub enum AppEvent {
    RefreshProgress { done: u32, total: u32 },
    RefreshDone { new_count: u32, errors: u32, offline: bool },
    ArticlesChanged { new_count: u32 },
    FeedError { feed_id: i64, message: String },
}
```

- An `EventSender` (a `tokio::sync::mpsc::UnboundedSender<AppEvent>` wrapper whose `send`
  ignores a closed channel) is created in `Backend::start` **before** the scheduler starts, and
  the receiver is returned to the caller. That replaces `startup.ts`'s "listen, then load" rule:
  events are buffered until the UI reads them, so none are lost during start-up.
- `Scheduler::start`, `run_batch` and `finish` take an `EventSender` instead of `AppHandle<R>`.
  They lose their `R: Runtime` generics.
- `tests/scheduler.rs`: replace `mock_app()` with a channel whose receiver the test drains, and
  assert on the collected `AppEvent`s. Every existing assertion must still hold. Don't delete or
  weaken any.

### 5.3 Runtime

- Replace `tauri::async_runtime::spawn` → `tokio::spawn`, `spawn_blocking` →
  `tokio::task::spawn_blocking`, `block_on` → run inside the runtime (the two `block_on`s are in
  `store/mod.rs` and `lib.rs` setup). `store/mod.rs`'s `block_on` inside `Store::open` should
  become a plain synchronous check on a connection, since `open` already runs at start-up.
- `main.rs` builds one multi-thread tokio runtime, calls `Backend::start` inside it, keeps it
  alive for the whole process, and enables iced's `tokio` feature so `Task::perform` futures also
  run in a tokio context (reqwest needs one). If iced's own executor and the app runtime conflict,
  run `Backend::start` via `iced`'s boot task instead and record the choice.

### 5.4 Errors and models

- `AppError` keeps `{ kind, message }` and its `Display`. Drop `From<tauri::Error>`.
- `models.rs` keeps all types. Changes: `ArticleListItem.thumbnail` becomes the **original**
  image URL (no proxy URL), and `Article` loses `content_html` / `images_blocked` in favour of the
  IR (§8). `FeedNode.icon` stays a file name.
- `services/settings.rs`: delete `Theme::to_window_theme()`; the UI maps `Theme` to iced itself.

---

## 6. UI architecture (iced)

### 6.1 Shape

The Elm architecture: one `App` struct, one `Message` enum, `update(&mut self, Message) ->
Task<Message>`, `view(&self) -> Element<Message>`, `subscription(&self) -> Subscription<Message>`,
plus `theme(&self)` and `scale_factor(&self)`. Built with `iced::application(...)` and
`window::Settings { size: 1280×800, min_size: 720×480, icon, platform_specific: { application_id:
"omarss" } }`. The `application_id` (Wayland app id / X11 class) must be `omarss` so the desktop
file's `StartupWMClass=omarss` matches, as today.

`App` owns: `backend: Backend`, `phase: Loading | Failed(String) | Ready`, and the ports of each
store: `ArticlesState`, `SidebarState`, `RefreshState`, `Option<Settings>`, `Option<Dialog>`,
`Toasts`, `Option<ContextMenu>`, `AddFeedFlow` (inside the dialog), `KeyDispatcher`, `now`
(clock), image handles (§8.4), drag state, splitter drag state.

### 6.2 Porting the stores

Each Svelte store becomes a plain Rust struct in `state/` with the same fields and methods.
Async calls become `Task::perform(backend.clone().method(args), Message::X)`; the result comes
back as a message and is applied by a method on the state struct. Rules to keep:

- **Stale responses are dropped.** Keep the `#listRequest` / `#articleRequest` counters: every
  request message carries the generation it was sent with, and the handler ignores it if the
  counter has moved on. Same for `hasNewArticles` being cleared when the request *starts*.
- **Optimistic updates roll back.** `set_read`, `toggle_star`, `markScrolledPast`, sidebar
  reorder and settings updates patch local state first and keep what they need to undo it; the
  failure message restores it and shows a toast.
- **Mark-read-after-delay** uses a `Task` that sleeps and then sends `MarkReadTimerFired { id,
  generation }`; opening another article or closing bumps the generation, which cancels it.
- **Debounced sidebar reload** (`scheduleReload(delay)`) keeps its coalescing: store a reload
  generation, schedule a sleep task, and only reload if no newer request arrived.
- To keep the store tests portable (§12), put state transitions in methods that take plain
  values (`apply_page(generation, result)`, `patch(id, changes) -> Undo`, …) so tests can call
  them without running async code. Keep `update` in `app.rs` as thin routing.

### 6.3 Subscriptions

- Backend events: a `Subscription::run` (or the 0.14 equivalent) stream over the `EventReceiver`
  from `Backend::start` (take it out of an `Arc<Mutex<Option<_>>>` once). Map each `AppEvent`
  exactly as `stores/app.svelte.ts` does (progress → refresh state, reload the sidebar every 10
  feeds with a 1 s debounce; done → reload now; articles changed → reload and
  `notify_new_articles`; feed error → debounced reload).
- Clock: `iced::time::every(60 s)` → `Message::Tick`.
- Keyboard and mouse: see §7.9 and §7.7. Only subscribe to mouse movement while a splitter or
  drag-and-drop is in progress.
- Toast expiry and the other timers are one-off tasks, not subscriptions.

---

## 7. Feature-by-feature port

Match today's behaviour. When in doubt, run the Tauri app (P2–P8 keep it working) and compare.

### 7.1 Layout

- Three-pane: sidebar | splitter | list | splitter | reader. Two-pane: sidebar | splitter |
  (list, or the reader with a Back button when an article is open). `Esc` in two-pane closes the
  reader.
- Widths come from `sidebarWidth` / `listWidth`, capped as today (sidebar ≤ 30 % of the window,
  list ≤ 40 %; 40 % for the sidebar in two-pane). Splitter limits: sidebar 180–480, list 240–720.
  Track the window size (resize events) to apply the caps.
- The Broken feeds page replaces list + reader.
- Loading screen until the first load finishes; a centred error with `app.startError` if it
  fails.

### 7.2 Sidebar

- Header: app menu button (Import OPML, Export OPML, Keyboard shortcuts, Settings), add-feed
  button, refresh-all button.
- Smart views All / Unread / Starred / Today with counts; folders (collapsible, unread sums);
  root feeds; "Deleted feeds" when present; "Broken feeds" entry with a count when any feed is
  broken (`views::is_broken`).
- Each feed row: favicon or letter avatar (§7.6), title, unread count, warning icon when
  `error_count ≥ 3`, with the last error as a tooltip (`iced::widget::tooltip`).
- Footer: refresh status text (refreshing N/M, offline, "updated X ago"), theme switcher.
- Right-click menus: feed (Refresh, Edit, Open site, Unsubscribe…), folder (Refresh, Rename,
  Delete folder…). Same confirmations and follow-up reloads as `Sidebar.svelte`.
- Empty state with an "Add a feed" button when there are no feeds.
- Drag and drop reordering: §7.7.

### 7.3 Article list

- Header: view title (or "Deleted feeds"), unread/all toggle (hidden in Unread and Starred),
  sort toggle, mark-all-read button with its menu (all / older than a day / older than a week).
- **Virtualised**, fixed 100 px rows (§7.8). Row: favicon + feed name, relative time, title,
  ~140-char excerpt, star, unread styling, optional thumbnail (§8.4).
- Infinite scroll: load the next page (100) when within a screen of the end.
- "Show new articles" button when `has_new_articles`; loading, error and "all caught up" states.
- Scroll to top on view/filter/sort change; keep the selected row visible when moving with keys
  (`scroll_to_reveal`).
- Mark-as-read-on-scroll: rows entirely above the viewport are batched and flushed after 400 ms.
- Row right-click menu: Open, Mark read/unread, Star/Unstar, Open original, Copy link.

### 7.4 Reader

- Toolbar: back (two-pane), mark read/unread, star, open original, share menu (Copy link, Copy
  as Markdown, Email link).
- Header: title, feed, author, full date; then the content (§8), then enclosures (link, MIME
  type, size via `format::file_size`).
- Typography from settings: font (System / Sans / Serif → iced `Family::SansSerif`,
  `Family::SansSerif`, `Family::Serif`; see Q7), size in px, line height as a relative line
  height, line width in characters converted to a max width (`font_size × 0.55 × chars` is a fine
  start; tune by eye).
- Scrolls to the top on each new article. `Space` / `Shift+Space` scroll 85 % of a screen
  (min 40 px); at the end, `Space` opens the next article. Needs the scrollable's viewport,
  captured through its scroll callback.
- "Load images" button when images wait for a click (§8.4).
- Error state when the article failed to load.

### 7.5 Dialogs and overlays

One modal at a time, drawn with a `stack` over a dimmed, click-to-close backdrop (iced's modal
example is the pattern). `Esc` closes. While a modal or menu is open, shortcuts are off.

| Dialog | Port of | Notes |
|---|---|---|
| Add feed | `AddFeedDialog.svelte` + `addFeed.svelte.ts` | Same state machine: address → discovering → choose (radio list) → previewing → preview (title, latest 5, name, folder pick list incl. "New folder…" + name input) → subscribing. Errors return to the failing step. Enter submits. |
| Edit feed | `EditFeedDialog.svelte` | Address, name, folder, refresh interval (default / §6.5 options / manual), paused, advanced User-Agent, last fetched, last error, Unsubscribe. `focusUrl` focuses the address input (from Broken feeds' "Edit address"). |
| Folder | `FolderDialog.svelte` | Create / rename. |
| Confirm | `ConfirmDialog.svelte` | Title, message, danger confirm button; shows the error if `on_confirm` fails. The closure becomes a `ConfirmAction` enum (`Unsubscribe(feed_id)`, `DeleteFolder(folder_id)`) handled in `update`. |
| Shortcuts | `ShortcutsDialog.svelte` | Groups from `shortcutGroups.ts`, current bindings rendered as key caps. |
| Settings | `SettingsDialog.svelte` + `settings/*` | §7.11 |

Toasts (bottom of the window, stacked): error vs info styling, optional action button ("Undo"),
close button, expiry (6 s default, 10 s for mark-all undo, 12 s for OPML import, 2.5 s for
"Copied").

Context menus: an overlay positioned at the cursor (iced 0.14's `pin`, or an `absolute`-style
container in a `stack`), closed by clicking outside or `Esc`, with Up/Down/Enter keyboard
support while open. Menu items that open a dialog close the menu first.

### 7.6 Feed icons

`FeedNode.icon` is a file name in `icons/`; load it with `image::Handle::from_path(backend.
icon_path(name))`, cached in a `HashMap<String, Handle>` in `App`. Without an icon, draw the
letter avatar with the same letter and colour as `format.ts`'s `letterAvatar` (port the hash
exactly: `hash = hash * 31 + code_point` in wrapping `u32` over the title's chars, colour =
`AVATAR_COLORS[hash % 8]`).

### 7.7 Sidebar drag and drop

iced has no built-in drag and drop between widgets. Implement it in the app:

- Wrap each feed and folder row in a `mouse_area`. A press records a candidate; once the cursor
  has moved more than 4 px with the button held, it's a drag.
- While dragging, rows report hover (`on_enter`, `on_move` with the position inside the row)
  so the app knows the target and whether it's the top or bottom half, exactly as
  `Sidebar.svelte`'s `overFeed` / `overFolder` / `overRoot`. Draw the same before/after/into
  indicator.
- On release, compute the new order with the ported `sidebar_order::move_feed` /
  `move_folder`, apply it optimistically, call `reorder_sidebar`, roll back and toast on failure.
- `Esc` cancels a drag.

### 7.8 Virtual list

`VirtualList.svelte` + `virtual.ts` port to `widget/virtual_list.rs`: a `scrollable` whose
content is `Space(height = start × ROW)` + the visible rows + `Space(remaining)`, driven by the
viewport from the scrollable's `on_scroll`. Port `visible_range` and `scroll_to_reveal` as pure
functions with their tests. Scroll programmatically with the scrollable operation for its `Id`.
Must stay smooth at 100k rows (SPEC §6.2): only visible rows plus overscan are ever built.

### 7.9 Keyboard

- Port `keyboard.rs` from `keyboard.ts` one to one: `ACTIONS` with the same ids and default keys
  (`t`, `f`, `/` stay unbound until M4/M5), `resolve_bindings`, `overrides_from`, `bound_to`,
  `conflicts_with`, `key_parts`, and `KeyDispatcher` with the 1500 ms sequence timeout and an
  injectable clock.
- **Binding strings must stay identical** (`"j"`, `"J"`, `"?"`, `"Enter"`, `"Space"`,
  `"Shift+Space"`, `"Ctrl+,"`, `"g a"`), so users' saved overrides keep working. Write
  `key_to_binding(key: &iced::keyboard::Key, modifiers)` to reproduce `eventToKey`: use the
  key's produced character when there is one (so Shift+j gives `"J"` and Shift+/ gives `"?"`),
  `Space` for space, named keys by their DOM names (`Enter`, `Escape`, `ArrowDown`, `Tab`, …),
  `Ctrl`/`Alt`/`Meta` prefixes, `Shift` only for named keys, `None` for lone modifiers. Unit-test
  it against the same cases as the Vitest suite.
- Listen to key presses through an event subscription that also gets the event **status**, and
  only treat presses the widgets **ignored** as shortcuts. That's how "shortcuts are off while
  typing" (SPEC §6.7) works: a focused `text_input` captures the key. Also skip them while a
  dialog or menu is open, as today.
- `Esc` handling: close menu → close dialog → cancel drag → close the reader in two-pane →
  reset a pending sequence, in that order.
- `Enter`/`Space` exceptions for focused buttons don't apply (iced buttons aren't keyboard
  focusable); `Space` always scrolls the reader.
- In dialogs, `Tab` / `Shift+Tab` move between text inputs (`focus_next` / `focus_previous`
  operations) and `Enter` submits.
- Rebinding UI (Settings → Keyboard): "Change" enters capture mode; the next key (and, for a
  sequence, a second key within the timeout, shown as "then…") becomes the binding; conflicts are
  refused with the message naming the other action; per-action Reset and Reset all. Capture mode
  swallows the keys so they don't trigger actions.

### 7.10 Theme and appearance

- Port every CSS custom property in `src/app.css` (light block and dark block) into a `Tokens`
  struct with two constructors, `Tokens::light()` and `Tokens::dark()`, with the same colour
  values: `bg`, `bg_sidebar`, `bg_list`, `bg_input`, `bg_hover`, `bg_selected`, `bg_code`, `text`,
  `text_read`, `text_muted`, `border`, `border_subtle`, `accent`, `accent_text`,
  `accent_contrast`, `focus_ring`, `warning`, `danger`, `star`, plus `radius` (6 px) and shadow.
  Keep WCAG AA contrast (SPEC §12).
- Build the iced `Theme` with `Theme::custom` from the tokens, and write style functions for
  buttons, rows (hover, selected, unread), inputs, scrollables, containers, menus, toasts and
  dialogs that read the tokens. Pass `&Tokens` to views; don't hard-code colours in views.
- Theme setting Light / Dark / System. System follows the OS colour scheme: use iced's
  system-theme support if the pinned version has it, otherwise read the XDG desktop portal's
  `org.freedesktop.appearance color-scheme` (e.g. via the `ashpd` crate) and listen for changes.
  Record which.
- Accent colour: port `contrastText` and the overrides (`accent`, `accent_text`, `focus_ring`,
  `accent_contrast`, and `bg_selected` = 18 % accent mixed into `bg`; write a small sRGB `mix`).
  iced has no colour picker: offer the preset swatches plus a `#rrggbb` text input, validated
  like the backend does.
- UI scale 80–150 % → the application's `scale_factor`. Changes apply live.
- Reduced motion: the port has no animations (smooth scrolling included), so there's nothing
  to reduce. Keep it that way.
- `custom.css` has no equivalent. See Q4: default is a `~/.config/omarss/theme.toml` with the
  same token names (per theme: `[light]`, `[dark]`, `#rrggbb` values), read at start-up and by a
  "Reload theme file" button in Appearance, at most 64 KB, unknown keys and bad values ignored
  with a log warning.

### 7.11 Settings dialog

Sections and controls, matching the current components (keep their i18n keys):

| Section | Controls |
|---|---|
| General | Layout (three-pane / two-pane), mark as read (on open / after N s / on scroll) + delay 1–60, mark updated articles unread, Import OPML / Export OPML buttons |
| Reading | Reader font, font size 12–32, line width 40–120 ch, line height 120–220 %, list thumbnails, a live text preview |
| Refresh | Interval (15 min, 30 min, 1 h, 2 h, 6 h, manual), refresh on start-up. **Pause on metered is removed** (§10). |
| Appearance | Theme, accent (default / swatches / custom hex), interface scale, theme file reload (Q4), sidebar and list width sliders (keyboard alternative to dragging the splitters; replaces the splitters' arrow-key support) |
| Keyboard | §7.9 |
| Privacy & network | Load remote images (Always / Only in opened articles / Never), strip `utm_*`, proxy URL + Apply with the backend's validation message |
| Storage | Database size, image cache size, article count, data folder; retention (30/90/180/365/forever); image cache limit; Clear image cache; Compact database (busy state while running) |
| About | Logo, version, tagline, homepage (opens in browser), licence, data and config folders, the privacy statement |

Settings save through the optimistic `SettingsState::save(patch)` with rollback and a
`settings.saveError` toast on failure, like today.

### 7.12 Strings, dates and numbers

- Move `src/lib/i18n/en.json` to `crates/omarss/assets/i18n/en.json` unchanged; load it once
  with `include_str!` + `serde_json` into a `LazyLock<HashMap<String, String>>`.
  `t(key, &[("name", value)])` fills `{name}` placeholders, leaves unknown ones visible, and
  formats numbers with thousands separators.
- A unit test walks `crates/omarss/src/**/*.rs`, collects every `t("…")` literal, and fails if
  a key is missing from `en.json`. A second test checks there are no empty strings. Delete keys
  that are no longer used after P9 (and the metered ones).
- `format.rs`: port `relative_time` (English output matching today's `Intl` "short, numeric:
  auto" style: "now", "5 min. ago", "3 hr. ago", "yesterday", "3 days ago", then "Sep 3" /
  "Sep 3, 2025"), `full_date` (local time, "Sep 30, 2026, 3:04 PM"), `file_size` (decimal units:
  bytes spelled out, then kB/MB/GB with one decimal under 10). Put the words in `en.json`.
  SPEC §12 asks for locale formatting "ready for translation"; English-only formatting is fine
  for v1. Log the decision.

---

## 8. Article content rendering (the hard part)

The WebView rendered sanitised HTML. iced has no HTML engine, so content is turned into a small
document model in core and drawn with native widgets. This removes the whole class of script
injection bugs (nothing is ever executed) and replaces CSP, the navigation guard, the
`omarss-img://` protocol and the sandboxed iframes.

### 8.1 Decision: own IR, not a third-party HTML widget

Build `omarss_core::document` ourselves: parse the **stored, already-sanitised** HTML with
`scraper`/html5ever (already a dependency) into a typed IR, then render the IR in the UI crate.
Reasons: images must go through our cache with our click-to-load rule; embeds need our
placeholder handling; the IR is pure and unit-testable in core without a GPU; and we control
the supported subset exactly. Before starting P4, spend at most an hour checking `frostmark`
(an html5ever-based HTML viewer for iced) against these needs. Use it only if it clearly covers
§8.2–§8.5 and allows custom image loading, and record the decision either way.

### 8.2 The IR

```rust
pub struct Document { pub blocks: Vec<Block> }

pub enum Block {
    Paragraph(Vec<Inline>),
    Heading { level: u8, content: Vec<Inline> },
    List { ordered: bool, start: u32, items: Vec<Vec<Block>> },
    Quote(Vec<Block>),
    Code { text: String },                          // <pre>, whitespace preserved
    Image(ImageRef),
    Figure { content: Vec<Block>, caption: Vec<Inline> },
    Table { head: Vec<Vec<Cell>>, body: Vec<Vec<Cell>> },
    Rule,                                           // <hr>
    Details { summary: Vec<Inline>, content: Vec<Block> },
    Embed { provider: EmbedProvider, watch_url: String, label: String }, // omarss-embed placeholder
    Media { kind: MediaKind, src: String, poster: Option<ImageRef> },     // <video>/<audio>
}

pub enum Inline {
    Text { text: String, style: TextStyle },        // bold, italic, underline, strike, code, sub, sup, mark
    Link { href: String, content: Vec<Inline> },
    Image(ImageRef),                                // small inline images become blocks when rendered
    LineBreak,
}

pub struct ImageRef { pub url: String, pub alt: String, pub width: Option<u32>, pub height: Option<u32> }
pub struct Cell { pub content: Vec<Inline>, pub header: bool, pub colspan: u16 }
```

Mapping rules (write each as a test):

- Every tag in `content.rs`'s `ALLOWED_TAGS` maps to something. Structural wrappers (`div`,
  `section`, `article`, `main`, `header`, `footer`, `aside`, `nav`, `hgroup`, `address`,
  `center`, `span`, `picture`, `time`, `data`, `abbr`, `cite`, `dfn`, `bdi`, `bdo`, `q` with its
  quotes, `ruby` with its `rp`/`rt` flattened) are unwrapped into their content.
- Whitespace collapses like HTML outside `<pre>`. Text directly inside a container becomes a
  paragraph. Empty paragraphs are dropped.
- `img`: pick `src`, else the best `srcset` candidate (largest width ≤ 1600 px, else the
  first). Drop images the rewrite rules would (trackers, 1×1, no source). `data:image/*` sources
  are decoded directly, never fetched.
- Links: `a[href]` with `utm_*` stripped when `strip_tracking_params` is on
  (`rewrite::strip_tracking_params`). Only `http`, `https` and `mailto` are clickable; other
  schemes render as plain text.
- `figure.omarss-embed[data-embed]` → `Embed` (validate with the same rule as `embeds.ts`'s
  `isAllowedEmbed`), shown as a card (§8.5).
- Tables: keep up to 20 columns; `colspan` respected, `rowspan` ignored (renders in the first
  row). Wider tables fall back to one paragraph per row.
- `details`/`summary`: collapsible in the UI, closed unless `open`.
- Depth limit (e.g. 64): deeper content is flattened into text. Size limit: stop building after
  a few thousand blocks and add a "Content truncated, open the original" notice.

`backend.article_document(id)` returns an `ArticleView` (today's `Article` fields plus
`document: Document` and `images_blocked: bool`), built in `spawn_blocking`. Build it from
`content_html` exactly as `services/articles.rs` picks it today; M5's full text will reuse this.

### 8.3 Rendering the IR

`widget/content.rs` turns `Document` into widgets:

- Paragraphs, headings, list items, cells and captions use `rich_text` with spans: bold/italic
  via font weight/style, code via monospace family + `bg_code`, links via span links that send
  `Message::OpenLink(href)`, coloured with `accent_text` and underlined. Heading sizes relative to
  the reader font size (h1 1.6×, h2 1.4×, h3 1.2×, h4–h6 1.0× bold).
- Lists use a row of (marker, content column), nested with indentation; ordered lists honour
  `start`.
- Quotes: left border in `border`, indented, `text_muted`.
- Code blocks: monospace, `bg_code`, horizontally scrollable, no wrapping.
- Rule: a 1 px `rule`.
- Everything sits in a column capped at the reader line width and centred like today.

### 8.4 Images (replaces the `omarss-img://` proxy)

- The UI never fetches images itself. It asks `backend.load_image(url)`, which is today's
  `ImageCache::load`: same cache files, same size limit and LRU, tracker refusal, 10-minute
  failure memory, at most 6 downloads at once, no cookies or Referer. Existing cache files stay
  valid (they're keyed by the URL's SHA-256, not by the proxy URL).
- `App` keeps `images: HashMap<String, ImageSlot>` where `ImageSlot` is `Loading`,
  `Ready(image::Handle)` or `Failed`. Opening an article requests all its images at once (the
  cache limits concurrency). `Handle::from_bytes` builds the handle. Evict handles for articles
  no longer shown so memory stays bounded.
- While loading, reserve the space from `width`/`height` when known (scaled to the column),
  so text doesn't jump. Failed images are hidden, as today.
- Big images are scaled down to the column width, never up. Animated GIFs show their first
  frame (Q8).
- "Load remote images": **Always** loads reader images and list thumbnails; **Only in opened
  articles** loads reader images but no thumbnails; **Never** shows a "Load images" button that
  loads them for the current article only. The rule lives in core (`images_blocked`, thumbnails
  only when `list_thumbnails && load_images == Always`, as in `services/articles.rs` today).
- List thumbnails: `ArticleListItem.thumbnail` is the original URL; the list requests visible
  rows' thumbnails through the same cache and draws them at their fixed size.

### 8.5 Video embeds and media

No web engine means no in-app players. `Embed` renders as a card: a play icon, "YouTube video"
/ "Vimeo video", and the host; clicking opens the video's page (`watch_url`) in the browser. This
settles the "FLAG: YouTube refuses to play without a Referer" entry in DECISIONS, and the
navigation guard's embed-host allowlist goes away. `video`/`audio` render as a similar card
linking to the source (with the poster image if there is one). See Q3.

### 8.6 Links

Clicking a link sends `OpenLink(url)`; the UI checks the allowlist and opens it with the `open`
crate (§9.3). Nothing ever navigates inside the app. Hovering a link shows its address in a
tooltip or status line so users can see where it goes (a WebView did that for free).

### 8.7 Security tests

Keep every sanitiser test. Add IR tests that run the XSS fixture (`xss_feed.xml`) and the
existing sanitiser corpus through ingest + `Document` building and assert: no link with a
scheme other than http/https/mailto, no image source other than http/https/`data:image/*`, no
leftover `script`/`style` text. That covers SPEC §15's "An XSS test corpus rendered in the
reader executes no script".

---

## 9. Platform integration (Linux)

### 9.1 Paths

`paths.rs` resolves `dirs::data_local_dir()` and `dirs::config_dir()` + `omarss`. On Linux these
honour `XDG_DATA_HOME` / `XDG_CONFIG_HOME` and give exactly today's `~/.local/share/omarss/` and
`~/.config/omarss/`. Keep `AppPaths` and its test. If `dirs` returns `None`, fail start-up with
a clear error.

### 9.2 File dialogs

`rfd::AsyncFileDialog` with the xdg-portal backend, OPML filter (`opml`, `xml`), titles
"Import subscriptions" / "Export subscriptions", default name `omarss-subscriptions.opml`,
parented to the main window if rfd/iced make that easy. Cancel → nothing happens, as today.

### 9.3 Opening links

`platform::open_external(url)`: parse, check the core allowlist, then `open::that_detached`.
Used by the reader, "Open original", "Open site", the About homepage, and "Email link"
(`mailto:`).

### 9.4 Clipboard

"Copy link" / "Copy as Markdown" use iced's clipboard write task, then the "Copied" toast.
Port `share.rs` (`markdown_link`, `mailto_link`) with its tests.

### 9.5 Window and desktop integration

- App id / WM class `omarss`, title "Omarss", window icon from `assets/icon.png`.
- The desktop file becomes `omarss.desktop` (lowercase; Tauri forced `Omarss.desktop`, see
  DECISIONS), `Name=Omarss`, `Exec=omarss %U`, `Icon=omarss`, `StartupWMClass=omarss`,
  `Categories=Network;News;`. Hicolor icons at 32, 128, 256 and scalable.
- Renderer: wgpu with iced's automatic tiny-skia fallback. Document in the README that
  `ICED_BACKEND=tiny-skia` forces software rendering if a GPU driver misbehaves (check the exact
  variable for the pinned version). This replaces the `WEBKIT_DISABLE_DMABUF_RENDERER`
  troubleshooting note and SPEC §12's WebKitGTK paragraph.
- Wayland (incl. Hyprland) and X11 both via winit. Test both (SPEC §12).
- Window size/position persistence, tray, notifications, single instance and `feed://` stay in
  M6. Note for M6: `ksni` or `tray-icon` for the tray, `notify-rust` for notifications, a Unix
  socket in `$XDG_RUNTIME_DIR` for single instance, and `MimeType=x-scheme-handler/feed;…` in
  the desktop file. Don't build them now.

---

## 10. Dropping Windows

Delete:

- `src-tauri/windows-app-manifest.xml`; the MSVC manifest linker args in `build.rs` (the whole
  `build.rs` goes with Tauri in P9).
- `src-tauri/src/network.rs` and the `[target.'cfg(windows)'.dependencies] windows` crate. The
  scheduler's metered check and `METERED_RECHECK` go with it.
- `Settings.pause_on_metered` and its validation, UI, i18n keys, and `AppInfo.metered_supported`.
  A stored `pauseOnMetered` row is simply ignored (unknown keys are, per DECISIONS). Delete the
  row in the next migration that touches settings, not now.
- `main.rs`'s `windows_subsystem` attribute.
- `tauri.conf.json`'s `bundle.windows` block (P1; the file itself goes in P9).
- CI's `windows-latest` matrix entry; release.yml's Windows build, `nsis,msi` bundles, the
  SmartScreen note, and `.exe`/`.msi` collection.
- README: the Windows download section, the Windows column of "Where data lives", Node/Tauri
  prerequisites (P9).
- Platform branches in comments and docs ("`http://omarss-img.localhost` on Windows",
  "`tauri.localhost`", close-to-tray "on on Windows", portable mode, WebView2).

Keep `.gitattributes` (LF endings are still good hygiene) but drop the Windows justification
from its comment.

In `SPEC.md` (done in P0): §0 and `CLAUDE.md` "keep Linux and Windows both working" → Linux
only; §1 and §1.2 (Windows out, macOS still a non-goal); §2 table (Windows row gone; ARM64 line
Linux only); §3 stack table (§3 rewrite below); §6.6 close-to-tray default off; §6.8 custom CSS
per Q4; §7.2 metered paragraph removed (Linux NetworkManager support MAY stay as a post-v1
proposal); §8.2/§8.4 per §8 here; §11.1 Windows paths and portable mode removed; §11.2
unchanged; §11.4 capabilities and CSP replaced with "content is rendered natively from a
sanitised model; nothing from a feed is ever executed; the UI crate has no network or database
access"; §12 installer size and WebKitGTK notes; §13 frontend tests and CI matrix; §14 M1 and
M8 wording (packaging list without NSIS/MSI); §15 "Fresh install on Windows 11" and "Backup
exported on Linux restores on Windows" removed.

---

## 11. Build, CI, release, packaging

### 11.1 Local commands (replace README "Development" and CLAUDE.md "Before finishing")

```sh
cargo run -p omarss                     # the app (omarss-next until P9)
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

System packages for building on Debian/Ubuntu: find the minimum by trying on a clean
`ubuntu-22.04` runner; expect roughly `pkg-config libxkbcommon-dev libwayland-dev libx11-dev
libxcursor-dev libxi-dev libxrandr-dev libfontconfig1-dev libvulkan-dev`, and no GTK or
WebKit. Record the final list in the README and ci.yml.

### 11.2 CI (`ci.yml`)

- Keep the docs-only detection job as is.
- Remove the frontend job (P9; until then it keeps running for the Tauri app).
- App job: **ubuntu-22.04 only** (keeps the glibc 2.35 floor from SPEC §2), apt deps from §11.1,
  Rust stable with rustfmt + clippy, `Swatinem/rust-cache` on the workspace target dir,
  `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`, `cargo build --release -p omarss`, upload the binary as an artifact.
- Remove the stale-bindings check (there are no bindings).
- A headless smoke test (§12.3) runs as part of `cargo test`.

### 11.3 Release (`release.yml`)

- Same trigger, concurrency and publish logic; same version scheme.
- **Version source moves** from `src-tauri/tauri.conf.json` to the root `Cargo.toml`'s
  `[workspace.package] version`. The first-parent walk must read old commits too: for each
  commit, try the root `Cargo.toml` (`git show <c>:Cargo.toml` and read
  `workspace.package.version`, e.g. with a small `awk`/`grep` or `yq -p toml`), and fall back to
  `src-tauri/tauri.conf.json` via `jq` when it's missing. Keep the `v0.2` legacy-marker branch.
  Update the header comment and add a DECISIONS line.
- Build job: ubuntu-22.04 only. Build `cargo build --release -p omarss` with the release version
  injected (set the workspace version in `Cargo.toml` in the job before building, the way the
  Tauri job writes a temp config today), then:
  - `.deb` with `cargo-deb` (metadata in `crates/omarss/Cargo.toml`: binary, desktop file,
    icons, metainfo, `Depends` generated by `$auto`, section `net`).
  - `.rpm` with `cargo-generate-rpm` (same assets).
  - `.AppImage` with `appimagetool` (or `linuxdeploy`) from an AppDir holding the binary,
    desktop file and icon. Don't bundle GPU/graphics libraries (`libvulkan`, `libEGL`, `libGL`,
    `libwayland-*`, `libxkbcommon`); use the host's. The AppImage should now be a few MB,
    which resolves the "AppImage is about 100 MB" FLAG in DECISIONS.
  - `SHA256SUMS.txt` as today.
- **Package names must not change**, so `apt`/`dnf` upgrade 0.3 installs in place. Before
  writing the packaging, download the latest release's `.deb` and `.rpm` and read their package
  names (`dpkg-deb -f <deb> Package`, `rpm -qp --queryformat '%{NAME}' <rpm>`); use exactly
  those. The old `Omarss.desktop` is owned by the old package, so the upgrade removes it.
- Release notes text: Linux installers only.
- Asset file names: keep the current pattern (`Omarss_<ver>_amd64.deb`, `Omarss-<ver>-1.x86_64.rpm`,
  `Omarss_<ver>_amd64.AppImage`) so the README's install commands stay valid, or update the
  README in the same PR.

### 11.4 The `run-omarss` skill

The driver's WebKit-inspector approach can't work with iced. Rewrite the skill in P9:

- Keep: the fixtures server (it's plain HTTP; port it or keep the Node script, since a dev tool
  may use Node if the owner is fine with it, see Q6), isolated `XDG_DATA_HOME` /
  `XDG_CONFIG_HOME` under `/tmp/omarss-driver`, the hidden Hyprland special workspace launch
  (and its Lua-dispatch and PATH gotchas), `db`, `log`, `status`, `quit`.
- Screenshots: `grim` on the window's geometry from `hyprctl clients -j`.
- Driving the UI: the iced headless test harness (§12.3) is the supported way to script
  interactions; document how to write a quick scenario test. For poking a live window, document
  `ydotool`/`wtype` if available, but don't depend on it.
- Drop `invoke`, `eval`, `click`, `click-text`, `console` and the WebKit gotchas.

---

## 12. Testing

"Never delete or weaken a failing test" (CLAUDE.md) applies to the port: every existing test
moves or gets a Rust equivalent. Keep a table in the PRs saying where each went.

### 12.1 Backend tests (~133)

Move with the code. Only `tests/scheduler.rs`'s harness and the `commands`-level tests (if any
use `State`) change. `bindings.rs`'s test is deleted with the bindings (there's nothing left to
check). Add tests for `Backend`'s side effects (§5.1: subscribe emits `ArticlesChanged` and
reschedules; a bad proxy is never saved; OPML file import respects the size limit) and for the
IR (§8.2, §8.7).

### 12.2 Frontend tests (~75) → Rust unit tests in `crates/omarss`

| Vitest file | Rust home | Notes |
|---|---|---|
| `keyboard.test.ts` | `keyboard.rs` | Plus `key_to_binding` cases for iced keys. `isTypingTarget` has no equivalent (status-based capture replaces it); cover the "ignored status only" rule with a test on the event filter instead |
| `views.test.ts` | `views.rs` | |
| `sidebarOrder.test.ts` | `sidebar_order.rs` | |
| `virtual.test.ts` | `widget/virtual_list.rs` | |
| `format.test.ts` | `format.rs` | Same expected strings |
| `share.test.ts` | `share.rs` | `scrollReader` cases → reader scroll maths |
| `embeds.test.ts` | core `document.rs` | `isAllowedEmbed` |
| `theme.test.ts` | `theme.rs` | Accent overrides, `contrast_text`; `applyTheme` becomes "System follows the OS value" |
| `i18n/i18n.test.ts` | `i18n.rs` | Plus the "every used key exists" test |
| `stores/articles.test.ts` | `state/articles.rs` | All 16 cases, against the pure transition methods (§6.2) |
| `stores/settings.test.ts` | `state/settings.rs` | |
| `stores/toasts.test.ts` | `state/toasts.rs` | Expiry via explicit tick/expire messages |
| `addFeed.test.ts` | `state/add_feed.rs` | |
| `startup.test.ts` | core `events.rs` | "Events sent before the UI reads them aren't lost" |
| `api.test.ts` | — | Delete: there's no IPC wrapper. Its intent (errors keep their kind) is covered by `error.rs` tests |

### 12.3 Headless UI smoke test

SPEC §13's Playwright smoke test becomes an iced headless test (iced 0.14's `iced_test`
simulator, or whatever the pinned version provides) using a temp data dir and a wiremock feed:
start → add feed → open article → star → mark all read → undo → export OPML (to a temp path via
the backend call). If the headless API can't drive something (e.g. file dialogs), test that part
at the `update` level. Search joins it in M4.

---

## 13. Parity checklist (must all work in the iced app before P9 deletes Tauri)

From CHANGELOG M1–M3:

- [ ] Opens the existing 0.3 database and settings unchanged; theme, layout, widths, reader
      settings and shortcut overrides all apply.
- [ ] Three-pane and two-pane layouts; resizable, persisted sidebar/list widths.
- [ ] Smart views All/Unread/Starred/Today with counts; folders (collapse, unread sums); feeds
      with favicons or letter avatars; Deleted feeds; broken-feed warning icons and tooltip.
- [ ] Add feed: address or site URL, discovery, picker for several feeds, preview (title +
      latest 5), name, folder or new folder, subscribe, feed fetched straight away.
- [ ] Edit feed (address, name, folder, interval, paused, User-Agent), unsubscribe with
      confirmation, starred articles kept under Deleted feeds.
- [ ] Folders: create, rename, delete (feeds move to top level); drag-and-drop ordering of feeds
      and folders, into/out of folders.
- [ ] Article list: paging, virtualised smooth scrolling at 100k rows, unread/all, sort, relative
      times, excerpts, thumbnails, "Show new articles", empty/loading/error states.
- [ ] Reader: header, content (§8), enclosures, star, read/unread, open original, share menu,
      Load images, video cards, links open in the browser.
- [ ] Mark as read on open / after N s / on scroll; mark updated articles unread setting.
- [ ] Mark all read (all / >1 day / >1 week) with a 10 s Undo toast.
- [ ] Refresh all / folder / feed; progress, offline and "updated X ago" status; background
      refresh on schedule and at start-up.
- [ ] Every M3 shortcut (`j k n p J K o Enter v m s A r R g a g u g s Space Shift+Space Ctrl+,
      ? Esc`), two-key sequences, off while typing and in dialogs; cheatsheet; rebinding with
      conflict detection and reset.
- [ ] OPML import (summary toast) and export (path toast) via the portal dialogs.
- [ ] Broken feeds page: last error, Try again, Edit address.
- [ ] Settings: every control in §7.11, saved immediately, bad values rejected with the
      backend's message.
- [ ] Images through the cache (offline after first view), trackers removed, `utm_*` stripped,
      all three image modes.
- [ ] Light/dark/system theme, accent colour, UI scale 80–150 %, theme file (Q4).
- [ ] Right-click menus on feeds, folders, articles; app menu.
- [ ] Retention and weekly compaction still run; Storage page numbers and actions work.
- [ ] Works on Wayland (Hyprland, GNOME) and X11; software fallback when the GPU path fails.
- [ ] SPEC §12 targets re-measured: cold start, view switch, idle memory (expect far below
      250 MB), idle CPU ~0 %.

---

## 14. Questions for the owner (answer before P0; defaults in bold)

| # | Question | Default |
|---|---|---|
| Q1 | **Accessibility.** SPEC §12 requires screen-reader labels and ARIA roles. Check at the start of P5 whether the pinned iced exposes widgets to screen readers (AccessKit). If it doesn't, the port loses screen-reader support, and buttons can't be reached with Tab (keyboard use goes through the shortcuts instead). Accept? | **Accept for now.** Revise §12 to "full keyboard operation via shortcuts; screen-reader support when iced ships it", and log it as a known regression. |
| Q2 | **Text selection.** iced text can't be selected with the mouse (check the pinned version). Users can't select and copy a sentence from an article. Accept? | **Accept.** Copy link / Copy as Markdown stay. Revisit when iced supports selectable rich text. |
| Q3 | **Video embeds** become cards that open YouTube/Vimeo in the browser. `<video>`/`<audio>` become link cards too. OK? | **Yes.** (It also fixes the Linux "YouTube needs a Referer" bug in DECISIONS.) |
| Q4 | **`custom.css`** can't exist without CSS. Replace with `~/.config/omarss/theme.toml` (colour tokens per theme), or drop custom theming? | **`theme.toml`** as in §7.10. |
| Q5 | **Versioning and milestones.** The port changes nothing for users feature-wise but everything under the hood. | **Port ships as 0.4.0** at P9 (bump in P9's PR). M4 follows as 0.5.0. Add "M3.5 – iced port" to §14 of SPEC.md. |
| Q6 | **Node in dev tooling.** Is a Node script acceptable for the `run-omarss` fixtures server, or must the repo be Node-free? | **Node-free.** Port the fixtures server to a small Rust dev binary (`cargo run -p xtask -- fixtures`). |
| Q7 | **Fonts.** Use system fonts via generic families (`sans-serif`, `serif`, `monospace` through fontconfig), or bundle fonts (e.g. Inter, Source Serif, JetBrains Mono; ~1–2 MB) for consistent rendering? | **System fonts.** Log it; revisit if a distro renders badly. |
| Q8 | **Animated GIFs** show only their first frame. OK? | **Yes.** |
| Q9 | **Keyboard resizing of panes** moves from the splitters' arrow keys to width sliders in Settings → Appearance. OK? | **Yes.** |

Anything else the implementing agent finds that the spec doesn't settle: stop and ask if it
changes behaviour users would notice; otherwise pick the simplest option and log it.

---

## 15. Phases (one PR each)

Each phase must leave `main` building, tested, and shipping a working app. P0–P3 merge to
`main` while the Tauri app is still the shipped app; P4–P8 add the iced app alongside it
(unpackaged, binary `omarss-next`); P9 switches over.

### P0 – Spec and docs (docs-only PR)

- Fold §1, §3, §4, §7.10, §8, §9, §10, §11, §12 and the owner's §14 answers into `SPEC.md`
  (§0, §1, §2, §3, §4 incl. §4.1–§4.3, §6.6, §6.8, §7.2, §8.2, §8.4, §11, §12, §13, §14, §15).
  §4.3 "IPC contract" becomes "Core/UI boundary: the UI crate calls `omarss_core::Backend` and
  receives `AppEvent`s; it has no database, filesystem or network dependencies of its own".
- `CLAUDE.md`: Linux only; conventions (no SQL outside `crates/omarss-core/src/store/`; the
  generated-types rule is replaced by the crate boundary rule); "Before finishing" commands
  from §11.1; status line mentions the port.
- `docs/DECISIONS.md`: one line per owner answer, and superseding lines for the Windows-related
  entries (don't delete history).
- Done when: docs merged; nothing else changed.

### P1 – Drop Windows (still Tauri)

- Everything in §10 that doesn't depend on the port: network.rs, `pause_on_metered`,
  `metered_supported`, the metered UI and strings, manifest and build.rs linker bits, `windows`
  crate, `bundle.windows` in `tauri.conf.json`, the Windows CI and release jobs, README Windows
  sections, and the Windows form of the image protocol in the CSP.
- Done when: CI green on Ubuntu only, a Linux-only release builds, the app runs, metered
  setting gone without breaking existing settings rows.

### P2 – Workspace and `omarss-core`

- Create the workspace; move the backend (everything but Tauri glue) into
  `crates/omarss-core`, tests and fixtures included, `migrations/` alongside.
- Implement §5: `Backend`, `AppEvent` channel, dirs-based paths, tokio runtime calls, no Tauri or
  specta in core.
- `src-tauri` becomes a thin shell: its commands call `Backend`, a task forwards `AppEvent`s to
  tauri-specta events, the protocol handler calls `backend.load_image`. The frontend and the
  generated bindings don't change (models keep their shapes; specta derives can stay behind a
  `specta` cargo feature of core that only `src-tauri` enables, and go in P9).
- Done when: all backend tests pass from their new home; the Tauri app behaves exactly as before
  (manual check with the run-omarss skill); release still builds.

### P3 – Content IR in core

- §8.1 spike and decision; `document.rs` with the IR, the mapping rules and the tests in §8.2
  and §8.7; `backend.article_document(id)`. Not used by any UI yet.
- Done when: IR tests pass, including the whole fixture corpus building without panics.

### P4 – iced skeleton

- `crates/omarss` (binary `omarss-next`, not packaged, built in CI): `main.rs`, `App`, theme
  tokens and styles, i18n, format, the event subscription, loading/error screens, three-pane
  layout with sidebar (views, folders, feeds, counts, icons, status footer), article list
  (virtualised, paging, filters, sort), a minimal reader (header + plain-text content), refresh
  buttons.
- Done when: `cargo run -p omarss` opens the real database and you can browse and read; ported
  tests for views, virtual list, format, i18n, theme pass.

### P5 – Reader

- §8.3–§8.6: IR rendering, images and thumbnails through the cache, image modes, embed and
  media cards, links, enclosures, share menu, clipboard, open in browser, typography settings,
  reader scrolling. Check Q1/Q2 against the pinned iced and report.
- Done when: the fixture feeds and a few real feeds render readably next to the Tauri app;
  XSS fixture renders as inert text.

### P6 – Articles state and actions

- Full `ArticlesState` port (optimistic updates, stale-response dropping, mark-read modes,
  mark all + undo, show new articles), toasts, context menus, `actions.rs`.
- Done when: all `articles.test.ts` / `toasts.test.ts` equivalents pass.

### P7 – Subscriptions and sidebar editing

- Add feed, Edit feed, Folder, Confirm dialogs; unsubscribe; folders; drag and drop; Broken feeds
  page; OPML import/export with rfd; app menu.
- Done when: `addFeed.test.ts` / `sidebarOrder.test.ts` equivalents pass and the flows work
  against the fixtures server.

### P8 – Keyboard, settings, layout

- Keyboard dispatcher, all shortcuts, cheatsheet; Settings dialog, all sections (§7.11), rebinding
  UI; splitters; two-pane; theme file (Q4); system theme following; UI scale.
- Headless smoke test (§12.3).
- Done when: every box in §13 is ticked, checked by the agent on Hyprland (and ideally GNOME /
  X11 by the owner).

### P9 – Switch over

- Rename the binary to `omarss`; delete `src-tauri/`, `src/`, `index.html`, `public/` (keep the
  logo in `crates/omarss/assets/`), `package.json`, `package-lock.json`, `vite.config.ts`,
  `svelte.config.js`, `tsconfig.json`, `eslint.config.js`, `.prettierrc`, `.prettierignore`,
  Node bits of `.gitignore`, and the specta feature in core.
- Packaging and release (§11.3), CI (§11.2), README (install, build, data locations,
  troubleshooting), CHANGELOG 0.4.0 entry, version bump to 0.4.0, the `run-omarss` skill
  (§11.4), CLAUDE.md status line.
- Done when: CI green; a PR run of release.yml (it runs on PRs that touch packaging) produces
  `.deb`, `.rpm` and `.AppImage`; installing the `.deb` over a 0.3 install upgrades it in place
  and the app opens the old data; the AppImage runs on a clean Arch/Hyprland and an Ubuntu 22.04
  machine.

---

## 16. Risks and how to handle them

| Risk | Mitigation |
|---|---|
| iced API churn (0.14 → 0.15 mid-port) | Pin the minor version; upgrade only as its own PR after P9. |
| Rich HTML renders worse than a browser | The IR is deliberately small; compare against the Tauri app during P5 with the fixtures and 10–20 real feeds (long reads, code blogs, image-heavy, tables). Fix mapping rules, not one-off hacks. |
| Big articles slow to lay out | Build the IR off the UI thread (already in `spawn_blocking`); cap blocks (§8.2); lay out lazily if needed (only build widgets for blocks near the viewport, like the list). |
| Many images use lots of GPU memory | Only hold handles for the open article and visible thumbnails; decode at display size if the cache returns huge images. |
| GPU driver problems on some Linux machines | tiny-skia fallback and the documented env var (§9.5). |
| Losing behaviour hidden in Svelte components | Treat each `.svelte` file as spec: before deleting it in P9, re-read it and tick off every handler it had. |
| Package-name change breaking upgrades | §11.3's check against the real 0.3 artifacts. |

---

## Appendix A – File-by-file fate

| Path | Fate | Phase |
|---|---|---|
| `src-tauri/src/{store,feed,fetch,content*,opml.rs,models.rs,error.rs,clock.rs,logging.rs,services/*}` | Move to `crates/omarss-core/src/`, Tauri calls replaced | P2 |
| `src-tauri/src/scheduler.rs`, `events.rs` | Move; emit through `EventSender` | P2 |
| `src-tauri/src/paths.rs` | Move; `dirs` instead of Tauri | P2 |
| `src-tauri/src/commands/*` | Logic into `Backend`; files stay as thin Tauri wrappers until P9 | P2, P9 |
| `src-tauri/src/navigation.rs` | Allowlist fn → core; plugin deleted in P9 | P2, P9 |
| `src-tauri/src/protocol.rs` | `is_icon_file_name` → core; rest deleted in P9 | P2, P9 |
| `src-tauri/src/network.rs` | Delete | P1 |
| `src-tauri/src/bindings.rs`, `lib.rs`, `main.rs`, `build.rs`, `tauri.conf.json`, `capabilities/`, `icons/`, `windows-app-manifest.xml` | Delete (icons re-created under `crates/omarss/assets/`; manifest in P1) | P9 |
| `src-tauri/migrations/` | `crates/omarss-core/migrations/` | P2 |
| `src-tauri/src/tests/**` | `crates/omarss-core/src/tests/**` | P2 |
| `src/**` (Svelte/TS), `index.html`, `public/`, Node configs | Ported per §7 and §12, then deleted | P4–P9 |
| `src/lib/i18n/en.json` | `crates/omarss/assets/i18n/en.json` | P4 |
| `.github/workflows/ci.yml`, `release.yml` | Linux only (P1), then cargo-only (P9) | P1, P9 |
| `.claude/skills/run-omarss/` | Rewritten | P9 |
| `README.md`, `CHANGELOG.md`, `CLAUDE.md`, `SPEC.md`, `docs/DECISIONS.md` | Updated | P0, P1, P9 |
