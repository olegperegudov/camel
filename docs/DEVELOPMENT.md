# Development

Tauri 2, no frontend framework, no bundler — `src/` is served as-is
(`frontendDist: "../src"`). Rust does everything stateful; the webview only
formats and paints.

## Layout

```
src/                  panel frontend: index.html, main.js, styles.css
src/format.js         pure formatting (reset times, ages, thresholds) — vitest
src/camel.png         master icon; `npx tauri icon src/camel.png` rebuilds icons/
src-tauri/src/
  lib.rs              setup, tray, panel window, commands, pollers
  limits.rs           reads/parses ~/.claude/statusline-last.json — cargo tests
  tray_icon.rs        runtime-drawn bar icon + update badge — cargo tests
  private.rs          0600/0700 file writes (the debug log goes through it)
  debug_log.rs        fresh-per-launch event log in the app data dir
```

## Data source

`$HOME/.claude/statusline-last.json`, written by the user's Claude Code
statusline script. Camel polls it every 30 s (mtime first), interprets
`rate_limits.{five_hour,seven_day}.{used_percentage,resets_at}` and shows
*remaining* percent. A window whose `resets_at` is already in the past renders
as full — the quota refreshed, only no session has rewritten the file since.

Missing file or `rate_limits: null` → grey bars and the panel's empty state,
never zeros.

## Run & test

```
npm install
npm test                             # vitest: src/format.test.js
cargo test --lib                     # in src-tauri: parsing, thresholds, icon, file modes
npm run tauri dev
```

The tray title (worst-window percent) is macOS-only; Windows gets the icon and
tooltip.

## Screenshots

Taken by the app's own code, not mocked up:

- Panel states: `_camel_shot.mjs` (web_eye harness) serves `src/` over http,
  stubs `window.__TAURI__` and shoots `panel / low / update / empty`.
- Tray strip: `cargo test -- --ignored dump_icons` writes the real rendered
  RGBA icons to `target/icon-dump/`; a small PIL script composes them onto a
  menu-bar background.

## CI / release

Every push to `main` is a release: the `version` job bumps the patch in
`tauri.conf.json` + `Cargo.toml`, tags, and the platform jobs build and upload.
macOS builds run after Windows and per-arch in sequence — `latest.json` is
read-modify-written by each upload and parallel writers drop platform keys.
`Verify updater manifest` fails the run if a platform is missing or a
universal bundle sneaks in.

Signing: self-signed "Camel Code Signing" cert (no notarization). The updater
manifest is signed with the repo's minisign key (`TAURI_SIGNING_PRIVATE_KEY`
secret); the public half sits in `tauri.conf.json`.

## Logs

`~/Library/Application Support/camel/debug.log` (macOS) — events only, wiped
on every launch.
