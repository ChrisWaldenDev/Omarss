# Dependency cleanup plan

Date: 2026-10-03. Scope: `src-tauri/Cargo.toml` (30 runtime, 1 Windows-only, 1 build, 3 dev
dependencies) and `package.json` (1 runtime, 17 dev dependencies).

This is a plan only. Nothing has been changed yet.

## How this was measured

Crate counts are unique `name version` pairs from `cargo tree`, for `x86_64-unknown-linux-gnu`
and `x86_64-pc-windows-msvc` combined. Each change was tried on a scratch copy of `Cargo.toml`.

- **Compiled** means everything `cargo build` compiles (`-e normal,build`), including build
  scripts and proc-macros.
- **Shipped** means crates linked into the app binary (`-e normal,no-proc-macro`).

To check a crate yourself: `cargo tree -i <crate>` shows who pulls it in, and
`cargo tree -e normal,no-proc-macro --target <triple> --prefix none | sort -u | wc -l` counts the
shipped crates.

## Summary

|                                                 | Compiled | Shipped |
| ----------------------------------------------- | -------: | ------: |
| Bare Tauri 2 app (tauri, tauri-build, serde)    |      292 |     180 |
| Omarss today                                    |      481 |     375 |
| After the six recommended changes (§1)          |      468 |     352 |
| ...plus the opener swap (§2, needs owner OK)    |      447 |     331 |
| ...plus every optional item (§3)                |      432 |     322 |

Most of the ~190 crates Omarss adds on top of Tauri come from things the spec requires. The
biggest are reqwest (29 shipped crates), the dialog plugin's xdg-portal backend (22), image (15),
ammonia (9) and specta (5). The dependencies we don't need add about 23 shipped crates. Dropping
one plugin the spec lists would remove 21 more.

Two findings are duplicates of code we already ship, not just extra features:

1. **Two HTML parsers.** scraper bundles html5ever 0.39, and ammonia bundles html5ever 0.40 (§1.1).
2. **A second async runtime and a second copy of the Windows bindings**, both from
   tauri-plugin-opener (§2.1).

## Verdict for every direct dependency

| Crate                                     | Verdict                          | Reason                                                                                                       |
| ----------------------------------------- | -------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| `tauri`, `tauri-build`                    | Keep                             | App shell (§3)                                                                                               |
| `tauri-plugin-dialog`                     | Keep                             | §3. The xdg-portal backend is needed for Wayland (DECISIONS 2026-09-27)                                      |
| `tauri-plugin-opener`                     | **Owner decision** (§2.1)        | One call, which is just `open::that_detached`. Brings a second async runtime                                 |
| `serde`, `serde_json`                     | Keep                             | IPC and settings. Tauri already pulls in `serde_json`                                                        |
| `chrono`                                  | Keep                             | Date parsing and the local "Today" boundary. feed-rs already pulls it in                                     |
| `rusqlite`                                | Keep                             | §3                                                                                                           |
| `r2d2`, `r2d2_sqlite`                     | **Remove** (§1.2)                |                                                                                                              |
| `specta`, `tauri-specta`                  | Keep                             | §4.3 generated bindings                                                                                      |
| `specta-typescript`                       | Keep                             | Needed to name `Typescript` in `bindings.rs`. tauri-specta already pulls it in, so it costs nothing          |
| `tracing`                                 | Keep                             | §3                                                                                                           |
| `tracing-subscriber`                      | Keep, **drop `env-filter`** (§1.6) |                                                                                                            |
| `tracing-appender`                        | Keep (see §3.4)                  | §3 names a rolling file appender                                                                             |
| `tokio`                                   | Keep                             | Semaphore, mpsc, oneshot, JoinSet, `select!`, sleep. Tauri builds it anyway                                  |
| `reqwest`                                 | Keep                             | §3                                                                                                           |
| `rustls`                                  | Keep                             | Needed to install the `ring` provider (DECISIONS 2026-09-27). Already pulled in by reqwest                   |
| `feed-rs`, `ammonia`                      | Keep                             | §3                                                                                                           |
| `scraper`, `ego-tree`                     | **Remove** (§1.1)                |                                                                                                              |
| `encoding_rs`                             | Keep                             | §7.3. Already pulled in by quick-xml (via feed-rs)                                                           |
| `sha2`                                    | Keep (optional version change, §3.1) | §7.4 requires SHA-256                                                                                    |
| `url`                                     | Keep                             | Free: Tauri, reqwest and ammonia use the same crate (`tauri::Url` is a re-export of it)                      |
| `httpdate`                                | **Remove** (§1.4)                |                                                                                                              |
| `image`                                   | Keep (optional feature trim, §3.2) | §7.5 requires resizing to a 32×32 PNG                                                                      |
| `futures`                                 | **Remove** (§1.3)                |                                                                                                              |
| `base64`                                  | **Remove** (§1.5)                |                                                                                                              |
| `roxmltree`                               | Keep (see §3.3)                  | OPML import                                                                                                  |
| `windows` (Windows only)                  | Keep                             | Metered-connection check (§7.2). Bump to Tauri's version if §2.1 is done                                     |
| `tempfile`, `wiremock`, `tauri[test]` (dev) | Keep                           | `wiremock` is required by §13. `tempfile` is used by 9 test modules                                          |

