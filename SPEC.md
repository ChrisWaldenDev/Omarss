# RSS Reader: Product & Technical Specification

> Name: **omarss**.
> Status: v1 spec. Audience: the coding agent implementing it, and the owner reviewing it.

---

## 0. Instructions for the implementing agent

- Build this in the **milestone order in §14**. Each milestone must build, pass its tests, and be runnable before starting the next.
- When this spec is silent or ambiguous, choose the simplest option that is consistent with the rest of the spec, and record the decision in `docs/DECISIONS.md` (one line: date, decision, reason).
- Do not add features beyond this spec without recording them as proposals in `docs/DECISIONS.md` first.
- "MUST" = required for v1. "SHOULD" = expected unless there's a recorded reason not to. "MAY" = optional/nice to have.
- Keep the app working on **both Linux and Windows** at every milestone. Don't use platform-specific APIs without a fallback for the other OS.

---

## 1. Overview

A fast, keyboard-friendly, all-in-one desktop RSS/Atom reader that runs as a single installable app on **Linux and Windows**. It works fully offline with local feeds, and can optionally sync with self-hosted services (FreshRSS, Miniflux). It fetches full article text for truncated feeds, supports user-defined filter rules, and sends desktop notifications for new items.

### 1.1 Goals
1. One app, one install: no separate server, no browser tab, no account required.
2. Instant UI: opening the app, switching feeds, and searching feel immediate even with 500 feeds and 200k articles.
3. Great reading experience: clean typography, full-text extraction, readable in light and dark.
4. Keyboard-first, mouse-friendly.
5. Your data is yours: local SQLite database, OPML import/export, no telemetry.

