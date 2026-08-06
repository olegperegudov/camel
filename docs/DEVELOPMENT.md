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
*remaining* percent.

Reading the file has three outcomes, not two (`limits::Reading`): `Ok`,
`Missing` (no file — the status line has never written one) and `Unreadable`
(a file with nothing we can parse). The panel says something different for
each; collapsing the last two into one screen is how a first-time user ends up
following instructions that cannot work.

A window whose `resets_at` has passed is reported full and flagged `refilled`:
the quota did come back, and the timestamp in the file now names that refill
rather than a future event. Without the flag the panel counted down to a
moment in the past, forever, in confident green.

## What the panel draws

One row per window: the name (`5h` / `7d`), then a coloured line whose length
is the remaining share, then how long that share has to last. There is no
track behind the line — empty capacity is not information, and drawing it made
every reading two shapes to compare instead of one length to look at.

Length is set as `--pct` on the element; how long a full line is (`--track`)
lives in the stylesheet next to the window's own width, so the two cannot
drift apart.

A burn-rate forecast lived here until 0.1.11 (`pace.rs`, a 20-minute sample
buffer, a verdict line above the rows). It was correct and nobody read it: the
panel is opened to see how much is left and how long it has to last, and every
extra line sat between the eye and that. Removed with its line — see git
history if it ever earns a way back.

## Panel size

The window takes the height the page reports (`contentHeight()` → the
`fit_panel` command). The constants in `lib.rs` are only a first guess so the
window opens at roughly the right size. Height kept by hand in Rust is how the
footer got cut off the first time a verdict wrapped to two lines: the page has
`overflow: hidden` and no scrollbar, so clipping is silent.

## Colour

Two palettes on purpose. The panel draws its own dark surface, so its text and
fills are tuned against `#1b1f26` — the critical percentage and the freshness
line both sat below WCAG AA before and were repalletted. The tray icon sits on
a menu bar that may be light or dark, so it keeps mid-tone colours that survive
both. Aligning them would cost legibility on a light menu bar.

The bar has no track to contrast against, so each fill is read against the
card itself — green, yellow and red all clear 3:1 on `#1b1f26`, which is the
WCAG 1.4.11 bar for a graphic carrying meaning.

## Run & test

```
npm install
npm test                             # vitest: src/format.test.js
cargo test --lib                     # in src-tauri: parsing, thresholds, icon, file modes
npm run tauri dev
```

The tray shows bars only; exact percents live in the tooltip and the panel.

## Screenshots

Taken by the app's own code, not mocked up:

- Panel states: `_camel_shot.mjs` (web_eye harness) serves `src/` over its own
  http server, stubs `window.__TAURI__`, sizes the viewport with the app's own
  `contentHeight()` and shoots
  `panel / low / zero / refilled / update / empty / unreadable`.
  ```
  cd ~/membeme/system/tools/web_eye
  SHOT=low OUT=~/pets/camel/docs/screenshots/panel-rows-low.png node _camel_shot.mjs
  ```
- Tray strip: `cargo test -- --ignored dump_icons` writes the real rendered
  RGBA icons to `target/icon-dump/`; a small PIL script composes them onto a
  menu-bar background.

Two rules learned the hard way:

- **Fixture times are relative to `now`, never wall-clock.** An absolute
  "17:30" rolls to tomorrow when the shot is taken in the evening, and the
  README then showed a 5-hour window resetting 23 hours out. The harness
  asserts the 5-hour reset is within five hours.
- **A replaced screenshot needs a new filename.** GitHub serves images through
  a caching proxy keyed by URL, so new pixels under an old name keep showing
  the old picture for weeks.

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
