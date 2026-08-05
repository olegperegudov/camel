# Changelog

## 0.1.0 — 2026-08-05

First release.

- Menu-bar icon: two runtime-drawn bars (5-hour window, week), each coloured
  by its own remaining share (green ≥50, yellow ≥20, red below), worst-window
  percent as the tray title. Grey bars when there is no data yet.
- Click panel: both windows with remaining %, reset times ("resets today at
  17:30 · in 3 h"), data freshness and app version. Sized to content; grows
  only when an update row appears.
- Data source: `~/.claude/statusline-last.json` (written by the user's Claude
  Code statusline script), polled every 30 s. A window past its reset shows
  full, not stale numbers.
- Updater: green badge on the icon + right-click menu item + install row in
  the panel. Background check 5 s after launch, then every 30 min.
- Tests: parsing/thresholds/reset rollover (cargo), icon pixels incl. the
  green badge (cargo), file modes 0600/0700 (cargo), time formatting (vitest).