## 1. Recommended removals

All six together remove 23 shipped crates (375 to 352) and 13 compiled crates (481 to 468), and
cut the direct runtime dependencies from 30 to 24. Each item lists the steps and tests.

### 1.1 `scraper` + `ego-tree` → `html5ever` (the version ammonia already uses)

**Why it's unnecessary.** ammonia ships html5ever 0.40, and scraper 0.27 (the latest) ships
html5ever 0.39, so the binary contains two complete HTML parsers. We use scraper for three small
jobs: finding `<link>` elements, finding `<base>`, and extracting visible text. `ego-tree` is a
direct dependency only so one `match` can name `Edge`. scraper's default `main` feature also
builds `getopts` for a command-line tool we don't use.

**What it saves.** 13 shipped crates: scraper, ego-tree, html5ever 0.39, markup5ever 0.39,
cssparser 0.37, selectors, servo_arc, string_cache 0.9, web_atoms 0.2, derive_more, rustc-hash,
getopts and unicode-width. Only 4 fewer compiled crates, because tauri-utils still builds the 0.39
stack at compile time.

**Files.** `Cargo.toml`, `src/content.rs` (`html_to_text`), `src/fetch/discovery.rs`
(`feed_links`), `src/fetch/favicon.rs` (`icon_links`), and a new helper such as
`src/content/dom.rs`.

**Steps.**

1. In `Cargo.toml`, remove `scraper` and `ego-tree`, and add `html5ever = "0.40"` with a comment
   saying it must match ammonia's version. ammonia exposes its tree as the public
   `ammonia::rcdom::RcDom`, which implements html5ever's `TreeSink`. If the versions drift apart,
   the build fails, so the duplicate can't come back unnoticed.
2. Add a small helper module (about 40 lines):
   - `parse_document(html) -> RcDom` using
     `html5ever::parse_document(RcDom::default(), Default::default()).one(html)`. This needs
     `use html5ever::tendril::TendrilSink`.
   - `parse_fragment(html) -> RcDom`, the same with `html5ever::parse_fragment`, using a `body`
     context (`QualName::new(None, ns!(html), local_name!("body"))`, `vec![]`, `false`).
   - `elements(dom, name)`: an iterative depth-first walk that returns elements with that local
     name in document order. Use an explicit stack, not recursion, because feed HTML can be nested
     arbitrarily deep. scraper's `traverse()` is iterative too.
   - `attr(node, name) -> Option<String>`.
3. `feed_links`: use the first `base` element with an `href` as the base URL, then apply today's
   filters to each `link` element that has `rel`, `href` and `type`.
4. `icon_links`: rank each `link` element with `rel` and `href` exactly as today.
5. `html_to_text`: walk the fragment with the same explicit stack. Append text nodes, and add a
   space when entering and when leaving an `is_block` element. This replaces the
   `Edge::Open`/`Edge::Close` events.
6. Tests: these must pass unchanged:
   - the `html_to_text` tests in `content.rs`
   - the three `discovery.rs` tests and `favicon.rs::ranks_icon_links`
   - the discovery tests in `tests/refresh.rs`

   Add one test with deeply nested input (for example 50,000 nested `<div>`s) to show it doesn't
   overflow the stack.
7. Add to DECISIONS.md: "Feed discovery, favicon links and text excerpts parse HTML with html5ever
   and ammonia's `rcdom` instead of scraper. scraper bundles a second, older html5ever, and this
   keeps one HTML parser in the binary. html5ever's version must track ammonia's."

**Note for M5.** The full-text extractor (§8.1) will bring its own HTML stack. When choosing it,
run `cargo tree -d | grep html5ever`. Prefer a candidate built on the same html5ever as ammonia,
or the duplicate parser comes back. Record the result in DECISIONS.md.

