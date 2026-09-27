---
name: run-omarss
description: Build, run, and drive the omarss Tauri desktop app (RSS reader). Use when asked to start or launch omarss, check that the app launches, take a screenshot of its UI, click through it, call its backend commands over IPC, inspect its SQLite database, or run its tests.
---

omarss is a Tauri 2 app (Rust backend, Svelte frontend in WebKitGTK). Agents drive it with
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

```bash
node .claude/skills/run-omarss/driver.mjs launch --fresh   # build (~35 s cold, seconds warm) + start with an empty DB
node .claude/skills/run-omarss/driver.mjs text ".list-pane h1"
node .claude/skills/run-omarss/driver.mjs click-text "Starred"
node .claude/skills/run-omarss/driver.mjs click ".row"
node .claude/skills/run-omarss/driver.mjs ss 01-article      # -> /tmp/omarss-driver/shots/01-article.png
node .claude/skills/run-omarss/driver.mjs quit
```

Look at the screenshot (Read the PNG). It's the 1280×800 viewport.

| command | what it does |
|---|---|
| `launch [--fresh] [--no-build] [--dev]` | build with `tauri build --debug`, start hidden, wait until the Svelte app has mounted. `--fresh` wipes the data dir; `--no-build` reuses the last driver build; `--dev` runs `npx tauri dev` (Vite HMR) instead |
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
node .claude/skills/run-omarss/driver.mjs db "select key, value from settings"
node .claude/skills/run-omarss/driver.mjs invoke get_article '{"id": 99999}'
node .claude/skills/run-omarss/driver.mjs eval "document.documentElement.dataset.theme"
node .claude/skills/run-omarss/driver.mjs launch --dev && node .claude/skills/run-omarss/driver.mjs console
```

Env overrides: `OMARSS_DRIVER_DIR` (default `/tmp/omarss-driver`), `OMARSS_INSPECTOR_PORT`
(default `9227`).

## Run (human path)

```bash
npx tauri dev    # window opens on the current workspace, hot reload; close the window to stop
```

## Test

```bash
npm test                   # Vitest: 25 tests
npm run check              # svelte-check
npm run lint && npm run format:check
cd src-tauri && cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check   # 28 tests
npm run gen:bindings       # regenerate src/lib/types.ts from the Rust commands/types
```

For backend-only changes, `cargo test <name>` in `src-tauri/` is the fastest loop. Use
`invoke` to check a command end-to-end through real IPC.

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
- **The article list defaults to unread-only.** A feed with 0 unread shows "You're all caught
  up", and `click .row` gives `NOT_FOUND`. Run `click-text "All"` first. (`click-text "All"`
  hits the filter button, not "All articles": exact matches win.)
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
