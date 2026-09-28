# Omarss (RSS reader)

`SPEC.md` is the source of truth. Read it fully before starting any work, and follow §0.

## How we work
- **One milestone per session** (§14). Implement only the milestone you're asked for. When it's done, stop and summarise: what was built, how to run it, tests added, and any decisions logged.
- A milestone is done when it builds, `cargo test` and frontend tests pass, and the app launches.
- Log any decision the spec doesn't settle in `docs/DECISIONS.md` (`YYYY-MM-DD: decision (reason)`). Don't add features outside the spec. Propose them there instead.
- If the spec seems wrong or contradictory, stop and ask. Don't silently deviate.
- Never delete or weaken a failing test to get green. Fix the code, or flag it in the summary.
- Keep Linux and Windows both working. No platform-specific code without a fallback (§2).

## Conventions
- The database is embedded SQLite (`rusqlite`, bundled). No database server or Docker.
- No SQL outside `src-tauri/src/store/` (§4.1).
- TypeScript types for IPC are generated from Rust with tauri-specta (§4.3). Never hand-write duplicates.
- Before finishing: `cargo fmt`, `cargo clippy -- -D warnings`, and the frontend lint/format/`svelte-check` from §13.

## Git workflow
- Repo: https://github.com/ChrisWaldenDev/omarss. `main` is protected: changes reach it only through a pull request.
- Never commit to `main`. Start each piece of work on a new branch from an up-to-date `main` (e.g. `m2-local-feeds`, `fix/<short-name>`), push it, and open a PR with `gh pr create`.
- Don't merge PRs unless the owner asks. CI (`.github/workflows/ci.yml`) runs on every PR.
- Every merge to `main` publishes a GitHub Release with Linux and Windows installers (`.github/workflows/release.yml`), so anything merged ships.
- Release versions are `<major>.<minor>.<patch>`, tagged and published automatically on merge (no manual tagging). When a milestone bumps `<major>.<minor>` (e.g. 0.3 → 0.4), bump the version in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` and `package.json` as part of that milestone's PR — that's the only manual step. The merge that lands the bump releases as `<major>.<minor>.0`; later merges increment the patch number on their own. Don't create a `v<major>.<minor>` git tag: that was the old (pre-0.3) manual marker step and would throw off the automatic count. See `docs/DECISIONS.md` for how the count works.

## Status
Completed: **M1 (Skeleton)**, **M2 (Local feeds core)**. Next: **M3 (Daily-driver polish)**. Update this line when a milestone is completed.