### 1.2 `r2d2` + `r2d2_sqlite` → a small pool in `store/mod.rs`

**Why it's unnecessary.** The pool is used in one place: `Store::run` takes a connection on a
blocking thread. We only need up to 8 reused connections, each opened with the three PRAGMAs.
r2d2 adds a background maintenance thread pool (`scheduled-thread-pool`) and a health-check query
on every checkout. r2d2_sqlite turns on uuid's random `v4` feature, only to name in-memory
databases, and that pulls in `rand` 0.10 and `chacha20`.

**What it saves.** 6 crates, shipped and compiled: r2d2, r2d2_sqlite, scheduled-thread-pool,
rand 0.10, rand_core 0.10 and chacha20.

**Files.** `Cargo.toml`, `src/store/mod.rs`, `src/error.rs` (`From<r2d2::Error>`).

**Steps.**

1. Replace `pool: r2d2::Pool<SqliteConnectionManager>` with an `Arc<Pool>`. The pool holds the
   DB path, `idle: Mutex<Vec<Connection>>`, and a `tokio::sync::Semaphore` with `POOL_SIZE` (8)
   permits.
2. In `Store::run`:
   1. `acquire_owned()` a permit. This is async, so waiting for a connection never ties up a
      thread.
   2. Inside `spawn_blocking`, pop an idle connection, or open a new one and run the existing
      PRAGMA batch.
   3. Run `f`, push the connection back, and drop the permit.

   If `f` panics, its connection is dropped rather than reused.
3. Delete `impl From<r2d2::Error> for AppError`. Opening a connection already fails with
   `rusqlite::Error`.
4. Two behaviour changes, both harmless:
   - Connections open on first use. r2d2 opened all 8 before `Store::open` returned.
   - There's no 30-second checkout timeout. A waiting task just waits, and SQLite's
     `busy_timeout=5000` still limits lock waits.
5. Tests: the two tests in `store/mod.rs` (WAL, `foreign_keys` on pooled connections) and every
   service test. Add a test that runs more than 8 concurrent `run` calls, then checks that they
   all finish and that each connection has `foreign_keys=1`.
6. Add a DECISIONS.md entry that supersedes the "Pooled connections (r2d2, 8)" wording in the
   2026-09-27 migrations entry.

### 1.3 `futures` → spawned tokio tasks

**Why it's unnecessary.** It's used for `join_all`, twice, in `fetch/discovery.rs`. tokio already
covers this, and `scheduler.rs` already uses `JoinSet`.

**What it saves.** 1 crate, the `futures` facade. `futures-util` stays, because reqwest uses it.

**Steps.**

