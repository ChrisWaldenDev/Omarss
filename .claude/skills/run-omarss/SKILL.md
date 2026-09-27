---
name: run-omarss
description: Build, run, and drive the Omarss Tauri desktop app (RSS reader). Use when asked to start or launch Omarss, check that the app launches, take a screenshot of its UI, click through it, call its backend commands over IPC, inspect its SQLite database, or run its tests.
---

Omarss is a Tauri 2 app (Rust backend, Svelte frontend in WebKitGTK). Agents drive it with
`.claude/skills/run-omarss/driver.mjs`. The driver builds a debug binary and launches it on a
**hidden Hyprland special workspace**: no window on the user's screen and no stolen focus. The
app gets an isolated data dir, and the driver talks to it through WebKitGTK's remote inspector.
Each command is a separate invocation; the app keeps running between them.

All paths are relative to the repo root.

## Prerequisites

Verified on Arch/omarchy with Hyprland 0.56, Rust 1.98, Node 26 (mise), webkit2gtk-4.1 2.52.
Nothing extra was installed. The driver needs Node ≥ 22.5 (global `WebSocket`, `node:sqlite`)
and a Hyprland session (`HYPRLAND_INSTANCE_SIGNATURE` set). No other launcher is implemented:
there's no Xvfb path, and CI's Ubuntu `apt-get` line in `.github/workflows/ci.yml` was not run
here.

```bash
npm install
```

## Run (agent path)

The app starts with no feeds. `fixtures` serves a local test site (a blog page whose
`<link rel="alternate">` points at its feed, a favicon, a podcast, a 1,500-item feed, and the
parser fixtures), so subscribing needs no internet:

```bash
node .claude/skills/run-omarss/driver.mjs fixtures                 # background server on 127.0.0.1:9230
node .claude/skills/run-omarss/driver.mjs launch --fresh           # build (~35 s cold, seconds warm) + start with an empty DB
node .claude/skills/run-omarss/driver.mjs invoke subscribe_feed '{"request": {"url": "http://127.0.0.1:9230/blog/feed.xml", "folderId": null, "title": null}}'
node .claude/skills/run-omarss/driver.mjs eval "location.reload()"  # the UI reloads its data after a subscribe made over IPC
sleep 2                                                             # let the reloaded page mount
node .claude/skills/run-omarss/driver.mjs click "button.row"       # open the newest article
node .claude/skills/run-omarss/driver.mjs ss 01-article            # -> /tmp/omarss-driver/shots/01-article.png
node .claude/skills/run-omarss/driver.mjs quit
node .claude/skills/run-omarss/driver.mjs fixtures stop
```

Look at the screenshot (Read the PNG). It's the 1280×800 viewport.

To go through the real Add feed dialog (discovery → preview → subscribe) instead of `invoke`:

```bash
node .claude/skills/run-omarss/driver.mjs click-text "Add a feed"   # the welcome button; the sidebar's + works too
node .claude/skills/run-omarss/driver.mjs eval "(() => { const i = document.querySelector('dialog input'); i.value = 'http://127.0.0.1:9230/blog/'; i.dispatchEvent(new Event('input', {bubbles: true})); })()"
node .claude/skills/run-omarss/driver.mjs click "dialog button[type=submit]"   # discovers the feed, shows the preview
sleep 1                                                             # discovery + preview fetch
node .claude/skills/run-omarss/driver.mjs click-text "Subscribe"
```

| command | what it does |
|---|---|
| `launch [--fresh] [--no-build] [--dev]` | build with `tauri build --debug`, start hidden, wait until the Svelte app has mounted. `--fresh` wipes the data dir; `--no-build` reuses the last driver build; `--dev` runs `npx tauri dev` (Vite HMR) instead |
| `fixtures [stop]` | start/stop the local test site on `127.0.0.1:9230` (`/blog/`, `/blog/feed.xml`, `/podcast.xml`, `/big.xml`, `/feeds/<fixture>`) |
| `status` | running? pid, window size, data/log/screenshot paths |
| `text [css]` | `innerText` of an element (default `body`) |
| `click <css>` | DOM `.click()` on the first match; prints `NOT_FOUND` and exits 1 if missing |
| `click-text <text>` | click a button/link/label by exact visible text, else by substring |
| `eval <js>` | evaluate an expression in the page; promises are awaited; result as JSON |
| `ss [name]` | screenshot → `/tmp/omarss-driver/shots/<name>.png` |
| `console` | console messages logged so far (e.g. `[vite] connected.` in dev mode) |
| `invoke <command> [json]` | call a backend command over IPC → `{ "ok": … }` or `{ "error": { kind, message } }` |
| `db <sql>` | read-only query on the app's SQLite DB (`/tmp/omarss-driver/data/omarss/omarss.sqlite`) |
| `log [n]` | tail of the app's stdout/stderr (`/tmp/omarss-driver/app.log`) |
| `quit` | stop the app |

Examples that ran:

```bash
node .claude/skills/run-omarss/driver.mjs click 'input[name=theme][value=light]'
node .claude/skills/run-omarss/driver.mjs db "select id, title, error_count, icon_path from feeds"
node .claude/skills/run-omarss/driver.mjs invoke get_refresh_status
node .claude/skills/run-omarss/driver.mjs invoke get_article '{"id": 99999}'
node .claude/skills/run-omarss/driver.mjs click "button[aria-label='Refresh all']"
node .claude/skills/run-omarss/driver.mjs invoke preview_feed '{"url": "https://expired.badssl.com/"}'   # TLS errors are refused
node .claude/skills/run-omarss/driver.mjs launch --dev && node .claude/skills/run-omarss/driver.mjs console
```

