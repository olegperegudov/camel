import { describe, expect, test } from 'vitest';
import { age, levelOf, paceLine, refilledWhen, resetIn, resetWhen } from './format.js';

// Wed 2026-08-05 14:32 local — an anchor mid-day, away from midnight edges.
const NOW = Math.floor(new Date(2026, 7, 5, 14, 32).getTime() / 1000);

describe('levelOf mirrors the status line thresholds', () => {
  test('50 and up is fine, 25–49 is low, below 25 is critical', () => {
    expect(levelOf(100)).toBe('ok');
    expect(levelOf(50)).toBe('ok');
    expect(levelOf(49)).toBe('low');
    expect(levelOf(25)).toBe('low');
    expect(levelOf(24)).toBe('critical');
  });
});

describe('resetWhen', () => {
  const at1730 = Math.floor(new Date(2026, 7, 5, 17, 30).getTime() / 1000);

  test('same day says today', () => {
    expect(resetWhen(at1730, NOW, false)).toBe('resets today at 17:30');
  });
  test('another day names the weekday', () => {
    const thu0100 = Math.floor(new Date(2026, 7, 6, 1, 0).getTime() / 1000);
    expect(resetWhen(thu0100, NOW, false)).toBe('resets Thu at 01:00');
  });
  test('a 12-hour machine gets a 12-hour clock', () => {
    expect(resetWhen(at1730, NOW, true)).toBe('resets today at 05:30 pm');
  });
});

describe('refilledWhen', () => {
  test('a window that already came back names the refill, not a reset', () => {
    const earlier = Math.floor(new Date(2026, 7, 5, 9, 0).getTime() / 1000);
    expect(refilledWhen(earlier, NOW)).toBe('refilled today at 09:00');
    const yesterday = Math.floor(new Date(2026, 7, 4, 22, 33).getTime() / 1000);
    expect(refilledWhen(yesterday, NOW)).toBe('refilled Tue at 22:33');
  });
});

describe('resetIn', () => {
  test('minutes under an hour, hours and minutes under a day, then days', () => {
    expect(resetIn(NOW + 40 * 60, NOW)).toBe('in 40 min');
    expect(resetIn(NOW + 3 * 3600, NOW)).toBe('in 3h');
    expect(resetIn(NOW + 3 * 86400, NOW)).toBe('in 3 d');
  });
  test('an hour and a half is not two hours', () => {
    // Rounding 91 minutes up handed the user 29 minutes of a five-hour
    // budget that were never there.
    expect(resetIn(NOW + 91 * 60, NOW)).toBe('in 1h 31m');
    expect(resetIn(NOW + 59 * 60, NOW)).toBe('in 59 min');
  });
  test('nothing rounds up: a countdown never promises time it does not have', () => {
    expect(resetIn(NOW + 59, NOW)).toBe('in 0 min');
    expect(resetIn(NOW + (2 * 86400 - 60), NOW)).toBe('in 1 d');
  });
  test('a reset in the past does not go negative', () => {
    expect(resetIn(NOW - 600, NOW)).toBe('in 0 min');
  });
});

describe('paceLine', () => {
  const at1812 = Math.floor(new Date(2026, 7, 5, 18, 12).getTime() / 1000);

  test('a pace that runs out names the clock time and how early', () => {
    expect(
      paceLine({ state: 'runs_out', window: 'five_hour', at: at1812, before_reset: 40 * 60 }, NOW, false)
    ).toEqual({ text: 'On pace to run out at 18:12', aside: '40 min early', level: 'warn' });
  });
  test('the weekly window says so, and names the day when it is not today', () => {
    const thu = Math.floor(new Date(2026, 7, 6, 9, 15).getTime() / 1000);
    expect(paceLine({ state: 'runs_out', window: 'seven_day', at: thu, before_reset: 2 * 86400 }, NOW, false))
      .toEqual({ text: 'Weekly limit runs out Thu at 09:15', aside: '2 d early', level: 'warn' });
  });
  test('safe and idle are calm, and say something rather than nothing', () => {
    expect(paceLine({ state: 'safe' }, NOW).level).toBe('calm');
    expect(paceLine({ state: 'idle', minutes: 12 }, NOW).text).toBe('Idle — nothing spent in 12 min');
  });
  test('too little history shows no line at all', () => {
    expect(paceLine({ state: 'unknown' }, NOW)).toBeNull();
  });
});

describe('age', () => {
  test('fresh, minutes, hours, days', () => {
    expect(age(NOW - 30, NOW)).toBe('just now');
    expect(age(NOW - 5 * 60, NOW)).toBe('5 min ago');
    expect(age(NOW - 3 * 3600, NOW)).toBe('3 h ago');
    expect(age(NOW - 2 * 86400, NOW)).toBe('2 d ago');
  });
});
