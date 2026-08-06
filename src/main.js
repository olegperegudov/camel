// Panel logic: ask Rust for the reading, paint two rows, stay current while
// visible. All numbers come pre-interpreted (remaining, not used) — the panel
// only formats.

import { age, levelOf, refilledWhen, resetIn, resetWhen } from './format.js';

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const LEVELS = ['ok', 'low', 'critical'];

// What the empty state says, per reason. A missing file and a file we cannot
// read send the user to two different places, so they get two different lines.
const NO_DATA = {
  missing: {
    head: 'No limit data yet.',
    how: 'Camel reads ~/.claude/statusline-last.json — your Claude Code status line has to write it. Two lines do it.',
  },
  unreadable: {
    head: 'Found the file, but no limits in it.',
    how: 'Camel expects rate_limits.five_hour in what the status line writes. If the status line changed shape, the snippet needs updating.',
  },
};

function paintRow(rowId, win, now) {
  const row = document.getElementById(rowId);
  const lvl = levelOf(win.remaining);
  const left = row.querySelector('.left');
  left.querySelector('.pct').textContent = `${win.remaining}%`;
  const bar = row.querySelector('.bar');
  bar.setAttribute('aria-valuenow', win.remaining);
  bar.setAttribute('aria-valuetext', `${win.remaining}% left`);
  const fill = row.querySelector('.bar span');
  fill.style.width = `${win.remaining}%`;
  for (const el of [left, fill]) {
    el.classList.remove(...LEVELS);
    el.classList.add(lvl);
  }
  // A refilled window has no future reset to count down to: its resets_at is
  // the moment it came back, and nothing has written the next one yet.
  row.querySelector('.when').textContent = win.refilled
    ? refilledWhen(win.resets_at, now)
    : resetWhen(win.resets_at, now);
  row.querySelector('.in').textContent = win.refilled ? '' : resetIn(win.resets_at, now);
}

async function render() {
  const data = await invoke('get_limits');
  const reading = data.reading;
  const s = reading.state === 'ok' ? reading.snapshot : null;
  document.getElementById('rows').hidden = !s;
  document.getElementById('empty').hidden = !!s;
  if (s) {
    paintRow('row-five', s.five_hour, data.now);
    paintRow('row-seven', s.seven_day, data.now);
    document.getElementById('age').textContent = `updated ${age(s.read_at, data.now)}`;
  } else {
    const copy = NO_DATA[reading.state] ?? NO_DATA.missing;
    document.getElementById('empty-head').textContent = copy.head;
    document.getElementById('empty-how').textContent = copy.how;
    document.getElementById('age').textContent = '';
  }
  document.getElementById('version').textContent = `Camel v${data.version}`;
  const update = document.getElementById('update');
  update.hidden = !data.update;
  // Same words as the tray menu item: one action, one name for it.
  if (data.update) update.textContent = `Update to v${data.update}`;
}

document.getElementById('update').addEventListener('click', () => {
  invoke('install_update').catch((e) => invoke('js_log', { message: `install failed: ${e}` }));
});

document.getElementById('setup').addEventListener('click', () => {
  invoke('open_setup_guide');
});

document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') invoke('hide_panel');
});

listen('limits-changed', render);
listen('update-available', render);
listen('panel-opened', render);

render().catch((e) => invoke('js_log', { message: `render failed: ${e}` }));