### 1.2 Non-goals (v1)
- Mobile apps, web version, or multi-user support.
- Podcast playback (enclosures are shown as links/attachments only).
- Social features, recommendations, or AI summarisation.
- Acting as a sync *server* for other clients.
- macOS support (MUST NOT be actively broken by design choices, but isn't tested or packaged).

---

## 2. Platforms & packaging

| Platform | Minimum | Packages |
|---|---|---|
| Linux x86_64 | glibc 2.35+ (Ubuntu 22.04, Fedora 38, Arch current); X11 and Wayland (incl. Hyprland) | AppImage, `.deb`, `.rpm`, Arch `PKGBUILD` (in `packaging/arch/`) |
| Windows x86_64 | Windows 10 21H2+ / Windows 11 | NSIS installer (`.exe`) and MSI; WebView2 bootstrapper included |

- Linux ARM64 and Windows ARM64: SHOULD build in CI, not required for v1 release.
- The app MUST run correctly with no system tray available (e.g. some Wayland compositors).

---

## 3. Technology stack

| Layer | Choice | Notes |
|---|---|---|
| App shell | **Tauri 2** | Native window, IPC, bundling, updater. WebKitGTK on Linux, WebView2 on Windows. |
| Backend | **Rust** (stable, 2021 edition) + **tokio** | All networking, parsing, storage, sync, and scheduling live here. |
| Frontend | **Svelte 5 + TypeScript + Vite** | Plain CSS with custom properties for theming; no heavy UI framework. |
| Database | **SQLite** via `rusqlite` (feature `bundled`) with **FTS5** | WAL mode. Migrations embedded in the binary. |
| HTTP | `reqwest` with `rustls` | gzip/brotli, HTTP/2, proxy support. |
| Feed parsing | `feed-rs` | RSS 0.9x/1.0/2.0, Atom, JSON Feed. Wrap it: the rest of the code uses our own `ParsedFeed` type. |
| HTML sanitising | `ammonia` | Strict allowlist (§8.3). |
| Full-text extraction | A Rust port of Mozilla Readability (evaluate `dom_smoothie`, `readability`, `article_scraper`; record choice) | Behind a trait so it can be swapped. |
| Credentials | `keyring` crate | Linux Secret Service (libsecret), Windows Credential Manager. Fallback in §11.4. |
| Logging | `tracing` + rolling file appender | |
| Tauri plugins | notification, single-instance, window-state, autostart, dialog, opener, updater, global-shortcut (optional) | Tray via Tauri 2 `tray-icon` feature. |

Rationale: Tauri gives a small (<15 MB installer) native app on both OSes, Rust keeps background fetching efficient, and the web frontend makes the reading view and theming easy.

---

## 4. Architecture

```
┌──────────────────────── Tauri app (single process) ────────────────────────┐
│  Frontend (Svelte, WebView)                                                 │
│   ├─ stores (feeds, articles, selection, settings)                          │
│   └─ api.ts ── typed wrappers around Tauri `invoke` + event listeners       │
│                         │  IPC (commands + events)                          │
│  Backend (Rust) ────────▼───────────────────────────────────────────────────│
│   commands/   thin Tauri command handlers → call services                   │
│   services/   FeedService, ArticleService, SearchService, RuleService,      │
│               ExtractService, OpmlService, SettingsService                  │
│   accounts/   Account trait → LocalAccount, FreshRssAccount, MinifluxAccount│
│   fetch/      HTTP client, conditional GET, discovery, favicon fetch        │
│   scheduler/  refresh queue, per-host concurrency, backoff                  │
│   store/      SQLite pool, migrations, repositories                         │
│   notify/     desktop notifications, tray badge                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 4.1 Rules
- Frontend never touches the network or disk directly except through backend commands. (The only exception: the WebView loads images from the backend's image cache protocol, §8.4.)
- All DB access goes through `store/` repositories. No SQL in commands or services outside `store/`.
- Long operations (refresh, import, sync, extraction) run as background tasks and report progress via events, never by blocking a command.
- Every public backend function returns `Result<T, AppError>`; `AppError` is serialisable to the frontend with a `kind` and user-readable `message`.

### 4.2 Repository layout
```
rss-reader/
├─ SPEC.md
├─ docs/DECISIONS.md
├─ src-tauri/
│  ├─ Cargo.toml
│  ├─ tauri.conf.json
│  ├─ migrations/           # 0001_init.sql, 0002_...sql
│  └─ src/
│     ├─ main.rs, lib.rs, error.rs
│     ├─ commands/ services/ accounts/ fetch/ scheduler/ store/ notify/ extract/ rules/ opml/
│     └─ tests/ (integration tests + fixtures/)
├─ src/                     # Svelte frontend
│  ├─ lib/api.ts, lib/types.ts (generated, see §4.3)
│  ├─ lib/stores/  lib/components/  lib/keyboard.ts
│  └─ routes or App.svelte
├─ packaging/arch/PKGBUILD
└─ .github/workflows/ci.yml, release.yml
```

### 4.3 IPC contract
- Use `specta` + `tauri-specta` (or `ts-rs`) to **generate TypeScript types and command bindings from Rust**. Hand-written duplicate types are not allowed.
- Events are namespaced: `refresh:progress`, `refresh:done`, `articles:changed`, `sync:status`, `feed:error`, `import:progress`.

---

## 5. Data model (SQLite)

All timestamps are UTC Unix seconds (`INTEGER`). IDs are `INTEGER PRIMARY KEY` unless stated. Enable `PRAGMA foreign_keys=ON`, `journal_mode=WAL`, `synchronous=NORMAL`.

```sql
CREATE TABLE accounts (
  id           INTEGER PRIMARY KEY,
  kind         TEXT NOT NULL CHECK (kind IN ('local','freshrss','miniflux')),
  name         TEXT NOT NULL,
  server_url   TEXT,                  -- NULL for local
  username     TEXT,
  -- secret stored in OS keyring under key "omarss:account:<id>"
  last_sync_at INTEGER,
  sync_state   TEXT,                  -- JSON blob of provider cursors
  created_at   INTEGER NOT NULL
);

CREATE TABLE folders (
  id          INTEGER PRIMARY KEY,
  account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  remote_id   TEXT,
  name        TEXT NOT NULL,
  sort_order  INTEGER NOT NULL DEFAULT 0,
  UNIQUE (account_id, name)
);

CREATE TABLE feeds (
  id              INTEGER PRIMARY KEY,
  account_id      INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  folder_id       INTEGER REFERENCES folders(id) ON DELETE SET NULL,
  remote_id       TEXT,
  url             TEXT NOT NULL,       -- feed URL (after permanent redirects)
  site_url        TEXT,
  title           TEXT NOT NULL,
  custom_title    TEXT,
  description     TEXT,
  icon_path       TEXT,                -- cached favicon file
  etag            TEXT,
  last_modified   TEXT,
  last_fetched_at INTEGER,
  next_fetch_at   INTEGER,
  fetch_interval  INTEGER,             -- seconds; NULL = use global default
  error_count     INTEGER NOT NULL DEFAULT 0,
  last_error      TEXT,
  full_text_mode  INTEGER NOT NULL DEFAULT 0,  -- 0 off, 1 auto-extract on arrival
  notify          INTEGER NOT NULL DEFAULT 0,
  paused          INTEGER NOT NULL DEFAULT 0,
  sort_order      INTEGER NOT NULL DEFAULT 0,
  created_at      INTEGER NOT NULL,
  UNIQUE (account_id, url)
);

CREATE TABLE articles (
  id             INTEGER PRIMARY KEY,
  feed_id        INTEGER NOT NULL REFERENCES feeds(id) ON DELETE CASCADE,
  remote_id      TEXT,
  guid           TEXT NOT NULL,        -- dedupe key, see §7.4
  url            TEXT,
  title          TEXT NOT NULL,
  author         TEXT,
  summary_html   TEXT,                 -- sanitised
  content_html   TEXT,                 -- sanitised, from feed
  fulltext_html  TEXT,                 -- sanitised, from extractor
  fulltext_state INTEGER NOT NULL DEFAULT 0,  -- 0 none, 1 ok, 2 failed
  published_at   INTEGER,
  updated_at     INTEGER,
  fetched_at     INTEGER NOT NULL,
  is_read        INTEGER NOT NULL DEFAULT 0,
  is_starred     INTEGER NOT NULL DEFAULT 0,
  read_at        INTEGER,
  starred_at     INTEGER,
  hidden         INTEGER NOT NULL DEFAULT 0,  -- set by rules
  content_hash   TEXT,                 -- to detect updated articles
  UNIQUE (feed_id, guid)
);
CREATE INDEX idx_articles_feed_pub   ON articles(feed_id, published_at DESC);
CREATE INDEX idx_articles_unread     ON articles(is_read, published_at DESC) WHERE hidden = 0;
CREATE INDEX idx_articles_starred    ON articles(is_starred, starred_at DESC) WHERE is_starred = 1;

CREATE TABLE enclosures (
  id          INTEGER PRIMARY KEY,
  article_id  INTEGER NOT NULL REFERENCES articles(id) ON DELETE CASCADE,
  url         TEXT NOT NULL,
  mime_type   TEXT,
  length      INTEGER
);

CREATE TABLE tags (
  id    INTEGER PRIMARY KEY,
  name  TEXT NOT NULL UNIQUE,
  color TEXT
);
CREATE TABLE article_tags (
  article_id INTEGER NOT NULL REFERENCES articles(id) ON DELETE CASCADE,
  tag_id     INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
  PRIMARY KEY (article_id, tag_id)
);

CREATE TABLE rules (
  id          INTEGER PRIMARY KEY,
  name        TEXT NOT NULL,
  enabled     INTEGER NOT NULL DEFAULT 1,
  scope       TEXT NOT NULL,           -- JSON: {"all":true} | {"feeds":[..]} | {"folders":[..]}
  conditions  TEXT NOT NULL,           -- JSON, see §10
  match_mode  TEXT NOT NULL CHECK (match_mode IN ('all','any')),
  actions     TEXT NOT NULL,           -- JSON, see §10
  sort_order  INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE pending_actions (         -- offline queue for remote accounts
  id          INTEGER PRIMARY KEY,
  account_id  INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  kind        TEXT NOT NULL,           -- mark_read, mark_unread, star, unstar, subscribe, unsubscribe, move, rename
  payload     TEXT NOT NULL,           -- JSON
  created_at  INTEGER NOT NULL,
  attempts    INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);  -- JSON values

CREATE VIRTUAL TABLE articles_fts USING fts5(
  title, author, body,                 -- body = text of content/fulltext, HTML stripped
  content='', tokenize='unicode61 remove_diacritics 2'
);
-- keep articles_fts in sync from Rust on insert/update/delete (rowid = articles.id)
```

- Migrations are numbered SQL files applied in order inside a transaction; the current version is stored in `PRAGMA user_version`.
- Before applying any migration to an existing DB, copy the DB file to `<data>/backups/pre-migration-<version>.sqlite`.

---

## 6. Features

### 6.1 Subscriptions
- **Add feed** (MUST): accept a feed URL *or* a website URL. If given a web page, run discovery:
  1. `<link rel="alternate" type="application/rss+xml|atom+xml|feed+json|json">` in `<head>`.
  2. Common paths: `/feed`, `/rss`, `/atom.xml`, `/feed.xml`, `/index.xml`, `/rss.xml`.
  3. If multiple feeds found, show a picker.
- Preview the feed (title, last 5 items) before confirming; choose folder and account.
- Edit feed: custom title, folder, refresh interval override, full-text mode, notifications, pause.
- Unsubscribe with confirmation; articles deleted except starred ones (starred ones are kept under a "Deleted feeds" pseudo-feed).
- Folders: create, rename, delete (feeds move to root), drag-and-drop reorder of feeds and folders.
- Feed health: feeds with `error_count >= 3` show a warning icon; a "Broken feeds" view lists them with the last error and a "Retry" / "Edit URL" action.

### 6.2 Reading
- **Three-pane layout** (default): sidebar (smart views, folders, feeds with unread counts) | article list | reader pane. User can switch to a **two-pane** layout (list expands into reader) in settings.
- Smart views: **All**, **Unread**, **Starred**, **Today**, and each **Tag**.
- Article list: title, feed icon + name, relative time, first ~140 chars of summary; optional thumbnail (first image). Unread items visually distinct. Virtualised; must scroll smoothly at 100k rows.
- Sort: newest first (default) / oldest first. Filter: unread only (default) / all.
- Reader pane: title, feed, author, date, link to original, rendered sanitised content, enclosures list, tags.
- Toggle **Feed content ↔ Full text** per article (§8). If a feed has full-text mode on, show full text by default.
- "Open in browser" uses the OS default browser.
- Reader settings: font family (serif/sans/system), font size, line width, line height. Stored globally.
- Mark as read: on open (default), or after N seconds, or on scroll past in list (configurable). "Mark all as read" for the current view, with an "older than 1 day / 1 week" option and **undo** (toast, 10 s).
- Star/unstar; add/remove tags from the reader.
- Share/copy: copy link, copy as Markdown link, "Email link" (mailto).

### 6.3 Search
- Global search box (`/` to focus). Full-text search over title, author, and body using FTS5.
- Supports quoted phrases, `-exclusion`, prefix `word*`, and field filters: `feed:<name>`, `folder:<name>`, `tag:<name>`, `is:unread`, `is:starred`, `before:YYYY-MM-DD`, `after:YYYY-MM-DD`.
- Results ranked by bm25, with a toggle to sort by date. Matched terms highlighted in the list snippet.
- Saved searches MAY be added as smart views.

### 6.4 OPML
- Import OPML 1.0/2.0: nested outlines become folders (nesting deeper than one level is flattened as `Parent / Child`). Show a summary (N feeds, M folders, K duplicates skipped). Import runs in background with progress.
- Export OPML 2.0 of all feeds or a single account, including folders and custom titles.
- Also export/import a **full backup** (`.omarssbackup` = zip of DB + settings + rules) for moving between machines.

### 6.5 Refresh
- Global default refresh interval: 30 min (options: 15 min, 30 min, 1 h, 2 h, 6 h, manual only).
- Refresh on startup (configurable), manual "Refresh all" (`r`/`Shift+R`), refresh single feed/folder.
- Honour feed-provided hints when they are *longer* than the user's interval: RSS `<ttl>`, `sy:updatePeriod`/`sy:updateFrequency`, HTTP `Cache-Control: max-age` and `Retry-After`. Never refresh a feed more often than every 10 minutes automatically.
- Details of fetching in §7.

### 6.6 Notifications & tray
- Desktop notification when new articles arrive in feeds with `notify = 1`, or when a rule's `notify` action matches. Batch: if >3 new items in one refresh cycle, send one summary notification ("12 new articles in 4 feeds").
- Clicking a notification focuses the app and opens that article (or the Unread view for summaries).
- System tray icon (MUST degrade gracefully when unavailable): shows unread count badge where supported; menu: Show/Hide, Refresh all, Pause refresh, Quit.
- "Close to tray" setting (default off on Linux, on on Windows); "Start minimised"; "Launch at login".
- Quiet hours setting: suppress notifications between two times.

### 6.7 Keyboard shortcuts (defaults, all rebindable in settings)

| Key | Action |
|---|---|
| `j` / `k` | Next / previous article (opens it) |
| `n` / `p` | Next / previous article without opening |
| `J` / `K` | Next / previous feed with unread |
| `o` / `Enter` | Open selected article |
| `v` | Open original in browser |
| `m` | Toggle read |
| `s` | Toggle star |
| `t` | Add tag |
| `f` | Toggle full text |
| `A` (Shift+a) | Mark all in view as read |
| `r` | Refresh current feed; `R` refresh all |
| `/` | Focus search |
| `g a` / `g u` / `g s` | Go to All / Unread / Starred |
| `Space` / `Shift+Space` | Scroll reader; at end, go to next article |
| `Ctrl+,` | Settings |
| `?` | Show shortcut cheatsheet |
| `Esc` | Close dialogs / clear search |

- Shortcuts are disabled while typing in inputs.
- Full mouse support; right-click context menus on feeds, folders, and articles.

### 6.8 Appearance
- Themes: Light, Dark, System (follows OS; on Linux via `prefers-color-scheme` / XDG portal). Accent colour picker.
- All colours defined as CSS custom properties so users MAY load a custom CSS file (`<config>/custom.css`) for theming.
- UI scale 80–150%. Respect OS reduced-motion.
- Sidebar and list widths are resizable and persisted; window size/position persisted (window-state plugin).

### 6.9 Retention & storage
- Keep articles for N days (default 90; options 30/90/180/365/forever). **Never** delete starred or tagged articles. Keep at least the latest 50 per feed regardless of age. Cleanup runs daily and after refresh, then `PRAGMA optimize`; `VACUUM` weekly when idle.
- Settings page shows DB size, image cache size, with "Clear image cache" and "Compact database".

---

## 7. Fetching

### 7.1 HTTP behaviour
- User-Agent: `omarss/<version> (+<project url>)`. Some sites block unknown agents; allow a per-feed UA override (advanced).
- Conditional GET with stored `ETag` / `Last-Modified`; handle `304` without parsing.
- Timeouts: 10 s connect, 30 s total; max response size 10 MB (feeds), 5 MB (pages for extraction).
- Redirects: follow up to 5. On `301`/`308`, update the stored feed URL. On `302`/`307`, don't.
- `410 Gone`: auto-pause the feed and flag it. `429`/`503`: honour `Retry-After`.
- Respect system proxy settings; allow a manual HTTP/SOCKS5 proxy in settings.
- Accept invalid TLS certs: **never** by default; a per-feed "allow insecure" advanced toggle MAY exist with a warning.

### 7.2 Scheduler
- A single scheduler task picks due feeds (`next_fetch_at <= now`, not paused) ordered by `next_fetch_at`.
- Global concurrency 8, **per-host concurrency 2**.
- Backoff on errors: next attempt = interval × 2^min(error_count, 5), capped at 24 h. Reset on success.
- Refresh pauses when the OS reports being offline and resumes on reconnect (poll connectivity every 60 s if no OS event is available). Refresh skips while on a metered connection if the user enables that setting (Windows only; Linux MAY via NetworkManager).
- Emits `refresh:progress {done, total}` and `refresh:done {new_count, errors}`.

### 7.3 Parsing
- Detect encoding from HTTP header, XML prolog, or BOM; fall back to UTF-8 with lossy decoding.
- Tolerate common malformations (unescaped `&`, bad dates, missing GUIDs). A parse failure counts as a feed error.
- Date parsing: RFC 822/2822, RFC 3339/ISO 8601, and common broken variants; if missing, use first-seen time.
- Resolve relative URLs in content against the item link, then `xml:base`, then the feed URL.
- Store enclosures (`<enclosure>`, Atom `rel="enclosure"`, `media:content`).
- Media RSS thumbnails (`media:thumbnail`) used for list thumbnails.

### 7.4 Deduplication
- `guid` = item `<guid>`/`<id>` if present; else item link; else SHA-256 of (title + published date + feed URL).
- If an existing article's `content_hash` changes, update its content but **keep read/star state** and do not re-notify. Optionally (setting, default off) mark updated articles unread.

### 7.5 Favicons
- Fetch in order: feed `<image>`/`<icon>`/`<logo>`, `<link rel="icon">` from `site_url`, `/favicon.ico`. Resize to 32×32 PNG, store in `<data>/icons/`. Refresh weekly. Fall back to a generated letter avatar.

---

## 8. Content handling

### 8.1 Full-text extraction
- `Extractor` trait: `async fn extract(url, html) -> Result<ExtractedArticle>`.
- On demand (press `f`) or automatically on arrival for feeds with `full_text_mode = 1` (queued, concurrency 2, per-host 1).
- Cache result in `articles.fulltext_html`; `fulltext_state` records success/failure. "Re-extract" action available.
- If extraction yields < 200 chars of text or fails, show the feed content with a small notice.

### 8.2 Rendering
- Content is rendered inside the reader pane in a sandboxed container with a restrictive Content Security Policy: no scripts, no inline event handlers, no forms, no iframes except an allowlist for YouTube/Vimeo embeds (converted to `youtube-nocookie.com`), which MAY be click-to-load.
- Links open in the external browser, never inside the app.

### 8.3 Sanitisation (ammonia)
- Allow: common text/structural tags, `a[href]`, `img[src,alt,title,width,height,srcset]`, `figure`, `figcaption`, `pre`, `code`, `blockquote`, tables, `ul/ol/li`, `h1–h6`, `sup/sub`, `details/summary`, `video/audio[src,controls,poster]`, `source`.
- Strip: `script`, `style`, `object`, `embed`, `form`, `input`, all `on*` attributes, `style` attributes, `javascript:`/`data:` URLs (except `data:image/*`).
- Add `rel="noopener noreferrer"` to links. Sanitise at ingest time *and* store sanitised HTML only.

### 8.4 Images & privacy
- Setting "Load remote images": Always (default) / Only for starred & opened / Never (click to load).
- Images are proxied through a custom Tauri protocol (`omarss-img://`) served by the backend with an on-disk LRU cache (default max 500 MB). This allows offline viewing and strips `Referer` and cookies.
- Strip known 1×1 tracking pixels and common tracker domains (small built-in blocklist). Remove `utm_*` params from article links (setting, default on).

---

## 9. Sync accounts

### 9.1 Account model
- There is always exactly one **Local** account (cannot be deleted). Users MAY add remote accounts; each has its own folder/feed tree shown as a top-level section in the sidebar (collapsible). Smart views span all accounts.
- `Account` trait (backend):
  ```rust
  #[async_trait]
  trait Account {
      async fn login(&self) -> Result<()>;
      async fn sync(&self, store: &Store, progress: ProgressFn) -> Result<SyncReport>;
      async fn subscribe(&self, url: &str, folder: Option<&str>) -> Result<RemoteFeed>;
      async fn unsubscribe(&self, feed: &Feed) -> Result<()>;
      async fn rename_feed / move_feed / create_folder / rename_folder / delete_folder ...;
      async fn set_read(&self, ids: &[RemoteId], read: bool) -> Result<()>;
      async fn set_starred(&self, ids: &[RemoteId], starred: bool) -> Result<()>;
      fn capabilities(&self) -> Capabilities;   // e.g. supports_tags, supports_full_text
  }
  ```
- For remote accounts, the **server fetches feeds**; the app does not fetch those feeds itself. Full-text extraction and rules still run locally on synced articles.

### 9.2 Providers (v1)
| Provider | API | Notes |
|---|---|---|
| **FreshRSS** | Google Reader API (`/api/greader.php`) | Auth: `ClientLogin` with API password. Also works with other GReader-compatible servers (Inoreader, The Old Reader, BazQux); the account form SHOULD allow a generic "Google Reader API" type with a custom URL. |
| **Miniflux** | Miniflux REST API v1 | Auth: API token (preferred) or username/password. |

MAY (post-v1): Nextcloud News, Feedbin, Fever API.

### 9.3 Sync algorithm
1. Flush `pending_actions` in order (retry with backoff; drop an action after 10 failed attempts and log it).
2. Pull subscriptions + folders; reconcile (server wins for structure).
3. Pull unread and starred ID lists; reconcile local state (server wins, except for actions still pending).
4. Fetch new article contents incrementally using the provider's cursor/`since` parameter stored in `accounts.sync_state`.
5. Apply rules to new articles; send notifications.
- Local actions on remote-account articles update the DB immediately (optimistic UI) and enqueue a `pending_action`.
- Sync runs on the same schedule as refresh, plus on startup and on manual refresh. Status shown in the sidebar footer (last synced time, error indicator).
- A remote account MAY be "converted" to local (export its feeds into the Local account) when removing it.

---

## 10. Rules (filters)

- Applied to each **new** article on arrival (local fetch or sync), in `sort_order`. A "Run on existing articles" button applies a rule retroactively.
- **Conditions** (JSON array):
  ```json
  [{"field": "title|author|content|url|feed|any", "op": "contains|not_contains|matches_regex|equals|starts_with", "value": "…", "case_sensitive": false}]
  ```
  Combined with `match_mode` `all` or `any`. Regex uses the Rust `regex` crate with a size limit; invalid regex rejected in the editor.
- **Actions** (JSON array): `mark_read`, `star`, `add_tag:<name>`, `hide` (sets `hidden=1`, excluded from all views except a "Hidden" view), `notify`, `stop` (skip later rules).
- Rules editor UI with live preview: "This rule would match N of the last 500 articles" plus the list.

---

## 11. Settings, storage locations & security

### 11.1 Locations
Use Tauri's path resolver (app identifier `dev.omarss.app`):
- Linux: config `~/.config/omarss/`, data `~/.local/share/omarss/` (DB, icons, image cache, backups), logs `~/.local/state/omarss/logs/` (or data dir).
- Windows: `%APPDATA%\omarss\` (config), `%LOCALAPPDATA%\omarss\` (data, cache, logs).
- Honour `XDG_*` variables on Linux.
- **Portable mode** (Windows, MAY): if a file named `portable` sits next to the executable, store everything in `./data/`.

### 11.2 Settings
Stored in the `settings` table (except window state). Settings UI sections: General, Reading, Refresh, Notifications, Appearance, Keyboard, Accounts, Rules, Privacy & Network, Storage, About. Every setting has a default and takes effect without restart unless marked otherwise.

### 11.3 Single instance
Second launch focuses the existing window. If launched with a feed URL argument or via `feed://` / `rss://` protocol handler (registered on install), open the Add Feed dialog pre-filled.

### 11.4 Security
- Tauri capabilities: grant the frontend only the commands it needs; no shell/fs plugin exposure to the WebView.
- App CSP: `default-src 'self'; img-src 'self' omarss-img: data:; script-src 'self'; frame-src https://www.youtube-nocookie.com https://player.vimeo.com; style-src 'self' 'unsafe-inline'`.
- Credentials only in the OS keyring. If the keyring is unavailable (e.g. no Secret Service on a minimal Linux), warn the user and offer to store the secret in the config dir with file permissions `0600` (clearly labelled as less secure).
- Never log secrets, tokens, or full article contents. Log files rotate daily, keep 7.
- No telemetry. Only network traffic: feed/page/image fetches, sync servers, and the update check (which can be disabled).

---

## 12. Non-functional requirements

| Area | Target |
|---|---|
| Cold start to interactive | < 1.5 s with 500 feeds / 200k articles on a mid-range laptop (SSD) |
| Switch feed / view | < 100 ms to show first page of list |
| Search | < 200 ms for typical queries over 200k articles |
| Refresh 500 feeds | < 2 min on a normal connection; UI stays responsive (no frame drops > 50 ms) |
| Idle memory | < 250 MB RSS total (including WebView) |
| Idle CPU | ~0% between refreshes |
| Installer size | < 15 MB (Windows NSIS), < 25 MB AppImage |
| Accessibility | Full keyboard navigation, visible focus rings, ARIA roles on lists/tree, WCAG AA contrast in built-in themes, screen reader labels on icon buttons |
| Robustness | A crash or kill during refresh never corrupts the DB (transactions + WAL). Malformed feeds never crash the app. |
| i18n | All UI strings in a single `en` locale file with keys, ready for translation; dates/numbers formatted by locale. Only English required for v1. |

### Linux/Wayland notes
- Test on Hyprland and GNOME (Wayland) and one X11 session.
- Known WebKitGTK issue: blank/black window on some GPU drivers. Document the workaround env var `WEBKIT_DISABLE_DMABUF_RENDERER=1` and apply it automatically if a startup render failure is detected, or expose it as a setting.
- Tray requires StatusNotifierItem support (libayatana-appindicator); treat absence as normal.

---

## 13. Testing & quality

- **Rust unit tests**: parsing fixtures (`tests/fixtures/` with ≥ 30 real-world feeds incl. broken ones: bad encoding, missing GUIDs, relative URLs, JSON Feed, RDF), dedupe, date parsing, sanitisation (XSS payload corpus), rules engine, OPML round-trip, search query parser, backoff maths.
- **Integration tests**: a local mock HTTP server (`wiremock`) for conditional GET, redirects, 410/429, discovery; mock FreshRSS and Miniflux APIs for sync including offline queue replay.
- **Frontend**: Vitest for stores and keyboard handling; Playwright (or WebdriverIO with `tauri-driver`) smoke test: add feed → read article → star → search → export OPML.
- **Performance test**: seed script generating 500 feeds / 200k articles; assert the targets in §12 for list query and search in CI (loose thresholds).
- Lint/format: `cargo fmt`, `cargo clippy -D warnings`, `eslint`, `prettier`, `svelte-check`.
- **CI** (GitHub Actions): matrix `ubuntu-22.04`, `windows-latest`; run all tests; build bundles on tags; release workflow publishes installers + updater manifest (signed with Tauri updater key).

---

## 14. Milestones

Each milestone ends with a runnable app on both OSes and a short changelog entry.

1. **M1 – Skeleton**: Tauri + Svelte scaffold, SQLite + migrations, generated IPC types, settings table, three-pane layout with dummy data, light/dark theme, CI building on Linux and Windows.
2. **M2 – Local feeds core**: add feed (with discovery), fetch/parse/store, folders, article list + reader, read/unread, star, refresh scheduler with conditional GET and backoff, favicons.
3. **M3 – Daily-driver polish**: keyboard shortcuts, mark all read + undo, OPML import/export, retention cleanup, sanitisation hardening, image proxy/cache, broken-feeds view, settings UI.
4. **M4 – Search & organisation**: FTS5 search with query syntax, tags, smart views, saved searches.
5. **M5 – Full text & rules**: extraction pipeline, per-feed full-text mode, rules engine + editor with preview, hidden view.
6. **M6 – Desktop integration**: notifications (batched, quiet hours), tray, close-to-tray, autostart, single instance, `feed://` handler, window state.
7. **M7 – Sync**: account framework, FreshRSS/GReader, Miniflux, offline pending-action queue, keyring storage.
8. **M8 – Release**: performance pass against §12, accessibility pass, backup/restore, packaging (AppImage, deb, rpm, PKGBUILD, NSIS, MSI), auto-updater, docs (README, user guide, shortcuts).

---

## 15. Acceptance criteria (v1 done when all pass)

- [ ] Fresh install on Windows 11 and Arch Linux (Hyprland) launches without extra setup.
- [ ] Importing a 300-feed OPML completes, and a full refresh finishes, with the UI responsive throughout.
- [ ] Adding `https://example-blog.com` (a site URL, not a feed) discovers and subscribes to its feed.
- [ ] Every shortcut in §6.7 works; the app is fully usable without a mouse.
- [ ] A truncated feed shows full article text after pressing `f`, and automatically when full-text mode is on.
- [ ] A rule "title contains 'sponsored' → hide" hides matching new articles and the preview shows the right count.
- [ ] Search `tag:rust "async trait" -tokio` returns correct results in < 200 ms on the seeded 200k DB.
- [ ] With a FreshRSS and a Miniflux account, marking items read while offline syncs correctly once back online.
- [ ] Notifications arrive for feeds with notifications on, batched when > 3, and clicking one opens the article.
- [ ] An XSS test corpus rendered in the reader executes no script.
- [ ] Killing the process mid-refresh and restarting leaves the DB intact and data consistent.
- [ ] Backup exported on Linux restores correctly on Windows (and vice versa).

---

## 16. Open questions (owner to decide; agent uses the default until then)

| Question | Default |
|---|---|
| Final app name & identifier | Decided: "omarss", `dev.omarss.app` |
| Auto-updater: which release host? | GitHub Releases |
| Include a "Read later" / Pocket-style saving of arbitrary URLs? | No (post-v1) |
| Add Nextcloud News sync in v1? | No (post-v1) |
| Licence | MIT |
