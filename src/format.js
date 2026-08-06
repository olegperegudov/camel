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

// "today at 17:30" / "Thu at 01:00" — the day is named only when it isn't today.
function dayTime(secs, nowSecs) {
  const at = new Date(secs * 1000);
  const now = new Date(nowSecs * 1000);
  const hm = at.toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit', hour12: false });
  if (at.toDateString() === now.toDateString()) return `today at ${hm}`;
  return `${at.toLocaleDateString('en-US', { weekday: 'short' })} at ${hm}`;
}

// "resets today at 17:30" / "resets Thu at 01:00".
export function resetWhen(resetsAtSecs, nowSecs) {
  return `resets ${dayTime(resetsAtSecs, nowSecs)}`;
}

// The window already came back and no session has written the next reset yet,
// so the same timestamp names a refill in the past, not an event ahead.
export function refilledWhen(resetsAtSecs, nowSecs) {
  return `refilled ${dayTime(resetsAtSecs, nowSecs)}`;
}

// "in 42 min" / "in 3 h" / "in 2 d" — the countdown next to the reset time.
export function resetIn(resetsAtSecs, nowSecs) {
  const mins = Math.max(0, Math.round((resetsAtSecs - nowSecs) / 60));
  if (mins < 60) return `in ${mins} min`;
  const hours = Math.round(mins / 60);
  if (hours < 48) return `in ${hours} h`;
  return `in ${Math.round(hours / 24)} d`;
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
