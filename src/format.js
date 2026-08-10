// Pure formatting for the panel: numbers in, short strings out.
// Kept free of Tauri so vitest can chew on it.

// Remaining % at which a window turns yellow / red. Mirrors the Rust side
// (limits.rs): yellow at half spent, red at three quarters.
export const LOW_AT = 50;
export const CRITICAL_AT = 25;

export function levelOf(remaining) {
  if (remaining >= LOW_AT) return 'ok';
  if (remaining >= CRITICAL_AT) return 'low';
  return 'critical';
}

// "47m" / "1h 31m" / "3h" / "4d" — a span of time, as short as it can be said.
//
// Everything truncates rather than rounds: on a five-hour budget, rounding 91
// minutes up to "2h" hands the user 29 minutes that don't exist, and it errs
// in the direction they plan work around.
export function duration(secs) {
  const mins = Math.max(0, Math.floor(secs / 60));
  if (mins < 60) return `${mins}m`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) {
    const rest = mins % 60;
    return rest ? `${hours}h ${rest}m` : `${hours}h`;
  }
  return `${Math.floor(hours / 24)}d`;
}

// What sits to the right of the bar: how long this window still has.
//
// A window that already came back has no next reset written yet — its
// resets_at names the refill behind us, and counting down to it printed a
// confident "0m" every morning. A dash says "nothing scheduled" instead.
export function resetLabel(win, nowSecs) {
  return win.refilled ? '—' : duration(win.resets_at - nowSecs);
}

// Past this, a reading stops describing the session in front of the user: the
// status line writes on every turn, so half an hour of silence means the
// numbers belong to work that has already moved on.
export const STALE_AFTER_SECS = 30 * 60;

// The line under the bars, or nothing at all. Freshness is only worth the
// user's attention when it has run out — a panel that swears it is current on
// every open is noise carrying no decision.
export function staleNote(readAtSecs, nowSecs) {
  if (nowSecs - readAtSecs < STALE_AFTER_SECS) return null;
  return `numbers last read ${age(readAtSecs, nowSecs)}`;
}

// How stale the numbers are: "just now" / "5 min ago" / "3 h ago" / "2 d ago".
export function age(readAtSecs, nowSecs) {
  const secs = Math.max(0, nowSecs - readAtSecs);
  if (secs < 90) return 'just now';
  const mins = Math.round(secs / 60);
  if (mins < 60) return `${mins} min ago`;
  const hours = Math.round(mins / 60);
  if (hours < 24) return `${hours} h ago`;
  return `${Math.round(hours / 24)} d ago`;
}