1. In `discover`, give each probe its own clone of `HttpClient` (cheap, it's an `Arc` inside) and
   its own `String` URL.
2. Spawn each probe with `tauri::async_runtime::spawn`, keep the `JoinHandle`s in a `Vec`, and
   await them in order. This keeps today's result order, which `dedupe` depends on: the first
   advertised feed wins. Treat a `JoinError` as `None`.
3. Remove `use futures::future::join_all` and the dependency.
4. Tests: the `discover(...)` tests in `tests/refresh.rs` (around lines 415 to 450).

### 1.4 `httpdate` → `chrono`

**Why it's unnecessary.** It has one call: parsing a `Retry-After` date in
`fetch/mod.rs::parse_retry_after`. The HTTP date format (IMF-fixdate,
`Sun, 06 Nov 1994 08:49:37 GMT`) is valid RFC 2822, which `chrono` (already a dependency)
parses.

**What it saves.** 1 crate.

**Steps.**

1. Use `chrono::DateTime::parse_from_rfc2822(value.trim()).ok()?`, and return
   `(at.timestamp() - now_unix_secs).max(0)`, where `now_unix_secs` comes from
   `now.duration_since(UNIX_EPOCH)`.
2. In the `parses_cache_hints` test, replace `httpdate::fmt_http_date(now + 90s)` with the literal
   `"Sun, 09 Sep 2001 01:48:10 GMT"` (that's `1_000_000_090`). The assertion stays the same.
3. One behaviour change: httpdate also accepted the obsolete RFC 850 and asctime formats, which
   RFC 9110 forbids servers from sending. A `Retry-After` in those formats would now be ignored,
   and the normal backoff applies instead. Note this in DECISIONS.md.

### 1.5 `base64` → hex encoding

**Why it's unnecessary.** It's used only to put an image URL into one path segment of the
`omarss-img` proxy URL, in `services/images.rs` (`proxy_url` and `decode_proxy_path`). Any
URL-safe, reversible encoding works, and hex takes about 10 lines of std.

No migration is needed:

- Proxy URLs are built at display time and never stored (DECISIONS 2026-09-27).
- Cache files are named by the SHA-256 of the image URL, so the existing cache stays valid.

**What it saves.** 1 shipped crate. base64 0.22 is still compiled for tauri-codegen at build time.

**Steps.**

1. `proxy_url`: write each byte of the URL as two lowercase hex digits.
2. `decode_proxy_path`: reject odd lengths and non-hex input, decode each pair with
   `u8::from_str_radix(.., 16)`, then keep today's `String::from_utf8` and http(s) checks.
3. Update the doc comments in `services/images.rs` and `protocol.rs` (`img/<base64url(url)>`
   becomes `img/<hex(url)>`). Add a DECISIONS.md entry that supersedes the URL format in the
   2026-09-27 image-proxy entry.
4. Tests: in `proxy_urls_round_trip`, replace `URL_SAFE_NO_PAD.encode("file:///etc/passwd")`
   with the new encoder. The character-set assertion still holds. The proxy checks in
   `tests/polish.rs` don't depend on the encoding.

Hex makes the path twice as long as the URL, instead of 1.33 times. That doesn't matter for a
local protocol.

### 1.6 `tracing-subscriber`: drop the `env-filter` feature

**Why it's unnecessary.** `EnvFilter` is the regex-based filter that can match spans and fields.
We use one fixed filter string, and `RUST_LOG` overrides it with the same `target=level` syntax.
`tracing_subscriber::filter::Targets` parses that syntax without regex.

**What it saves.** 1 crate (`matchers`). It's small but costs nothing, so do it in the same PR as
§1.3 to §1.5.

**Steps.**

1. In `Cargo.toml`, set:
   `tracing-subscriber = { version = "0.3", default-features = false, features = ["std", "fmt", "ansi", "registry", "smallvec", "tracing-log"] }`.
   Keep `tracing-log`. It forwards `log` records from Tauri, wry and tao into our log file.
   Without it, their warnings would quietly disappear.
2. In `logging.rs`:
   `let filter = std::env::var("RUST_LOG").ok().and_then(|s| s.parse::<Targets>().ok()).unwrap_or_else(|| DEFAULT_FILTER.parse().expect("valid filter"));`
3. One behaviour change: span and field directives in `RUST_LOG` (such as `[refresh]=debug`) stop
   working, while `target=level` directives work as before. Note this next to the 2026-09-27
   logging entry in DECISIONS.md.

## 2. Needs an owner decision (the spec names it)

### 2.1 `tauri-plugin-opener` → the `open` crate

**Why it's unnecessary.** We call one function, `app.opener().open_url(url, None)`, in
`navigation.rs`. The WebView gets no opener permissions (`capabilities/default.json`). In
tauri-plugin-opener 2.6, `open_url(url, None)` is exactly `open::that_detached(url)` (see its
`src/open.rs`). The plugin also brings two duplicates:

- **Linux:** the plugin enables zbus's `async-io` backend, which adds a second async runtime next
  to tokio: async-io, async-executor, async-process, blocking, polling and others.
- **Windows:** the plugin uses `windows` 0.61, while Tauri itself uses 0.62, so two copies of the
  Windows bindings ship. Our own `windows = "0.61"` matches the plugin, not Tauri.

**What it saves.** 21 crates, shipped and compiled, if `windows` is also bumped to 0.62.

**Spec.** §3 lists `opener` among the Tauri plugins. Per CLAUDE.md, this needs the owner's
approval and a PROPOSAL entry in DECISIONS.md before any code changes.

**Steps (if approved).**

1. In `Cargo.toml`, remove `tauri-plugin-opener`, add `open = "5"`, and bump `windows` to the
   version Tauri uses (0.62 today; check with `cargo tree -i windows@0.62.2`).
2. In `navigation.rs`, call `open::that_detached(url.as_str())` with the same error mapping, and
   drop the `OpenerExt` import. In `lib.rs`, remove `.plugin(tauri_plugin_opener::init())`.
3. In `network.rs`, make sure the WinRT `NetworkInformation` calls still compile against 0.62.
   Windows CI will catch it if not.
4. Capabilities need no change, since no opener permission was ever granted.
5. Manual check on Linux (Wayland and X11) and Windows: "Open in browser", links in the reader,
   and "Email link" (`mailto:`).
6. Add a DECISIONS.md entry, and a note against §3's plugin list.

M6 will add up to six more plugins (notification, single-instance, window-state, autostart,
updater, global-shortcut). When adding each one, run `cargo tree -d` and check for new duplicate
runtimes or `windows` versions.

## 3. Optional, low value (not recommended now)

### 3.1 `sha2` 0.11 → 0.10

Tauri's build-time code (tauri-codegen) uses sha2 0.10, so two digest stacks are compiled.
Switching to 0.10 removes 6 compiled crates: sha2 0.11, digest 0.11, block-buffer 0.12,
crypto-common 0.2, hybrid-array and const-oid. That only shortens build time; the shipped count
doesn't change. The API we use in `content.rs` (`Sha256::new`, `update`, `finalize`, `digest`)
is the same in both versions.

The downside is staying on an older major version and switching back once Tauri moves to 0.11.
Skip it unless CI build time becomes a concern.

### 3.2 `image` features

- `bmp` is redundant, because the `ico` feature already enables it. Removing it from the list is
  tidier but changes no crates.
- Dropping `gif` and `webp` would remove 5 crates (gif, weezl, color_quant, image-webp,
  quick-error). But GIF and WebP favicons and feed logos would then fall back to the next source
  or the letter avatar (§7.5). Keep them unless installer size becomes a problem.
- Keep `jpeg`: many feed `<image>` logos and podcast covers are JPEGs.

### 3.3 `roxmltree` → `quick-xml`

quick-xml 0.42 is already compiled for feed-rs, so OPML import could use it and save 1 crate. But
that means rewriting `opml::parse` as a streaming parser with a manual folder stack. By default
quick-xml also doesn't expand internal DTD entities (roxmltree does, with `allow_dtd: true`) and
doesn't report elements left unclosed at end of file. That's about 60 lines of riskier code to save
one small, dependency-free crate. Not worth it.

### 3.4 `tracing-appender` → a hand-written daily log file

This would save 2 crates (tracing-appender and symlink). But §3 names a rolling file appender, and
hand-writing date rollover plus "keep 7 files" pruning is about 80 lines with edge cases. Not
worth it.

## 4. Frontend (npm)

The frontend is already lean. It has one runtime dependency (`@tauri-apps/api`), and every dev
dependency is used by a config file or script:

| Package                                                                                                  | Used by                                                   |
| -------------------------------------------------------------------------------------------------------- | --------------------------------------------------------- |
| `vite`, `@sveltejs/vite-plugin-svelte`, `svelte`                                                         | Build                                                     |
| `@tauri-apps/cli`                                                                                        | `npm run tauri`                                           |
| `typescript`, `@tsconfig/svelte`, `svelte-check`                                                         | `npm run check` (§13)                                     |
| `@types/node`                                                                                            | `process.env` in `vite.config.ts` (`types: ["node"]`)     |
| `eslint`, `@eslint/js`, `typescript-eslint`, `eslint-plugin-svelte`, `eslint-config-prettier`, `globals` | `eslint.config.js` (§13)                                  |
| `prettier`, `prettier-plugin-svelte`                                                                     | `.prettierrc` (§13)                                       |
| `vitest`                                                                                                 | `npm test` (§13)                                          |

Nothing to remove.

## 5. Suggested pull requests

1. `fix/trim-small-deps`: §1.3 `futures`, §1.4 `httpdate`, §1.5 `base64` and §1.6 `env-filter`.
   The diffs are small and existing tests cover them.
2. `fix/sqlite-pool`: §1.2.
3. `fix/drop-scraper`: §1.1.
4. Only after the owner approves: `fix/opener-to-open` for §2.1.

For each PR:

- Run `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`, and the frontend
  lint/format/`svelte-check`/test, and launch the app.
- Check that `cargo tree -i <crate>` finds nothing for each removed crate.
- Add a DECISIONS.md entry.

## 6. Keeping it lean

A short checklist before adding a crate (this could go in CLAUDE.md if the owner wants it):

- Run `cargo tree -i <crate>` first. If the crate is already in the tree, using it is free; match
  the major version that's already there.
- If the job takes under about 30 lines of std or an existing dependency (one `join_all`, one date
  format, one base64 call), write those lines instead.
- After adding a crate, run `cargo tree -d` to look for new duplicate versions, and count the
  shipped crates before and after (see "How this was measured").
- Use `default-features = false` and turn on only the features the code uses.
