import { describe, expect, test } from 'vitest';
import { age, duration, levelOf, resetLabel } from './format.js';

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

describe('duration', () => {
  test('minutes under an hour, hours and minutes under a day, then days', () => {
    expect(duration(40 * 60)).toBe('40m');
    expect(duration(3 * 3600)).toBe('3h');
    expect(duration(3 * 86400)).toBe('3d');
  });
  test('an hour and a half is not two hours', () => {
    // Rounding 91 minutes up handed the user 29 minutes of a five-hour
    // budget that were never there.
    expect(duration(91 * 60)).toBe('1h 31m');
    expect(duration(59 * 60)).toBe('59m');
  });
  test('nothing rounds up: a countdown never promises time it does not have', () => {
    expect(duration(59)).toBe('0m');
    expect(duration(2 * 86400 - 60)).toBe('1d');
  });
});

describe('resetLabel', () => {
  test('a live window counts down to its reset', () => {
    expect(resetLabel({ remaining: 62, resets_at: NOW + 2 * 3600, refilled: false }, NOW)).toBe('2h');
  });
  test('a reset in the past does not go negative', () => {
    expect(resetLabel({ remaining: 3, resets_at: NOW - 600, refilled: false }, NOW)).toBe('0m');
  });
  test('a window that already came back has nothing to count down to', () => {
    // Its resets_at names the refill behind us; a countdown to it printed a
    // confident "0m" every morning.
    expect(resetLabel({ remaining: 100, resets_at: NOW - 3600, refilled: true }, NOW)).toBe('—');
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
