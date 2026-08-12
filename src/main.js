// Panel logic: ask Rust for the reading, paint a group per account, stay
// current while visible. All numbers come pre-interpreted (remaining, not
// used) — the panel only formats.

import { levelOf, resetLabel, staleNote } from './format.js';

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const LEVELS = ['ok', 'low', 'critical'];
const WINDOWS = [
  { key: 'five_hour', name: '5h', spoken: 'last 5 hours' },
  { key: 'seven_day', name: '7d', spoken: 'last 7 days' },
];

// What the empty state says, per reason. A missing file and a file we cannot
// read send the user to two different places, so they get two different lines.
const NO_DATA = {
  missing: {
    head: 'No limit data yet.',
    how: 'Camel reads statusline-last.json next to your Claude Code config — the status line has to write it. Two lines do it.',
  },
  unreadable: {
    head: 'Found the file, but no limits in it.',
    how: 'Camel expects rate_limits.five_hour in what the status line writes. If the status line changed shape, the snippet needs updating.',
  },
};

function row(win, spoken, name, now) {
  const el = document.getElementById('account-row').content.firstElementChild.cloneNode(true);
  const bar = el.querySelector('.bar');
  // The length is set as a share, not a pixel count: how long a full bar is
  // belongs to the stylesheet, next to the window's own width.
  bar.style.setProperty('--pct', win.remaining);
  bar.classList.add(levelOf(win.remaining));
  bar.setAttribute('aria-label', spoken);
  bar.setAttribute('aria-valuenow', win.remaining);
  bar.setAttribute('aria-valuetext', `${win.remaining}% left`);
  el.querySelector('.name').textContent = name;
  el.querySelector('.until').textContent = resetLabel(win, now);
  return el;
}

// One account: its name, its two windows, and a freshness note of its own.
// Freshness is per account on purpose — the work login can sit untouched for a
// day while the personal one is mid-session, and one shared note would lie
// about both.
function group(account, now, alone) {
  const box = document.createElement('section');
  box.className = 'account';

  // A single login needs no heading: naming it "personal" when there is
  // nothing to tell it apart from is a label doing no work.
  if (!alone) {
    const label = document.createElement('span');
    label.className = 'who';
    label.textContent = account.label;
    box.append(label);
  }

  const snapshot = account.reading.state === 'ok' ? account.reading.snapshot : null;
  if (!snapshot) {
    const line = document.createElement('p');
    line.className = 'quiet';
    line.textContent =
      account.reading.state === 'unreadable' ? 'Limits not readable.' : 'No sessions yet.';
    box.append(line);
    return box;
  }

  for (const w of WINDOWS) box.append(row(snapshot[w.key], w.spoken, w.name, now));

  const note = staleNote(snapshot.read_at, now);
  if (note) {
    const stale = document.createElement('p');
    stale.className = 'stale';
    stale.textContent = note;
    box.append(stale);
  }
  return box;
}

async function render() {
  const data = await invoke('get_limits');
  const accounts = data.accounts ?? [];
  const box = document.getElementById('accounts');
  box.hidden = !accounts.length;
  document.getElementById('empty').hidden = !!accounts.length;

  if (accounts.length) {
    box.replaceChildren(...accounts.map((a) => group(a, data.now, accounts.length === 1)));
  } else {
    // Nothing anywhere: not even a config that wrote a file once.
    const copy = NO_DATA.missing;
    document.getElementById('empty-head').textContent = copy.head;
    document.getElementById('empty-how').textContent = copy.how;
  }
  invoke('fit_panel', { height: contentHeight() });
}

// The window takes the height of what the panel actually contains. It used to
// be a constant kept by hand in Rust, and the first line of copy that wrapped
// was clipped silently — there is no scrollbar here to reveal it.
export function contentHeight() {
  const panel = document.querySelector('.panel');
  const css = getComputedStyle(panel);
  const px = (v) => parseFloat(v) || 0;
  const kids = [...panel.children].filter((el) => !el.hidden);
  const stack = kids.reduce((sum, el) => sum + el.getBoundingClientRect().height, 0);
  const gaps = px(css.rowGap) * Math.max(0, kids.length - 1);
  const frame =
    px(css.paddingTop) + px(css.paddingBottom) + px(css.borderTopWidth) + px(css.borderBottomWidth);
  return Math.ceil(stack + gaps + frame);
}

document.getElementById('setup').addEventListener('click', () => {
  invoke('open_setup_guide');
});

document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') invoke('hide_panel');
});

listen('limits-changed', render);
listen('panel-opened', render);

render().catch((e) => invoke('js_log', { message: `render failed: ${e}` }));
