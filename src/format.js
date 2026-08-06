// Pure formatting for the panel: numbers in, English phrases out.
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

// Whether this machine writes 5:30 PM or 17:30. The weekday stays English —
// the rest of the app is — but the clock is the user's own setting, and
// pinning it to 24-hour showed a US user a convention they don't use.
export function systemUses12Hour() {
  const cycle = new Intl.DateTimeFormat().resolvedOptions().hourCycle;
  return cycle === 'h11' || cycle === 'h12';
}

// "17:30" or "05:30 pm", depending on how this machine tells time.
export function clock(secs, hour12 = systemUses12Hour()) {
  return new Date(secs * 1000)
    .toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit', hour12 })
    .replace(/\s?([AP]M)$/i, (_, ap) => ` ${ap.toLowerCase()}`);
}

// "today at 17:30" / "Thu at 01:00" — the day is named only when it isn't today.
function dayTime(secs, nowSecs, hour12) {
  const at = new Date(secs * 1000);
  const now = new Date(nowSecs * 1000);
  const hm = clock(secs, hour12);
  if (at.toDateString() === now.toDateString()) return `today at ${hm}`;
  return `${at.toLocaleDateString('en-US', { weekday: 'short' })} at ${hm}`;
}

// "resets today at 17:30" / "resets Thu at 01:00".
export function resetWhen(resetsAtSecs, nowSecs, hour12 = systemUses12Hour()) {
  return `resets ${dayTime(resetsAtSecs, nowSecs, hour12)}`;
}

// The window already came back and no session has written the next reset yet,
// so the same timestamp names a refill in the past, not an event ahead.
export function refilledWhen(resetsAtSecs, nowSecs, hour12 = systemUses12Hour()) {
  return `refilled ${dayTime(resetsAtSecs, nowSecs, hour12)}`;
}

// "47 min" / "1h 31m" / "4 d" — a span of time, wherever one is shown.
//
// Everything truncates rather than rounds: on a five-hour budget, rounding 91
// minutes up to "2 h" hands the user 29 minutes that don't exist, and it errs
// in the direction they plan work around.
export function duration(secs) {
  const mins = Math.max(0, Math.floor(secs / 60));
  if (mins < 60) return `${mins} min`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) {
    const rest = mins % 60;
    return rest ? `${hours}h ${rest}m` : `${hours}h`;
  }
  return `${Math.floor(hours / 24)} d`;
}

// The countdown beside the reset time.
export function resetIn(resetsAtSecs, nowSecs) {
  return `in ${duration(resetsAtSecs - nowSecs)}`;
}

// "at 13:39" today, "Thu at 01:00" once it crosses midnight.
function shortWhen(secs, nowSecs, hour12) {
  const at = new Date(secs * 1000);
  const sameDay = at.toDateString() === new Date(nowSecs * 1000).toDateString();
  const day = sameDay ? '' : `${at.toLocaleDateString('en-US', { weekday: 'short' })} `;
  return `${day}at ${clock(secs, hour12)}`;
}

// The panel's one conclusion: not "how much is left" — the tray icon says that
// — but whether the current pace reaches the reset. `aside` carries the size
// of the miss, in the same two-column rhythm as the reset row below it.
export function paceLine(pace, nowSecs, hour12 = systemUses12Hour()) {
  switch (pace.state) {
    case 'idle':
      return { text: `Idle — nothing spent in ${pace.minutes} min`, aside: '', level: 'calm' };
    case 'safe':
      return { text: 'On pace to last the window', aside: '', level: 'calm' };
    case 'runs_out': {
      const what = pace.window === 'seven_day' ? 'Weekly limit runs out' : 'On pace to run out';
      return {
        text: `${what} ${shortWhen(pace.at, nowSecs, hour12)}`,
        aside: `${duration(pace.before_reset)} early`,
        level: 'warn',
      };
    }
    default:
      return null;
  }
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
