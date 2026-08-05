// Panel logic: ask Rust for the snapshot, paint two rows, stay current while
// visible. All numbers come pre-interpreted (remaining, not used) — the panel
// only formats.

import { age, levelOf, resetIn, resetWhen } from './format.js';

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const LEVELS = ['ok', 'low', 'critical'];

function paintRow(rowId, win, now) {
  const row = document.getElementById(rowId);
  const lvl = levelOf(win.remaining);
  const left = row.querySelector('.left');
  left.textContent = `${win.remaining}%`;
  const fill = row.querySelector('.bar span');
  fill.style.width = `${win.remaining}%`;
  for (const el of [left, fill]) {
    el.classList.remove(...LEVELS);
    el.classList.add(lvl);
  }
  row.querySelector('.when').textContent = resetWhen(win.resets_at, now);
  row.querySelector('.in').textContent = resetIn(win.resets_at, now);
}

async function render() {
  const data = await invoke('get_limits');
  const s = data.snapshot;
  document.getElementById('rows').hidden = !s;
  document.getElementById('empty').hidden = !!s;
  if (s) {
    paintRow('row-five', s.five_hour, data.now);
    paintRow('row-seven', s.seven_day, data.now);
    document.getElementById('age').textContent = `updated ${age(s.read_at, data.now)}`;
  } else {
    document.getElementById('age').textContent = '';
  }
  document.getElementById('version').textContent = `Camel v${data.version}`;
  const update = document.getElementById('update');
  update.hidden = !data.update;
  if (data.update) update.textContent = `Update to v${data.update} — install`;
}

document.getElementById('update').addEventListener('click', () => {
  invoke('install_update').catch((e) => invoke('js_log', { message: `install failed: ${e}` }));
});

document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') invoke('hide_panel');
});

listen('limits-changed', render);
listen('update-available', render);
listen('panel-opened', render);

render().catch((e) => invoke('js_log', { message: `render failed: ${e}` }));
