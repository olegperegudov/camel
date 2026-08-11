# Agent notes

Camel is a menu-bar app showing Claude Code usage limits. Tauri 2: Rust owns
all state, the webview only paints. No frontend framework, no bundler — `src/`
is served as-is.

Start here: **[docs/DEVELOPMENT.md](docs/DEVELOPMENT.md)** — layout, data
source, window behaviour, icons, CI and signing.

Quick facts:

- Run it: `npm install && npm run tauri dev`
- Tests: `npm test` (vitest, pure JS formatting) and `cargo test --lib` in `src-tauri/`
- The only data source is `~/.claude/statusline-last.json`; the app makes no
  other reads and no network calls beyond its own update check.
- Versions are bumped by CI on push to `main` — do not edit the version in
  `src-tauri/tauri.conf.json` or `Cargo.toml` by hand.
- User-visible changes go in `CHANGELOG.md` under `## Unreleased`, one plain
  bullet per change; CI cuts that section into the release notes.
