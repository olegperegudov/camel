# Changelog

## 0.1.7 — 2026-08-06

Design critique of the panel (`/impeccable critique`, scored 20/40) turned up
three things the panel was getting wrong at exactly the moments it is trusted
most. All three are fixed here.

- **An empty tank drew nothing.** The fill had no minimum width, so 0% left a
  bare track that reads as a rendering failure rather than "you are out". The
  tray icon has guarded this since 0.1.0; the panel now does too.
- **A refilled window was shown as a future reset.** When a window's reset
  moment passes and no session has run since, the file still carries the old
  timestamp. The panel printed it as an upcoming reset with a countdown frozen
  at "in 0 min" — every morning, in confident green. Rust now flags the window
  as refilled and the panel says "refilled today at 09:00" with no countdown.
- **The empty state told first-timers something false.** "Talk to Claude Code
  once and the bars appear" is only true once the status line writes the file.
  The panel now names the file, says the status line has to write it, and has
  a button that opens the setup guide. A file we cannot parse gets its own
  sentence instead of sharing the "no data" screen.
- Contrast: the critical percentage (4.22:1) and the freshness line (4.28:1)
  both sat below WCAG AA — the two elements most needed under time pressure.
  Repalletted to 6.6:1 and 5.9:1, and the bar track lightened so every fill
  clears 3:1 against it.
- The update button had no visible keyboard focus; both buttons now show a
  focus ring. The bars carry `role="progressbar"` so a screen reader gets the
  geometry, not just the digits.
- One vocabulary across tray, panel and README: the windows are "last 5 hours"
  and "last 7 days" everywhere ("Week" implied a Monday reset on a window that
  rolls), and the percentage now says what it is a percentage of — "73% left".
  Every bar the user has ever seen fills up as things get worse; this one
  drains, and the noun is what disambiguates it.
- The countdown stopped rounding up: 91 minutes read "in 2 h", which hands the
  user 29 minutes of a five-hour budget that don't exist. It now truncates and
  shows both parts — "in 1h 31m".
- The clock follows the machine: a US user with a 12-hour Mac was shown 17:30
  because the format was pinned to 24-hour.

### The panel answers the question it exists for

The tray icon already says how much is left. The panel now says whether the
current pace reaches the reset — the thing a person actually opens it to find
out, and the one answer no generic usage widget could ship.

- Camel re-read the file every 30 seconds and threw every previous reading
  away. It now keeps the last twenty minutes and reads the slope: "On pace to
  run out at 18:12 · 35 min early", "On pace to last the window", or "Idle —
  nothing spent in 12 min". Under three minutes of history it says nothing
  rather than guessing, and a refill starts the history over so a window
  coming back is never read as negative spending.
- The forecast names the window that empties first, so the weekly limit
  running out before the 5-hour one is not a silent surprise.
- `limits::worst()` is gone. It was the old "which window matters" answer, had
  no caller but its own test since the tray title was dropped, and the pace
  forecast is what replaced it.
- The window is sized by the page, not by constants in Rust. The old height
  was maintained by hand against copy it could not measure, and the first
  verdict that wrapped to two lines cut the footer off.

## 0.1.6 — 2026-08-05

- Left click finally opens the panel. The panel is now a non-activating
  NSPanel (the Spotlight mechanism, same as Iago): an ordinary window of a
  menu-bar-only app never reached the screen on macOS 26 — Tauri reported it
  visible while the window server kept it ordered out. Closes on a click
  anywhere outside (global NSEvent monitor).
- Right-click menu slimmed to update / version / quit: "Show limits"
  duplicated the left click and is gone.

## 0.1.5 — 2026-08-05

- Tray is bars only (as designed): the percent moved to the hover tooltip and
  the panel. Red now starts at three quarters spent (was 80%).
- Panel opens reliably: the click handler takes Down or Up (macOS delivers
  either, debounced), and a "Show limits" menu item backs the gesture up.
- Tray click events are logged.

## 0.1.2 — 2026-08-05

- Fix: fractional `used_percentage` (e.g. `7.0`) parsed as no-data — the tray
  fell back to grey bars whenever Claude Code wrote floats. Regression test.

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