Env overrides: `OMARSS_DRIVER_DIR` (default `/tmp/omarss-driver`), `OMARSS_INSPECTOR_PORT`
(default `9227`), `OMARSS_FIXTURES_PORT` (default `9230`).

## Run (human path)

```bash
npx tauri dev    # window opens on the current workspace, hot reload; close the window to stop
```

## Test

```bash
npm test                   # Vitest
npm run check              # svelte-check
npm run lint && npm run format:check
cd src-tauri && cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check
npm run gen:bindings       # regenerate src/lib/types.ts from the Rust commands/types
```

For backend-only changes, `cargo test <name>` in `src-tauri/` is the fastest loop. The Rust
tests include parsing fixtures (`src-tauri/src/tests/fixtures/feeds/`) and a mock HTTP server
(wiremock) for fetch/refresh behaviour. Use `invoke` to check a command end-to-end through real
IPC.

## Gotchas

- **Hyprland 0.56 dispatch is Lua.** `hyprctl dispatch 'exec [workspace special:x silent] cmd'`
  fails with `']' expected near 'special'`. The driver uses
  `hl.dsp.exec_cmd("<script>", { workspace = "special:omarss-driver silent", float = true })`.
  Without `float = true` the window tiles to the monitor size (2536×1390 here) instead of the
  1280×800 in `tauri.conf.json`.
- **Processes spawned by Hyprland don't get your shell's PATH.** `tauri dev` then dies with
  `failed to run 'cargo metadata' … No such file or directory`. The driver writes PATH into
  `/tmp/omarss-driver/launch.sh`.
- **`tauri dev` and `tauri build --debug` both write `src-tauri/target/debug/omarss`**, with
  different features. After a `tauri dev`, that binary loads `http://localhost:1420` and shows
  `about:blank` if Vite isn't running, so `launch` reported "UI never mounted". The driver
  builds into `src-tauri/target/driver/` so the two never overwrite each other.
- **The WebKit inspector protocol is not Chrome's CDP.** Every command must be wrapped in
  `Target.sendMessageToTarget`. `Runtime.evaluate` ignores `awaitPromise` and returns `{}`, so
  follow it with `Runtime.awaitPromise` (the driver does).
- **The hidden window never fires `requestAnimationFrame`.** Don't wait on rAF or transitions
  in `eval`. Svelte updates (microtasks), IPC and `Page.snapshotRect` screenshots all work.
- **There's no pointer or keyboard injection.** Clicks are DOM `.click()`. Use `eval` for
  anything else.
- **The article list is virtualised.** Only the ~15 visible rows exist in the DOM, and pages
  of 100 load as you scroll. To reach later rows, scroll `.list-pane .viewport`
  (`vp.scrollTop = vp.scrollHeight; vp.dispatchEvent(new Event('scroll'))`). Article rows are
  `button.row`; the list's own wrapper divs are `.virtual-row`.
- **Type full `http://` URLs for the fixtures.** A bare address like `127.0.0.1:9230/blog/` is
  treated as `https://…` (what users expect for real sites), and the fixture server only
  speaks HTTP.
- **Subscribing over `invoke` doesn't refresh the UI.** The dialog path reloads the sidebar
  itself; after an `invoke`, run `eval "location.reload()"`.
- **Context menus and drag and drop can be driven with synthetic events.** For example
  `el.dispatchEvent(new MouseEvent('contextmenu', {bubbles: true, clientX: 100, clientY: 300}))`,
  or `new DragEvent('dragstart' | 'dragover' | 'drop', {bubbles: true, cancelable: true,
  dataTransfer: new DataTransfer()})` on the sidebar rows. Read the DOM after a short wait:
  Svelte renders the menu on the next tick.
- **Remote images in articles don't load yet.** The CSP only allows `omarss-img:` images, and
  the image proxy arrives in M3. The reader hides blocked images.
- **The article list defaults to unread-only.** A feed with 0 unread shows "You're all caught
  up", and `click "button.row"` gives `NOT_FOUND`. Run `click-text "All"` first.
  (`click-text "All"` hits the filter button, not "All articles": exact matches win.)
- **Debug launches rewrite `src/lib/types.ts`** if Rust IPC types changed, by design. Expect
  a diff in git; commit it.
- **Your real data is untouched.** The app gets `XDG_DATA_HOME=/tmp/omarss-driver/data`.
  `launch` without `--fresh` keeps the DB, which is how to test persistence across restarts
  (`quit`, then `launch --no-build`).
- **Vitest hooks:** `beforeEach(() => mock.mockReset())` returns the mock, and Vitest treats a
  returned function as teardown and calls it. The next test then fails with
  `Unknown Error: <your mocked rejection>`. Use a block body: `beforeEach(() => { mock.mockReset(); })`.

## Troubleshooting

- **`ERROR: app is not running (no inspector on port 9227)`**: run `launch` first.
- **`ERROR: already running (or port 9227 is busy)`**: run `quit`. Check `status`.
- **`app window opened but the UI never mounted`**: `eval "location.href"` shows what the
  WebView loaded, and `log` shows the backend output. `about:blank` means a dev-server binary
  ran without Vite (see the target-dir gotcha).
