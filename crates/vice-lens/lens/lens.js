// Vice Lens: the browser side. No framework, no build step: the shell
// (activity bar, side panel, tabs, inspector, status bar, palette) and the
// views (map, model, archive, bytes, text, setup).

// ---------------------------------------------------------------- helpers

const $ = (s, el = document) => el.querySelector(s);

function h(tag, props = {}, ...kids) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(props || {})) {
    if (v == null || v === false) continue;
    if (k === 'class') el.className = v;
    else if (k === 'text') el.textContent = v;
    else if (k === 'style') el.style.cssText = v;
    else if (k.startsWith('on')) el.addEventListener(k.slice(2), v);
    else el.setAttribute(k, v === true ? '' : v);
  }
  for (const c of kids.flat()) if (c != null && c !== false) el.append(c.nodeType ? c : document.createTextNode(String(c)));
  return el;
}

const ICONS = {
  map: '<path d="M9 4 3 6v14l6-2 6 2 6-2V4l-6 2-6-2z"/><path d="M9 4v14M15 6v14"/>',
  cube: '<path d="M12 3 4 7.5v9L12 21l8-4.5v-9L12 3z"/><path d="M4 7.5 12 12l8-4.5M12 12v9"/>',
  files: '<path d="M4 5h6l2 2h8v12H4z"/><path d="M4 10h16"/>',
  text: '<path d="M5 6h14M12 6v13M8 19h8"/>',
  gear: '<circle cx="12" cy="12" r="3"/><path d="M12 2v3M12 19v3M4.2 4.2l2.1 2.1M17.7 17.7l2.1 2.1M2 12h3M19 12h3M4.2 19.8l2.1-2.1M17.7 6.3l2.1-2.1"/>',
  search: '<circle cx="11" cy="11" r="6"/><path d="m20 20-4.5-4.5"/>',
  panel: '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M15 4v16"/>',
  plus: '<path d="M12 5v14M5 12h14"/>',
  minus: '<path d="M5 12h14"/>',
  fit: '<path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5"/>',
  pin: '<path d="M12 21s-6-5.5-6-11a6 6 0 1 1 12 0c0 5.5-6 11-6 11z"/><circle cx="12" cy="10" r="2"/>',
  x: '<path d="M6 6l12 12M18 6 6 18"/>',
  hex: '<path d="M7 8 3 12l4 4M17 8l4 4-4 4M14 5l-4 14"/>',
  grid: '<path d="M4 9h16M4 15h16M9 4v16M15 4v16"/>',
  layers: '<path d="m12 3 9 5-9 5-9-5 9-5z"/><path d="m3 13 9 5 9-5"/>',
  copy: '<rect x="8" y="8" width="12" height="12" rx="2"/><path d="M16 8V5a1 1 0 0 0-1-1H5a1 1 0 0 0-1 1v10a1 1 0 0 0 1 1h3"/>',
  image: '<rect x="3" y="4" width="18" height="16" rx="2"/><circle cx="9" cy="10" r="2"/><path d="m21 16-5-5-9 9"/>',
  film: '<rect x="3" y="5" width="18" height="14" rx="2"/><path d="M7 5v14M17 5v14M3 9h4M3 15h4M17 9h4M17 15h4"/>',
  play: '<path d="M8 5v14l11-7z"/>',
  globe: '<circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18"/>',
  save: '<path d="M12 4v11M7 10l5 5 5-5M5 19h14"/>',
  folder: '<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>',
  dot: '<circle cx="12" cy="12" r="1.5"/>',
  wave: '<path d="M3 12h2M7 8v8M11 5v14M15 9v6M19 7v10M21 12h0"/>',
  chevron: '<path d="m6 9 6 6 6-6"/>',
  check: '<path d="m5 12 5 5 9-10"/>',
  prev: '<path d="M18 6 9 12l9 6zM6 6v12"/>',
  next: '<path d="m6 6 9 6-9 6zM18 6v12"/>',
  loop: '<path d="M17 2l3 3-3 3"/><path d="M4 11V9a4 4 0 0 1 4-4h12M7 22l-3-3 3-3"/><path d="M20 13v2a4 4 0 0 1-4 4H4"/>',
  export: '<path d="M12 15V3M7 8l5-5 5 5M5 14v5h14v-5"/>',
  swatch: '<circle cx="8" cy="9" r="3"/><circle cx="16" cy="9" r="3"/><circle cx="12" cy="16" r="3"/>',
  car: '<path d="M5 16V12l2-5h10l2 5v4"/><path d="M3 16h18v2H3z"/><circle cx="7.5" cy="18.5" r="1.5"/><circle cx="16.5" cy="18.5" r="1.5"/><path d="M6 12h12"/>',
  spin: '<path d="M20 12a8 8 0 1 1-2.3-5.6"/><path d="M20 4v5h-5"/>',
  person: '<circle cx="12" cy="6" r="3"/><path d="M6 21v-6a6 6 0 0 1 12 0v6"/><path d="M9 12v9M15 12v9"/>',
  paw: '<circle cx="7" cy="9" r="1.8"/><circle cx="11" cy="6" r="1.8"/><circle cx="15" cy="6" r="1.8"/><circle cx="19" cy="9" r="1.8"/><path d="M13 12c-3 0-6 3.5-6 6 0 1.5 1.5 2.5 3 2 1-.3 2-.7 3-.7s2 .4 3 .7c1.5.5 3-.5 3-2 0-2.5-3-6-6-6z"/>',
};

function icon(name) {
  const s = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  s.setAttribute('viewBox', '0 0 24 24');
  s.setAttribute('class', 'i');
  s.innerHTML = ICONS[name] || '';
  return s;
}

async function api(path, params = {}, opts = {}) {
  const u = new URL(path, location.origin);
  for (const [k, v] of Object.entries(params)) if (v != null) u.searchParams.set(k, v);
  const r = await fetch(u, opts);
  const j = await r.json().catch(() => ({}));
  if (!r.ok) throw new Error(j.error || r.statusText);
  return j;
}

async function bin(path, params = {}) {
  const u = new URL(path, location.origin);
  for (const [k, v] of Object.entries(params)) if (v != null) u.searchParams.set(k, v);
  const r = await fetch(u);
  if (!r.ok) throw new Error((await r.json().catch(() => ({}))).error || r.statusText);
  return r.arrayBuffer();
}

const fmtNum = (n) => Number(n).toLocaleString('en-US');
function fmtBytes(n) {
  const u = ['B', 'KB', 'MB', 'GB', 'TB'];
  let i = 0;
  while (n >= 1024 && i < u.length - 1) { n /= 1024; i++; }
  return `${n >= 100 || i === 0 ? n.toFixed(0) : n.toFixed(1)} ${u[i]}`;
}
const fmtM = (m) => (Math.abs(m) >= 1000 ? `${(m / 1000).toFixed(m >= 10000 ? 0 : 1)} km` : `${Math.round(m)} m`);
const fx = (v, d = 1) => Number(v).toFixed(d);

function debounce(fn, ms) {
  let t;
  return (...a) => { clearTimeout(t); t = setTimeout(() => fn(...a), ms); };
}

// A search's options as a RegExp, to mark what the server matched (its
// regular expressions are Rust's, which JavaScript reads the same in all
// but rare cases).
function matcher(p) {
  if (!p?.q) return null;
  let src = p.re ? p.q : p.q.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  if (p.word) src = `\\b(?:${src})\\b`;
  try { return new RegExp(src, p.case ? 'g' : 'gi'); } catch { return null; }
}

// Text with every match of a query (a string, or search options) marked.
function highlight(text, q) {
  const re = matcher(typeof q === 'string' ? { q } : q);
  if (!re) return [text];
  const out = [];
  let last = 0, m, n = 0;
  while ((m = re.exec(text)) && n++ < 40) {
    if (!m[0]) { re.lastIndex++; continue; }
    out.push(text.slice(last, m.index), h('mark', { text: m[0] }));
    last = m.index + m[0].length;
  }
  out.push(text.slice(last));
  return out;
}

// A search field with match options (case, whole words, regular
// expression) and the view's own filters; `run(params)` gets the query and
// every option as request parameters. Remembered per `key`.
function searchBox(key, placeholder, run, { filters = [], value } = {}) {
  const saved = store.get('search.' + key, {});
  const st = { case: !!saved.case, word: !!saved.word, re: !!saved.re, f: { ...(saved.f || {}) } };
  const input = h('input', { class: 'text', placeholder, spellcheck: 'false', value: value ?? saved.q ?? '' });
  const err = h('div', { class: 'err hidden' });
  const params = () => {
    const p = { q: input.value.trim(), case: st.case ? 1 : null, word: st.word ? 1 : null, re: st.re ? 1 : null };
    for (const f of filters) { const v = st.f[f.id] ?? f.options[0][0]; if (v !== '') p[f.id] = v; }
    return p;
  };
  const fire = () => {
    store.set('search.' + key, { q: input.value, case: st.case, word: st.word, re: st.re, f: st.f });
    err.classList.add('hidden');
    run(params());
  };
  const opt = (k, label, title) => {
    const b = h('button', { class: 'opt' + (st[k] ? ' on' : ''), type: 'button', title, onclick: () => { st[k] = !st[k]; b.classList.toggle('on', st[k]); fire(); input.focus(); } }, label);
    return b;
  };
  const sels = filters.map((f) => {
    const s = h('select', { class: 'pick', title: f.title || '' }, ...f.options.map(([v, l]) => h('option', { value: v, text: l })));
    s.value = st.f[f.id] ?? f.options[0][0];
    const mark = () => s.classList.toggle('set', s.value !== f.options[0][0]);
    mark();
    s.addEventListener('change', () => { st.f[f.id] = s.value; mark(); fire(); });
    return s;
  });
  input.addEventListener('input', debounce(fire, 160));
  input.addEventListener('keydown', (e) => { if (e.key === 'Escape' && input.value) { input.value = ''; fire(); e.stopPropagation(); } });
  const el = h('div', { class: 'searchbox' },
    h('div', { class: 'field' }, icon('search'), input,
      h('span', { class: 'opts' }, opt('case', 'Aa', 'Match case'), opt('word', h('u', { text: 'ab' }), 'Whole words only'), opt('re', '.*', 'Regular expression'))),
    sels.length ? h('div', { class: 'filters' }, ...sels) : null, err);
  return { el, input, params, fire, error(msg) { err.textContent = msg || ''; err.classList.toggle('hidden', !msg); } };
}

const fmtT = (s) => { s = Math.max(0, s || 0); const m = Math.floor(s / 60); return `${m}:${String(Math.floor(s % 60)).padStart(2, '0')}`; };
// A name fit for a file.
const fileName = (s) => String(s).replace(/[\\/:*?"<>|\s]+/g, '_').slice(0, 120);

const store = {
  get(k, d) { try { const v = localStorage.getItem('lens.' + k); return v == null ? d : JSON.parse(v); } catch { return d; } },
  set(k, v) { try { localStorage.setItem('lens.' + k, JSON.stringify(v)); } catch { /* private mode */ } },
};

let toastTimer;
function toast(msg) {
  let t = $('#toast');
  if (!t) { t = h('div', { id: 'toast', class: 'glass' }); document.body.append(t); }
  t.textContent = msg;
  t.style.opacity = 1;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (t.style.opacity = 0), 1800);
}

function copy(text) {
  navigator.clipboard?.writeText(text).then(() => toast(`Copied ${text.length > 40 ? 'to clipboard' : text}`), () => toast('Copy failed'));
}

// ------------------------------------------------- context menus, keys

let MENU = null;

function closeMenu() {
  MENU?.el.remove();
  MENU = null;
}

// A menu at the pointer (or at an element, from the keyboard). Items:
// { label, icon, key, run, disabled } or '-' for a separator.
function contextMenu(e, items, at) {
  e?.preventDefault?.();
  e?.stopPropagation?.();
  closeMenu();
  const live = items.filter(Boolean);
  const el = h('div', { class: 'menu glass', role: 'menu' });
  const entries = [];
  for (const it of live) {
    if (it === '-') { el.append(h('div', { class: 'sep' })); continue; }
    const row = h('div', { class: 'mi' + (it.disabled ? ' off' : ''), onclick: () => { if (it.disabled) return; closeMenu(); it.run(); }, onmousemove: () => mark(entries.indexOf(row)) },
      icon(it.icon || 'dot'), h('span', { class: 'l', text: it.label }), it.key ? h('kbd', { text: it.key }) : null);
    entries.push(row);
    row._it = it;
    el.append(row);
  }
  document.body.append(el);
  const r = at?.getBoundingClientRect?.();
  let x = r ? r.left + 24 : e.clientX, y = r ? r.bottom : e.clientY;
  const w = el.offsetWidth, hgt = el.offsetHeight;
  if (x + w > innerWidth - 6) x = innerWidth - w - 6;
  if (y + hgt > innerHeight - 6) y = Math.max(6, y - hgt);
  el.style.left = x + 'px';
  el.style.top = y + 'px';
  let sel = -1;
  const mark = (i) => { sel = i; entries.forEach((r, k) => r.classList.toggle('on', k === i)); };
  MENU = {
    el,
    key(ev) {
      if (ev.key === 'Escape') closeMenu();
      else if (ev.key === 'ArrowDown') { let i = sel; do i = (i + 1) % entries.length; while (entries[i]._it.disabled && i !== sel); mark(i); }
      else if (ev.key === 'ArrowUp') { let i = sel < 0 ? entries.length : sel; do i = (i - 1 + entries.length) % entries.length; while (entries[i]._it.disabled && i !== sel); mark(i); }
      else if (ev.key === 'Enter' && sel >= 0) { const it = entries[sel]._it; closeMenu(); if (!it.disabled) it.run(); }
      else return false;
      ev.preventDefault();
      return true;
    },
  };
  if (at) mark(entries.findIndex((r) => !r._it.disabled));
}

document.addEventListener('pointerdown', (e) => { if (MENU && !MENU.el.contains(e.target)) closeMenu(); }, true);
document.addEventListener('keydown', (e) => { if (MENU && MENU.key(e)) e.stopImmediatePropagation(); }, true);
window.addEventListener('blur', closeMenu);
window.addEventListener('resize', closeMenu);
// The page's own menus replace the web view's (Reload, Inspect…).
document.addEventListener('contextmenu', (e) => { if (!e.target.closest('input, textarea')) e.preventDefault(); });

// Arrow keys over a list or table: up/down, page, home/end; Enter opens;
// Shift+F10 or the menu key opens the row's context menu.
function navigable(box, { rows, open, select, menu, grid }) {
  box.tabIndex = 0;
  box.classList.add('nav');
  box.addEventListener('keydown', (e) => {
    const rs = rows();
    if (!rs.length) return;
    const i = rs.findIndex((r) => r.classList.contains('on'));
    const cols = grid ? Math.max(1, Math.round(box.clientWidth / (rs[0].offsetWidth + 6))) : 1;
    const go = (j) => {
      j = Math.max(0, Math.min(rs.length - 1, j));
      rs.forEach((r) => r.classList.remove('on'));
      rs[j].classList.add('on');
      rs[j].scrollIntoView({ block: 'nearest' });
      select?.(rs[j], j);
    };
    const page = Math.max(1, Math.floor(box.clientHeight / (rs[0].offsetHeight || 28)) - 1) * cols;
    if (e.key === 'ArrowDown') go(i < 0 ? 0 : i + cols);
    else if (e.key === 'ArrowUp') go(i < 0 ? 0 : i - cols);
    else if (grid && e.key === 'ArrowRight') go(i + 1);
    else if (grid && e.key === 'ArrowLeft') go(i - 1);
    else if (e.key === 'PageDown') go(i + page);
    else if (e.key === 'PageUp') go(i - page);
    else if (e.key === 'Home') go(0);
    else if (e.key === 'End') go(rs.length - 1);
    else if (e.key === 'Enter' && i >= 0) open?.(rs[i], i);
    else if ((e.key === 'ContextMenu' || (e.shiftKey && e.key === 'F10')) && i >= 0 && menu) contextMenu(null, menu(rs[i]), rs[i]);
    else return;
    e.preventDefault();
  });
  box.addEventListener('contextmenu', (e) => {
    const row = rows().find((r) => r.contains(e.target));
    if (!row || !menu) return;
    rows().forEach((r) => r.classList.remove('on'));
    row.classList.add('on');
    contextMenu(e, menu(row));
  });
}

// From a search box, the down arrow goes into its list.
function intoList(input, box) {
  input.addEventListener('keydown', (e) => {
    if (e.key !== 'ArrowDown' && e.key !== 'Enter') return;
    const first = box.querySelector('.row, tbody tr, .thumb');
    if (!first) return;
    e.preventDefault();
    box.focus();
    if (!box.querySelector('.on')) box.dispatchEvent(new KeyboardEvent('keydown', { key: 'Home' }));
    if (e.key === 'Enter') box.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter' }));
  });
}

async function explore(x, y, z) {
  try {
    const r = await api('/api/explore', { x, y, z: z == null ? 140 : z + 60 }, { method: 'POST' });
    toast(r.started ? 'Opening the 3D world. It streams in over a minute or so.' : `Could not open the 3D world: ${r.error}`);
  } catch (e) {
    toast(e.message);
  }
}

const KINDS = [
  { id: 0, name: 'Detailed models', color: '#36f5d0' },
  { id: 1, name: 'LOD stand-ins', color: '#ffc53d' },
  { id: 2, name: 'Super-LODs', color: '#ff5fae' },
  { id: 3, name: 'Water tiles', color: '#55b4ff' },
];

// The map's styles: what the server may have made.
const MAP_STYLES = [
  { id: 'satellite', name: 'Satellite', sea: '#215d66', tip: 'The game\'s textures, from above' },
  { id: 'clay', name: 'Clay', sea: '#a9bcc6', tip: 'One pale material: buildings and hills read by their shade' },
  { id: 'relief', name: 'Relief', sea: '#8ab4cb', tip: 'Coloured by height, contour lines every 10 m' },
];

// ---------------------------------------------------------------- shell

const S = { status: null, activity: null, tabs: [], active: null };

const ACTIVITIES = [
  { id: 'map', icon: 'map', title: 'Map', key: '1' },
  { id: 'models', icon: 'cube', title: 'Models', key: '2' },
  { id: 'files', icon: 'files', title: 'Files', key: '3' },
  { id: 'textures', icon: 'image', title: 'Textures', key: '4' },
  { id: 'video', icon: 'film', title: 'Video', key: '5' },
  { id: 'audio', icon: 'wave', title: 'Sound', key: '6' },
  { id: 'text', icon: 'text', title: 'Text', key: '7' },
  { id: 'cars', icon: 'car', title: 'Vehicles', key: '8' },
  { id: 'people', icon: 'person', title: 'People', key: '9' },
  { id: 'animals', icon: 'paw', title: 'Animals', key: null },
  { id: 'setup', icon: 'gear', title: 'Setup', key: null, bottom: true },
];

function setActivity(id) {
  S.activity = id;
  store.set('activity', id);
  for (const b of document.querySelectorAll('#act button')) b.classList.toggle('on', b.dataset.id === id);
  const a = ACTIVITIES.find((x) => x.id === id);
  $('#side-title').textContent = a.title;
  $('#side-extra').replaceChildren();
  const body = $('#side-body');
  body.replaceChildren();
  SIDES[id](body);
}

function inspect(...nodes) {
  $('#insp-body').replaceChildren(...nodes.flat().filter((n) => n != null && n !== false));
}

function setStatus(slot, text) {
  const el = document.querySelector(`#status [data-slot="${slot}"]`);
  if (el) el.textContent = text || '';
}

// Tabs: { id, title, icon, closable, view (element), show(), hide(), close() }
// `replace`: the id of a tab this one takes the place of (the next movie,
// another variant of a model).
function openTab(tab, replace) {
  const old = S.tabs.find((t) => t.id === tab.id);
  if (old) { showTab(old.id); return old; }
  const at = replace ? S.tabs.findIndex((t) => t.id === replace) : -1;
  if (at >= 0) S.tabs.splice(at + 1, 0, tab); else S.tabs.push(tab);
  tab.view.classList.add('view');
  $('#views').append(tab.view);
  if (at >= 0) { const [gone] = S.tabs.splice(at, 1); gone.hide?.(); gone.close?.(); gone.view.remove(); }
  renderTabs();
  showTab(tab.id);
  return tab;
}

function showTab(id) {
  for (const t of S.tabs) {
    const on = t.id === id;
    if (!on && t.view.classList.contains('on')) t.hide?.();
    t.view.classList.toggle('on', on);
  }
  S.active = id;
  renderTabs();
  const t = S.tabs.find((x) => x.id === id);
  t?.show?.();
}

function closeTab(id) {
  const i = S.tabs.findIndex((t) => t.id === id);
  if (i < 0 || !S.tabs[i].closable) return;
  const [t] = S.tabs.splice(i, 1);
  t.close?.();
  t.view.remove();
  if (S.active === id) showTab(S.tabs[Math.max(0, i - 1)].id);
  renderTabs();
}

// Shade the ends of the tab strip that have more tabs past them.
function tabEdges() {
  const b = $('#tabs'), bar = $('#tabbar');
  bar.classList.toggle('more-l', b.scrollLeft > 2);
  bar.classList.toggle('more-r', b.scrollLeft + b.clientWidth < b.scrollWidth - 2);
}

// Bring the active tab into the strip's view.
function revealTab() {
  const b = $('#tabs'), on = b.querySelector('.tab.on');
  if (!on) return;
  const r = on.getBoundingClientRect(), br = b.getBoundingClientRect();
  if (r.left < br.left) b.scrollLeft -= br.left - r.left + 12;
  else if (r.right > br.right) b.scrollLeft += r.right - br.right + 12;
}

function tabsMenu(at) {
  contextMenu(null, [
    ...S.tabs.map((t) => ({ label: t.title, icon: t.id === S.active ? 'check' : t.icon, run: () => showTab(t.id) })),
    '-',
    { label: 'Close all but this', icon: 'x', disabled: S.tabs.filter((x) => x.closable).length < 2, run: () => S.tabs.filter((x) => x.closable && x.id !== S.active).forEach((x) => closeTab(x.id)) },
  ], at);
}

function renderTabs() {
  const all = $('#tabs-all');
  all.replaceChildren(icon('chevron'), S.tabs.length > 1 ? h('span', { class: 'n', text: S.tabs.length }) : '');
  requestAnimationFrame(() => { revealTab(); tabEdges(); });
  $('#tabs').replaceChildren(
    ...S.tabs.map((t) =>
      h('div', { class: 'tab' + (t.id === S.active ? ' on' : ''), title: t.hint || t.title, onclick: () => showTab(t.id), onauxclick: (e) => e.button === 1 && closeTab(t.id),
        oncontextmenu: (e) => contextMenu(e, [
          t.closable ? { label: 'Close', icon: 'x', key: 'middle-click', run: () => closeTab(t.id) } : null,
          { label: 'Close others', icon: 'x', run: () => S.tabs.filter((x) => x.closable && x.id !== t.id).forEach((x) => closeTab(x.id)) },
          { label: 'Close to the right', icon: 'x', run: () => S.tabs.slice(S.tabs.indexOf(t) + 1).filter((x) => x.closable).forEach((x) => closeTab(x.id)) },
          '-',
          { label: 'Copy title', icon: 'copy', run: () => copy(t.title) },
        ]) },
        icon(t.icon), h('span', { class: 't', text: t.title }),
        t.closable ? h('span', { class: 'x', onclick: (e) => { e.stopPropagation(); closeTab(t.id); } }, icon('x')) : null)));
}

function initShell() {
  const act = $('#act');
  for (const a of ACTIVITIES) {
    if (a.bottom) act.append(h('div', { class: 'spacer' }));
    act.append(h('button', { 'data-id': a.id, 'data-tip': a.key ? `${a.title}  ${a.key}` : a.title, onclick: () => setActivity(a.id) }, icon(a.icon)));
  }
  $('#search-open').append(icon('search'), h('span', { text: 'Search models, text, or go to x, y…' }), h('kbd', { text: 'Ctrl K' }));
  $('#search-open').addEventListener('click', openPalette);
  const ti = $('#toggle-insp');
  ti.append(icon('panel'));
  const app = $('#app');
  const setInsp = (on) => { app.classList.toggle('no-inspector', !on); ti.classList.toggle('on', on); store.set('inspector', on); window.dispatchEvent(new Event('resize')); };
  setInsp(store.get('inspector', true));
  ti.addEventListener('click', () => setInsp(app.classList.contains('no-inspector')));
  $('#game-pill').addEventListener('click', () => setActivity('setup'));
  // In the app: the same page in the system browser, which draws as fast
  // as the screen refreshes (WebKitGTK may hold the window to 60 a second).
  if (window.ipc) {
    const b = h('button', { class: 'icon-btn', title: 'Open in your browser: as smooth as the screen allows (served from this window while it is open)', onclick: () => { window.ipc.postMessage('open-browser'); toast('Opening Vice Lens in your browser…'); } }, icon('globe'));
    $('.top-right').prepend(b);
  }
  // The tab strip scrolls with the wheel; the button lists every tab.
  const strip = $('#tabs');
  strip.addEventListener('wheel', (e) => {
    const d = Math.abs(e.deltaX) > Math.abs(e.deltaY) ? e.deltaX : e.deltaY;
    if (!d) return;
    e.preventDefault();
    strip.style.scrollBehavior = 'auto';
    strip.scrollLeft += d * (e.deltaMode === 1 ? 32 : 1);
    strip.style.scrollBehavior = '';
  }, { passive: false });
  strip.addEventListener('scroll', tabEdges);
  new ResizeObserver(tabEdges).observe(strip);
  $('#tabs-all').addEventListener('click', (e) => tabsMenu(e.currentTarget));
  // Resizable side panel and inspector.
  for (const r of document.querySelectorAll('.resizer')) {
    const which = r.dataset.for;
    const prop = which === 'side' ? '--side' : '--inspector';
    const saved = store.get(prop, null);
    if (saved) app.style.setProperty(prop, saved + 'px');
    r.addEventListener('pointerdown', (e) => {
      r.setPointerCapture(e.pointerId);
      r.classList.add('drag');
      const start = e.clientX;
      const w0 = (which === 'side' ? $('#side') : $('#insp')).getBoundingClientRect().width;
      const move = (ev) => {
        const w = Math.max(200, Math.min(640, which === 'side' ? w0 + ev.clientX - start : w0 - (ev.clientX - start)));
        app.style.setProperty(prop, w + 'px');
        store.set(prop, w);
        window.dispatchEvent(new Event('resize'));
      };
      const up = () => { r.classList.remove('drag'); r.removeEventListener('pointermove', move); r.removeEventListener('pointerup', up); };
      r.addEventListener('pointermove', move);
      r.addEventListener('pointerup', up);
    });
  }
  $('#status').append(
    h('span', { 'data-slot': 'cursor' }), h('span', { 'data-slot': 'sel' }), h('span', { class: 'grow' }),
    h('span', { 'data-slot': 'job', class: 'job' }), h('span', { 'data-slot': 'info' }), h('span', { 'data-slot': 'fps', title: 'Frames the page gets a second (Ctrl Shift F)' }));
  if (store.get('fps', false)) FPS.toggle(true);
  document.addEventListener('keydown', (e) => {
    const typing = /INPUT|TEXTAREA|SELECT/.test(document.activeElement?.tagName);
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') { e.preventDefault(); openPalette(); return; }
    if (e.ctrlKey && e.shiftKey && e.key.toLowerCase() === 'f') { e.preventDefault(); FPS.toggle(); return; }
    // Tabs: Ctrl Tab / Ctrl PageDown next, with Shift / PageUp back; Ctrl W closes.
    if (e.ctrlKey && (e.key === 'Tab' || e.key === 'PageDown' || e.key === 'PageUp') && S.tabs.length) {
      e.preventDefault();
      const back = e.key === 'PageUp' || (e.key === 'Tab' && e.shiftKey);
      const i = S.tabs.findIndex((t) => t.id === S.active), n = S.tabs.length;
      showTab(S.tabs[(i + (back ? n - 1 : 1)) % n].id);
      return;
    }
    if (e.ctrlKey && !e.shiftKey && e.key.toLowerCase() === 'w') { e.preventDefault(); closeTab(S.active); return; }
    if (typing) return;
    const a = ACTIVITIES.find((x) => x.key === e.key);
    if (a && !e.ctrlKey && !e.metaKey && !e.altKey) { setActivity(a.id); return; }
    if (e.key === 'i' && !e.ctrlKey) ti.click();
    if (e.key === '/') { e.preventDefault(); openPalette(); }
  });
  pollJobs();
}

async function pollJobs() {
  try {
    const j = await api('/api/jobs');
    const live = j.items.filter((x) => !x.finished);
    setStatus('job', live.map((x) => `${x.what} ${x.total ? Math.floor((100 * x.done) / x.total) : 0}%`).join('  ·  '));
    window.dispatchEvent(new CustomEvent('jobs', { detail: j.items }));
    for (const x of j.items.filter((x) => x.finished)) window.dispatchEvent(new CustomEvent('job-done', { detail: x }));
    setTimeout(pollJobs, live.length ? 1000 : 4000);
  } catch {
    setTimeout(pollJobs, 5000);
  }
}

// ---------------------------------------------------------------- frame rate

// How often the page gets a frame (requestAnimationFrame), shown in the
// status bar: Ctrl Shift F.
const FPS = {
  on: false, frames: 0, t0: 0, raf: 0,
  toggle(on = !this.on) {
    this.on = on;
    store.set('fps', on);
    cancelAnimationFrame(this.raf);
    setStatus('fps', '');
    if (!on) return;
    this.frames = 0;
    this.t0 = performance.now();
    const tick = (t) => {
      this.frames++;
      if (t - this.t0 >= 500) { setStatus('fps', `${Math.round((this.frames * 1000) / (t - this.t0))} fps`); this.frames = 0; this.t0 = t; }
      this.raf = requestAnimationFrame(tick);
    };
    this.raf = requestAnimationFrame(tick);
  },
};

// With #probe in the address (VICE_LENS_PROBE=1 for the app): measure the
// frames the page gets while still, while the map pans and while a model
// turns, and write them to the server's log.
async function probe() {
  const count = (ms, each) => new Promise((done) => {
    let n = 0, worst = 0, last = 0;
    const t0 = performance.now();
    const f = (t) => {
      if (last) worst = Math.max(worst, t - last);
      last = t; n++;
      each?.(t - t0);
      if (t - t0 < ms) requestAnimationFrame(f); else done({ fps: Math.round((n * 1000) / (t - t0)), worst: Math.round(worst) });
    };
    requestAnimationFrame(f);
  });
  const wait = (ms) => new Promise((r) => setTimeout(r, ms));
  await wait(2500);
  const still = await count(3000);
  setActivity('map'); showTab('map');
  const x0 = MAP.cx, y0 = MAP.cy;
  const pan = await count(3000, (t) => { MAP.cx = x0 + Math.sin(t / 400) * 300 * MAP.mpp; MAP.cy = y0 + Math.cos(t / 400) * 200 * MAP.mpp; MAP.draw(); });
  MAP.cx = x0; MAP.cy = y0; MAP.redraw();
  const r = await api('/api/models', { q: 'prop_am_bin_04a', limit: 1 }).catch(() => ({ items: [] }));
  let turn = { fps: 0, worst: 0 };
  if (r.items[0]) {
    openModel(r.items[0].h, r.items[0].name);
    const tab = S.tabs.find((t) => t.id === S.active);
    for (let i = 0; i < 40 && !tab.state?.viewer?.lo; i++) await wait(250);
    const v = tab.state?.viewer;
    if (v) turn = await count(3000, (t) => { v.yaw = t / 600; v.draw(); });
  }
  await api('/api/probe', { still: still.fps, still_worst: still.worst, pan: pan.fps, pan_worst: pan.worst, turn: turn.fps, turn_worst: turn.worst, dpr: devicePixelRatio, w: innerWidth, h: innerHeight }).catch(() => {});
  window.ipc?.postMessage('quit');
}

// ---------------------------------------------------------------- palette

function openPalette() {
  if ($('#palette-back')) return;
  const input = h('input', { placeholder: 'Model name, hash, text, or coordinates like -560, 44', spellcheck: 'false' });
  const results = h('div', { class: 'results' });
  const back = h('div', { id: 'palette-back', onclick: (e) => e.target === back && close() },
    h('div', { id: 'palette', class: 'glass' }, input, results,
      h('div', { class: 'foot' }, h('span', {}, h('kbd', { text: '↑↓' }), ' move'), h('span', {}, h('kbd', { text: 'Enter' }), ' open'), h('span', {}, h('kbd', { text: 'Esc' }), ' close'))));
  document.body.append(back);
  input.focus();
  let items = [];
  let sel = 0;
  const close = () => back.remove();
  const paint = () => {
    results.replaceChildren();
    let group = null;
    items.forEach((it, i) => {
      if (it.group !== group) { group = it.group; results.append(h('div', { class: 'group', text: group })); }
      results.append(h('div', { class: 'row' + (i === sel ? ' on' : ''), onmousemove: () => { if (sel !== i) { sel = i; paint(); } }, onclick: () => { close(); it.run(); } },
        icon(it.icon), h('span', { class: 'name' }, ...highlight(it.label, input.value.trim())), it.meta ? h('span', { class: 'meta', text: it.meta }) : null));
    });
    if (!items.length) results.append(h('div', { class: 'empty', text: input.value ? 'Nothing found.' : 'Type to search.' }));
    results.querySelector('.row.on')?.scrollIntoView({ block: 'nearest' });
  };
  const commands = [
    { group: 'Commands', icon: 'map', label: 'Show the map', run: () => { setActivity('map'); showTab('map'); } },
    { group: 'Commands', icon: 'files', label: 'Browse archives', run: () => setActivity('files') },
    { group: 'Commands', icon: 'gear', label: 'Setup: game folder and caches', run: () => openSetup() },
  ];
  let seq = 0;
  const search = debounce(async () => {
    const q = input.value.trim();
    const mine = ++seq;
    const out = [];
    const xy = q.match(/^\s*(-?\d+(?:\.\d+)?)\s*[, ]\s*(-?\d+(?:\.\d+)?)\s*$/);
    if (xy) out.push({ group: 'Go to', icon: 'pin', label: `${xy[1]}, ${xy[2]}`, meta: 'on the map', run: () => MAP.goTo(+xy[1], +xy[2], 2, true) });
    out.push(...commands.filter((c) => !q || c.label.toLowerCase().includes(q.toLowerCase())));
    if (q.length >= 2 && !xy) {
      const [m, t] = await Promise.all([api('/api/models', { q, limit: 8 }).catch(() => ({ items: [] })), api('/api/text', { q, limit: 5 }).catch(() => ({ items: [] }))]);
      if (mine !== seq) return;
      out.push(...m.items.map((x) => ({ group: 'Models', icon: 'cube', label: x.name, meta: x.placed ? `${fmtNum(x.placed)} placed` : '', run: () => openModel(x.h, x.name) })));
      out.push(...t.items.map(([label, text, src]) => ({ group: 'Text', icon: 'text', label: text.length > 80 ? text.slice(0, 80) + '…' : text, meta: src ? 'subtitles' : label, run: () => TEXT.find(q) })));
    }
    items = out;
    sel = 0;
    paint();
  }, 120);
  input.addEventListener('input', search);
  input.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') close();
    else if (e.key === 'ArrowDown') { sel = Math.min(items.length - 1, sel + 1); paint(); e.preventDefault(); }
    else if (e.key === 'ArrowUp') { sel = Math.max(0, sel - 1); paint(); e.preventDefault(); }
    else if (e.key === 'Enter' && items[sel]) { close(); items[sel].run(); }
  });
  search();
}

// ---------------------------------------------------------------- map

class MapView {
  constructor() {
    this.view = h('div', { class: 'map' });
    this.canvas = h('canvas');
    this.tip = h('div', { class: 'tip glass hidden' });
    this.scaleEl = h('div', { class: 'scale glass' });
    this.view.append(this.canvas, this.tip,
      h('div', { class: 'map-ui tr' },
        h('div', { class: 'glass stack' },
          h('button', { class: 'icon-btn', title: 'Zoom in (+)', onclick: () => this.zoomBy(1 / 1.6) }, icon('plus')),
          h('button', { class: 'icon-btn', title: 'Zoom out (−)', onclick: () => this.zoomBy(1.6) }, icon('minus')),
          h('button', { class: 'icon-btn', title: 'Whole island (0)', onclick: () => this.fit() }, icon('fit'))),
        h('div', { class: 'glass stack' },
          this.gridBtn = h('button', { class: 'icon-btn', title: 'Kilometre grid (G)', onclick: () => this.toggleGrid() }, icon('grid')))),
      h('div', { class: 'map-ui bl' }, this.scaleEl),
      this.styleSeg = h('div', { class: 'map-ui br glass seg map-styles' }));
    this.ctx = this.canvas.getContext('2d');
    this.style = store.get('map.style', 'satellite');
    this.meta = null;
    this.tiles = new Map();
    const v = store.get('map.view', null);
    this.cx = v?.cx ?? -560; this.cy = v?.cy ?? 44; this.mpp = v?.mpp ?? 8;
    this.layers = store.get('map.layers', { 0: true, 1: false, 2: false, 3: false });
    this.grid = store.get('map.grid', true);
    this.gridBtn.classList.toggle('on', this.grid);
    this.points = { items: [], total: 0, key: '' };
    this.marks = []; // [i, x, y] of a model's placements
    this.selected = null;
    this.hover = null;
    this.bind();
    new ResizeObserver(() => this.redraw()).observe(this.view);
  }

  async init() {
    try { this.meta = await api('/api/map/meta', { st: this.style }); } catch { this.meta = { ok: false }; }
    if (this.meta.ok && !this.meta.styles.includes(this.style)) { this.style = 'satellite'; this.meta = await api('/api/map/meta', { st: this.style }).catch(() => this.meta); }
    this.paintStyles();
    if (!this.meta.ok) {
      this.view.append(h('div', { class: 'center-msg' }, h('div', {},
        h('div', { style: 'font-size:15px;color:var(--text);margin-bottom:6px', text: 'No map yet' }),
        'Vice Lens draws the island once from the game\'s own LOD models.', h('br'),
        'Run ', h('code', { text: this.meta.command || './vice-lens.sh --make-map' }), ' and reload this page.')));
    }
    this.redraw();
  }

  paintStyles() {
    const have = this.meta?.styles || [];
    this.styleSeg.classList.toggle('hidden', have.length < 2);
    this.styleSeg.replaceChildren(...MAP_STYLES.filter((s) => have.includes(s.id)).map((s) =>
      h('button', { class: s.id === this.style ? 'on' : '', title: `${s.tip} (M switches)`, onclick: () => this.setStyle(s.id) }, s.name)));
  }

  async setStyle(id) {
    if (id === this.style) return;
    this.style = id;
    store.set('map.style', id);
    this.tiles.clear();
    this.meta = await api('/api/map/meta', { st: id }).catch(() => this.meta);
    this.paintStyles();
    this.redraw();
  }

  // The map's size on screen; while its tab is hidden it has none, so a
  // usual window size stands in (never 0: zoom divides by it).
  W() { return this.view.clientWidth || 1200; }
  H() { return this.view.clientHeight || 800; }
  toScreen(x, y) { return [(x - this.cx) / this.mpp + this.W() / 2, (this.cy - y) / this.mpp + this.H() / 2]; }
  toWorld(sx, sy) { return [this.cx + (sx - this.W() / 2) * this.mpp, this.cy - (sy - this.H() / 2) * this.mpp]; }

  save() { store.set('map.view', { cx: this.cx, cy: this.cy, mpp: this.mpp }); }

  goTo(x, y, mpp, mark) {
    if (S.activity !== 'map') setActivity('map');
    showTab('map');
    if (!Number.isFinite(x) || !Number.isFinite(y)) return;
    this.cx = x; this.cy = y;
    if (mpp && Number.isFinite(mpp)) this.mpp = Math.max(0.15, Math.min(80, mpp));
    if (mark) this.pin = [x, y]; else this.pin = null;
    this.save();
    this.redraw();
  }

  fit() {
    if (!this.meta?.ok) return;
    const m = this.meta;
    this.cx = m.west + (m.w * m.per_px) / 2;
    this.cy = m.north - (m.h * m.per_px) / 2;
    this.mpp = Math.max((m.w * m.per_px) / this.W(), (m.h * m.per_px) / this.H()) * 1.05;
    this.save();
    this.redraw();
  }

  zoomBy(f, sx = this.W() / 2, sy = this.H() / 2) {
    const [wx, wy] = this.toWorld(sx, sy);
    this.mpp = Math.max(0.15, Math.min(80, this.mpp * f));
    this.cx = wx - (sx - this.W() / 2) * this.mpp;
    this.cy = wy + (sy - this.H() / 2) * this.mpp;
    this.save();
    this.redraw();
  }

  toggleGrid() { this.grid = !this.grid; store.set('map.grid', this.grid); this.gridBtn.classList.toggle('on', this.grid); this.redraw(); }

  setLayer(k, on) { this.layers[k] = on; store.set('map.layers', this.layers); this.points.key = ''; this.redraw(); }

  bind() {
    let drag = null;
    const c = this.view;
    c.addEventListener('pointerdown', (e) => {
      if (e.target.closest('.map-ui')) return;
      drag = { x: e.clientX, y: e.clientY, cx: this.cx, cy: this.cy, moved: 0 };
      c.setPointerCapture(e.pointerId);
      c.classList.add('drag');
    });
    c.addEventListener('pointermove', (e) => {
      const r = c.getBoundingClientRect();
      const sx = e.clientX - r.left, sy = e.clientY - r.top;
      const [wx, wy] = this.toWorld(sx, sy);
      setStatus('cursor', `x ${fx(wx)}  y ${fx(wy)}`);
      if (drag) {
        const dx = e.clientX - drag.x, dy = e.clientY - drag.y;
        drag.moved = Math.max(drag.moved, Math.hypot(dx, dy));
        this.cx = drag.cx - dx * this.mpp;
        this.cy = drag.cy + dy * this.mpp;
        this.redraw();
        return;
      }
      const hit = this.pick(sx, sy);
      if (hit !== this.hover) { this.hover = hit; this.draw(); if (hit) PREVIEW.show(hit); }
      if (hit) {
        this.tip.classList.remove('hidden');
        this.tip.style.left = sx + 'px';
        this.tip.style.top = sy + 'px';
        this.tip.textContent = hit[4];
      } else this.tip.classList.add('hidden');
    });
    c.addEventListener('pointerup', (e) => {
      if (!drag) return;
      c.classList.remove('drag');
      if (drag.moved < 4 && !e.target.closest('.map-ui')) {
        const r = c.getBoundingClientRect();
        const hit = this.pick(e.clientX - r.left, e.clientY - r.top);
        if (hit) this.select(hit[0]); else { this.selected = null; this.draw(); }
      } else this.save();
      drag = null;
    });
    c.addEventListener('pointerleave', () => { this.tip.classList.add('hidden'); setStatus('cursor', ''); });
    c.addEventListener('contextmenu', (e) => {
      if (!this.meta?.ok) return;
      const r = c.getBoundingClientRect();
      const sx = e.clientX - r.left, sy = e.clientY - r.top;
      const [wx, wy] = this.toWorld(sx, sy);
      const hit = this.pick(sx, sy);
      const place = () => api('/api/placement', { i: hit[0] });
      contextMenu(e, [
        hit ? { label: `Open ${hit[4]}`, icon: 'cube', run: async () => { const p = await place(); openModel(p.model, p.name); } } : null,
        hit ? { label: 'Details', icon: 'pin', run: () => this.select(hit[0]) } : null,
        hit ? { label: 'Every placement of this model', icon: 'map', run: async () => { const p = await place(); const pl = await api('/api/model/places', { h: p.model }); this.showModel(p.model, p.name, pl.items); } } : null,
        hit ? { label: 'Explore in 3D from this object', icon: 'globe', run: async () => { const p = await place(); explore(p.pos[0], p.pos[1], p.pos[2]); } } : null,
        hit ? { label: 'Copy model name', icon: 'copy', run: () => copy(hit[4]) } : null,
        hit ? '-' : null,
        { label: 'Explore in 3D here', icon: 'globe', run: () => explore(wx, wy) },
        { label: 'Centre here', icon: 'fit', run: () => { this.cx = wx; this.cy = wy; this.save(); this.redraw(); } },
        { label: 'Zoom in here', icon: 'plus', run: () => this.zoomBy(1 / 2.5, sx, sy) },
        { label: `Copy ${fx(wx)}, ${fx(wy)}`, icon: 'copy', run: () => copy(`${fx(wx, 2)}, ${fx(wy, 2)}`) },
        '-',
        { label: 'Whole island', icon: 'fit', key: '0', run: () => this.fit() },
        { label: this.grid ? 'Hide the grid' : 'Show the grid', icon: 'grid', key: 'G', run: () => this.toggleGrid() },
      ]);
    });
    c.addEventListener('wheel', (e) => {
      e.preventDefault();
      const r = c.getBoundingClientRect();
      this.zoomBy(Math.pow(1.0018, e.deltaY), e.clientX - r.left, e.clientY - r.top);
    }, { passive: false });
    c.addEventListener('dblclick', (e) => {
      const r = c.getBoundingClientRect();
      this.zoomBy(1 / 2, e.clientX - r.left, e.clientY - r.top);
    });
    document.addEventListener('keydown', (e) => {
      if (S.active !== 'map' || /INPUT|TEXTAREA/.test(document.activeElement?.tagName) || e.ctrlKey || e.metaKey) return;
      if (e.key === '+' || e.key === '=') this.zoomBy(1 / 1.6);
      else if (e.key === '-') this.zoomBy(1.6);
      else if (e.key === '0') this.fit();
      else if (e.key === 'g') this.toggleGrid();
      else if (e.key === 'm') { const have = MAP_STYLES.filter((s) => this.meta?.styles?.includes(s.id)); const i = have.findIndex((s) => s.id === this.style); if (have.length > 1) this.setStyle(have[(i + 1) % have.length].id); }
      else if (e.key === 'Escape') { this.selected = null; this.marks = []; this.pin = null; PREVIEW.hide(); this.draw(); }
    });
  }

  pick(sx, sy) {
    let best = null, bd = 100;
    for (const p of this.points.items) {
      if (!this.layers[p[3]]) continue;
      const [x, y] = this.toScreen(p[1], p[2]);
      const d = (x - sx) ** 2 + (y - sy) ** 2;
      if (d < bd) { bd = d; best = p; }
    }
    return best;
  }

  async select(i) {
    this.selected = i;
    this.draw();
    try {
      const p = await api('/api/placement', { i });
      if (this.selected !== i) return;
      showPlacement(p);
    } catch (e) { toast(e.message); }
  }

  showModel(hash, name, places) {
    if (!places.length) { toast(`${name} is not placed on the map`); return; }
    // Show the map first: its size decides the zoom.
    if (S.activity !== 'map') setActivity('map');
    showTab('map');
    this.marks = places;
    let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
    for (const p of places) { x0 = Math.min(x0, p[1]); x1 = Math.max(x1, p[1]); y0 = Math.min(y0, p[2]); y1 = Math.max(y1, p[2]); }
    const span = Math.max(x1 - x0, y1 - y0, 200);
    this.goTo((x0 + x1) / 2, (y0 + y1) / 2, Math.max(0.6, (span * 1.3) / Math.min(this.W(), this.H())));
    setStatus('sel', `${name}: ${fmtNum(places.length)} placements`);
  }

  redraw() {
    cancelAnimationFrame(this.raf);
    this.raf = requestAnimationFrame(() => { this.draw(); this.fetchPoints(); });
  }

  // One draw for every tile that arrives in a frame.
  redrawSoon() {
    if (this.soon) return;
    this.soon = requestAnimationFrame(() => { this.soon = 0; this.draw(); });
  }

  level() {
    const m = this.meta;
    const s = Math.pow(2, Math.floor(Math.log2(Math.max(1, this.mpp / m.per_px))));
    return Math.max(1, Math.min(64, s));
  }

  tile(s, x, y) {
    const k = `${s}/${x}/${y}`;
    let t = this.tiles.get(k);
    if (!t) {
      // Decoded before the first draw that uses it: drawing an image that
      // is not decoded yet decodes it then, a hitch while panning.
      t = { img: new Image(), ok: false };
      t.img.src = `/api/map/tile?st=${this.style}&v=${this.meta.v || 0}&s=${s}&x=${x}&y=${y}`;
      t.ready = t.img.decode().then(() => { t.ok = true; this.redrawSoon(); }, () => { t.bad = true; });
      this.tiles.set(k, t);
      if (this.tiles.size > 600) for (const key of this.tiles.keys()) { this.tiles.delete(key); if (this.tiles.size < 450) break; }
    }
    return t;
  }

  drawLevel(ctx, s, request) {
    const m = this.meta;
    const span = 256 * s * m.per_px; // metres a tile covers
    const [x0, y1] = this.toWorld(0, 0), [x1, y0] = this.toWorld(this.W(), this.H());
    const tx0 = Math.max(0, Math.floor((x0 - m.west) / span)), tx1 = Math.floor((x1 - m.west) / span);
    const ty0 = Math.max(0, Math.floor((m.north - y1) / span)), ty1 = Math.floor((m.north - y0) / span);
    const maxX = Math.ceil(m.w / (256 * s)) - 1, maxY = Math.ceil(m.h / (256 * s)) - 1;
    let missing = false;
    for (let ty = ty0; ty <= Math.min(ty1, maxY); ty++) {
      for (let tx = tx0; tx <= Math.min(tx1, maxX); tx++) {
        const key = `${s}/${tx}/${ty}`;
        const t = request ? this.tile(s, tx, ty) : this.tiles.get(key);
        if (!t || !t.ok) { missing = true; continue; }
        const [sx, sy] = this.toScreen(m.west + tx * span, m.north - ty * span);
        const size = span / this.mpp;
        ctx.drawImage(t.img, Math.floor(sx), Math.floor(sy), Math.ceil(size) + 1, Math.ceil(size) + 1);
      }
    }
    return missing;
  }

  draw() {
    const dpr = window.devicePixelRatio || 1;
    const W = this.W(), H = this.H();
    if (!W || !H) return;
    if (this.canvas.width !== Math.round(W * dpr) || this.canvas.height !== Math.round(H * dpr)) {
      this.canvas.width = Math.round(W * dpr);
      this.canvas.height = Math.round(H * dpr);
    }
    const ctx = this.ctx;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.fillStyle = (MAP_STYLES.find((s) => s.id === this.style) || MAP_STYLES[0]).sea;
    ctx.fillRect(0, 0, W, H);
    if (!this.meta?.ok) return;
    ctx.imageSmoothingEnabled = this.mpp > this.meta.per_px * 0.75;
    const s = this.level();
    // Coarser tiles first, as a placeholder while the sharp ones come.
    if (this.drawLevel(ctx, s, false)) for (const c of [s * 4, s * 2]) if (c <= 64) this.drawLevel(ctx, c, true);
    this.drawLevel(ctx, s, true);
    if (this.grid) this.drawGrid(ctx, W, H);
    this.drawPoints(ctx);
    this.drawScale();
  }

  drawGrid(ctx, W, H) {
    const step = this.mpp < 2 ? 100 : this.mpp < 12 ? 1000 : 2000;
    const [x0, y1] = this.toWorld(0, 0), [x1, y0] = this.toWorld(W, H);
    if (![x0, x1, y0, y1].every(Number.isFinite) || (x1 - x0) / step > 400 || (y1 - y0) / step > 400) return;
    ctx.lineWidth = 1;
    ctx.font = '10.5px ' + getComputedStyle(document.body).getPropertyValue('--mono');
    ctx.fillStyle = 'rgba(214,219,226,0.55)';
    for (let x = Math.ceil(x0 / step) * step; x <= x1; x += step) {
      const [sx] = this.toScreen(x, 0);
      ctx.strokeStyle = x === 0 ? 'rgba(255,111,177,0.45)' : 'rgba(214,219,226,0.12)';
      ctx.beginPath(); ctx.moveTo(Math.round(sx) + 0.5, 0); ctx.lineTo(Math.round(sx) + 0.5, H); ctx.stroke();
      ctx.fillText(String(x), sx + 4, 12);
    }
    for (let y = Math.ceil(y0 / step) * step; y <= y1; y += step) {
      const [, sy] = this.toScreen(0, y);
      ctx.strokeStyle = y === 0 ? 'rgba(255,111,177,0.45)' : 'rgba(214,219,226,0.12)';
      ctx.beginPath(); ctx.moveTo(0, Math.round(sy) + 0.5); ctx.lineTo(W, Math.round(sy) + 0.5); ctx.stroke();
      ctx.fillText(String(y), 4, sy - 4);
    }
  }

  // Placements as bright dots with a dark rim, one path per kind; larger
  // as the map zooms in.
  drawPoints(ctx) {
    const r = this.mpp < 0.35 ? 6 : this.mpp < 0.8 ? 4.5 : this.mpp < 2 ? 3.2 : 2.5;
    const paths = KINDS.map(() => new Path2D());
    const W = this.W(), H = this.H();
    for (const p of this.points.items) {
      if (!this.layers[p[3]]) continue;
      const [x, y] = this.toScreen(p[1], p[2]);
      if (x < -r || y < -r || x > W + r || y > H + r) continue;
      paths[p[3]].moveTo(x + r, y);
      paths[p[3]].arc(x, y, r, 0, Math.PI * 2);
    }
    ctx.lineWidth = r > 3 ? 1.5 : 1;
    ctx.strokeStyle = 'rgba(4, 8, 12, 0.8)';
    paths.forEach((path, k) => { ctx.fillStyle = KINDS[k].color; ctx.fill(path); ctx.stroke(path); });
    for (const p of this.marks) {
      const [x, y] = this.toScreen(p[1], p[2]);
      ctx.strokeStyle = '#ff6fb1';
      ctx.lineWidth = 2;
      ctx.beginPath(); ctx.arc(x, y, 7, 0, Math.PI * 2); ctx.stroke();
    }
    const ring = (x, y, color, rad) => { const [sx, sy] = this.toScreen(x, y); ctx.strokeStyle = color; ctx.lineWidth = 2; ctx.beginPath(); ctx.arc(sx, sy, rad, 0, Math.PI * 2); ctx.stroke(); };
    if (this.hover) { ring(this.hover[1], this.hover[2], 'rgba(0,0,0,0.6)', r + 5); ring(this.hover[1], this.hover[2], '#ffffff', r + 4); }
    const sel = this.points.items.find((p) => p[0] === this.selected) || (this.selectedAt && this.selected != null ? [this.selected, ...this.selectedAt] : null);
    if (sel) { ring(sel[1], sel[2], '#4fe0c4', 9); ring(sel[1], sel[2], 'rgba(79,224,196,0.35)', 14); }
    if (this.pin) { ring(this.pin[0], this.pin[1], '#ff6fb1', 10); ring(this.pin[0], this.pin[1], '#ff6fb1', 3); }
  }

  drawScale() {
    const target = 110 * this.mpp;
    const p = Math.pow(10, Math.floor(Math.log10(target)));
    const nice = [1, 2, 5, 10].map((k) => k * p).filter((v) => v <= target).pop() || p;
    this.scaleEl.replaceChildren(h('span', { text: fmtM(nice) }), h('div', { class: 'bar', style: `width:${nice / this.mpp}px` }));
  }

  fetchPoints = debounce(async () => {
    if (!this.meta?.ok) return;
    const kinds = Object.entries(this.layers).filter(([, on]) => on).map(([k]) => k).join(',');
    if (this.mpp > 6 || !kinds) {
      if (this.points.items.length) { this.points = { items: [], total: 0, key: '' }; this.draw(); }
      setStatus('info', this.mpp > 6 ? 'zoom in to see placements' : '');
      SIDE_MAP?.counts?.(null);
      return;
    }
    const [x0, y1] = this.toWorld(-40, -40), [x1, y0] = this.toWorld(this.W() + 40, this.H() + 40);
    const key = `${Math.round(x0)},${Math.round(y0)},${Math.round(x1)},${Math.round(y1)},${kinds}`;
    if (key === this.points.key) return;
    try {
      const r = await api('/api/map/placements', { x0, y0, x1, y1, kinds, limit: 6000 });
      this.points = { items: r.items, total: r.total, key };
      setStatus('info', `${fmtNum(r.total)} placements in view${r.total > r.items.length ? `, ${fmtNum(r.items.length)} drawn` : ''}`);
      const c = {};
      for (const p of r.items) c[p[3]] = (c[p[3]] || 0) + 1;
      SIDE_MAP?.counts?.(c);
      this.draw();
    } catch (e) {
      setStatus('info', e.message);
    }
  }, 140);
}

let MAP;
let SIDE_MAP;

function kv(pairs) {
  const el = h('div', { class: 'kv' });
  for (const [k, v] of pairs) if (v != null && v !== '') el.append(h('div', { class: 'k', text: k }), h('div', { class: 'v' }, v));
  return el;
}

function showPlacement(p) {
  const kind = KINDS[p.kind];
  setStatus('sel', `${p.name} at ${fx(p.pos[0])}, ${fx(p.pos[1])}`);
  inspect(
    h('div', { class: 'insp-title' }, h('div', { class: 'n', text: p.name }),
      h('div', { class: 's' }, h('span', { class: 'swatch', style: `background:${kind.color}` }), kind.name, h('span', { class: 'badge', text: `#${p.i}` }))),
    h('div', { class: 'insp-actions' },
      p.file ? h('button', { class: 'btn primary', onclick: () => openModel(p.model, p.name) }, icon('cube'), 'Open model') : h('span', { class: 'badge warn', text: 'model not in the index' }),
      h('button', { class: 'btn', title: 'Open the 3D world here', onclick: () => explore(p.pos[0], p.pos[1], p.pos[2]) }, icon('globe'), 'Explore in 3D'),
      h('button', { class: 'btn', onclick: () => copy(`${fx(p.pos[0], 2)}, ${fx(p.pos[1], 2)}, ${fx(p.pos[2], 2)}`) }, icon('copy'), 'Copy position'),
      p.file ? h('button', { class: 'btn', onclick: () => openBytes(p.file.archive, p.file.offset, p.name) }, icon('hex'), 'Bytes') : null),
    h('div', { class: 'insp-h', text: 'Placement' }),
    kv([
      ['Position', `${fx(p.pos[0], 2)}, ${fx(p.pos[1], 2)}, ${fx(p.pos[2], 2)}`],
      ['Rotation', p.rot.map((v) => fx(v, 3)).join(', ')],
      ['Scale', `${fx(p.scale[0], 2)} × ${fx(p.scale[1], 2)}`],
      ['Model hash', p.model],
      ['Flags', p.flags],
      ['Parent', p.parent === -1 ? 'none' : `${p.parent} (0x${(p.parent >>> 0).toString(16)})`],
    ]),
    h('div', { class: 'insp-h', text: 'Level of detail' }),
    kv([
      ['LOD distance', p.lod > 0 ? `${fx(p.lod, 0)} m` : 'from archetype'],
      ['Archetype', p.archetype ? `${fx(p.archetype.lod, 0)} m, radius ${fx(p.archetype.radius, 1)} m` : 'unknown'],
      ['Children', p.children ? `${p.children}, take over within ${fx(p.child_lod, 0)} m` : 'none'],
    ]),
    p.file ? h('div', { class: 'insp-h', text: 'Stored in' }) : null,
    p.file ? kv([['Archive', p.file.archive], ['Offset', p.file.offset], ['Entry', p.file.entry ?? 'whole resource']]) : null);
}

// ---------------------------------------------------------------- side panels

function modelMenu(x) {
  return [
    { label: 'Open in 3D viewer', icon: 'cube', key: 'Enter', run: () => openModel(x.h, x.name) },
    { label: 'Show on the map', icon: 'map', disabled: !x.placed, run: async () => { const pl = await api('/api/model/places', { h: x.h }); MAP.showModel(x.h, x.name, pl.items); } },
    '-',
    { label: 'Copy name', icon: 'copy', run: () => copy(x.name) },
    { label: 'Copy hash', icon: 'copy', run: () => copy(x.h) },
  ];
}

const rowsOf = (box) => () => [...box.querySelectorAll(':scope > .row, :scope > tr, :scope tbody > tr, :scope > .thumb')];

const SIDES = {
  map(body) {
    const counts = {};
    const layer = (k) => {
      const box = h('input', { type: 'checkbox' });
      box.checked = !!MAP.layers[k.id];
      box.addEventListener('change', () => MAP.setLayer(k.id, box.checked));
      counts[k.id] = h('span', { class: 'count' });
      return h('label', { class: 'toggle' }, box, h('span', { class: 'box' }), h('span', { class: 'swatch', style: `background:${k.color}` }), k.name, counts[k.id]);
    };
    SIDE_MAP = { counts: (c) => { for (const k of KINDS) if (counts[k.id]) counts[k.id].textContent = c && c[k.id] ? fmtNum(c[k.id]) : ''; } };
    const results = h('ul', { class: 'list' });
    let seq = 0;
    const find = searchBox('mapfind', 'Find a model on the map', async (p) => {
      const mine = ++seq;
      if (p.q.length < 2) { results.replaceChildren(); return; }
      const r = await api('/api/models', { ...p, placed: 1, limit: 120 }).catch((e) => { find.error(e.message); return { items: [] }; });
      if (mine !== seq) return;
      results.replaceChildren(...r.items.map((x) => {
        const li = h('li', { class: 'row', onclick: () => { for (const el of results.children) el.classList.remove('on'); li.classList.add('on'); show(x); } },
          h('span', { class: 'name' }, ...highlight(x.name, p)), h('span', { class: 'meta', text: fmtNum(x.placed) }));
        li._x = x;
        return li;
      }));
      if (!results.children.length) results.append(h('li', { class: 'empty', text: 'No placed model matches.' }));
    }, { filters: [
      { id: 'kind', title: 'What kind of model', options: [['', 'Any kind'], ['prop', 'Props'], ['interior', 'Interiors'], ['nature', 'Plants & rocks'], ['terrain', 'Terrain tiles'], ['other', 'Buildings & other']] },
      { id: 'sort', title: 'Order', options: [['', 'Best match'], ['placed', 'Most placed'], ['name', 'By name']] },
    ] });
    const show = async (x) => { const pl = await api('/api/model/places', { h: x.h }); MAP.showModel(x.h, x.name, pl.items); };
    navigable(results, { rows: rowsOf(results), open: (r) => show(r._x), menu: (r) => modelMenu(r._x) });
    intoList(find.input, results);
    const goto = h('input', { class: 'text plain', placeholder: 'x, y   e.g. -560, 44', spellcheck: 'false' });
    goto.addEventListener('keydown', (e) => {
      if (e.key !== 'Enter') return;
      const m = goto.value.match(/(-?\d+(?:\.\d+)?)\s*[, ]\s*(-?\d+(?:\.\d+)?)/);
      if (m) MAP.goTo(+m[1], +m[2], 2, true); else toast('Type two numbers: x, y');
    });
    body.append(
      h('div', { class: 'section' }, h('h4', { text: 'Layers' }), ...KINDS.map(layer), h('div', { class: 'faint', style: 'font-size:11.5px;margin-top:4px', text: 'Placements show from about 6 m per pixel in.' })),
      h('div', { class: 'section' }, h('h4', { text: 'Go to' }), goto),
      h('div', { class: 'section sticky' }, h('h4', { text: 'Find on the map' }), find.el),
      results);
    if (find.input.value.trim().length >= 2) find.fire();
    MAP.fetchPoints();
  },

  models(body) {
    const list = h('ul', { class: 'list' });
    const info = h('div', { class: 'faint', style: 'padding:6px 12px 2px;font-size:12px' });
    let seq = 0;
    const run = async (p) => {
      const mine = ++seq;
      const r = await api('/api/models', { ...p, limit: 400 }).catch((e) => ({ items: [], error: e.message }));
      if (mine !== seq) return;
      box.error(r.error);
      info.textContent = r.error ? '' : `${fmtNum(r.total)} models${r.total > r.items.length ? `, first ${r.items.length}` : ''}`;
      list.replaceChildren(...r.items.map((x) => {
        const li = h('li', { class: 'row', title: x.h, onclick: () => { rowsOf(list)().forEach((e) => e.classList.remove('on')); li.classList.add('on'); }, ondblclick: () => openModel(x.h, x.name) },
          icon('cube'), h('span', { class: 'name' }, ...highlight(x.name, p)), x.placed ? h('span', { class: 'meta', text: fmtNum(x.placed) }) : null);
        li._x = x;
        return li;
      }));
    };
    navigable(list, { rows: rowsOf(list), open: (r) => openModel(r._x.h, r._x.name), menu: (r) => modelMenu(r._x) });
    const box = searchBox('models', 'Model name or hash', run, { filters: [
      { id: 'kind', title: 'What kind of model (by its name)', options: [['', 'Any kind'], ['prop', 'Props'], ['interior', 'Interiors'], ['character', 'Characters & clothes'], ['weapon', 'Weapons'], ['nature', 'Plants & rocks'], ['terrain', 'Terrain tiles'], ['effect', 'Effects'], ['other', 'Buildings & other']] },
      { id: 'placed', title: 'Whether the map places it', options: [['', 'Placed or not'], ['1', 'On the map'], ['0', 'Not on the map']] },
      { id: 'lods', title: 'LOD stand-ins and super-LODs', options: [['', 'With LOD models'], ['0', 'Without LOD models']] },
      { id: 'sort', title: 'Order', options: [['', 'Best match'], ['name', 'By name'], ['placed', 'Most placed']] },
    ] });
    intoList(box.input, list);
    body.append(h('div', { class: 'section sticky' }, box.el, info), list);
    run(box.params());
    setTimeout(() => box.input.focus(), 0);
  },

  async files(body) {
    const list = h('ul', { class: 'list' });
    body.append(h('div', { class: 'section faint', text: 'Archives in the game folder. Listing an archive reads it once and keeps the list in the cache.' }), list);
    try {
      const r = await api('/api/archives');
      list.replaceChildren(...r.items.map((a) => {
        const li = h('li', { class: 'row', onclick: () => openArchive(a.name) },
          icon('files'), h('span', { class: 'name', text: a.name }),
          h('span', { class: 'meta', text: fmtBytes(a.bytes) }), a.indexed ? h('span', { class: 'badge ok', text: 'listed' }) : h('span', { class: 'badge', text: 'not listed' }));
        li._a = a;
        return li;
      }));
      navigable(list, { rows: rowsOf(list), open: (r) => openArchive(r._a.name), menu: (r) => [{ label: 'Open', icon: 'files', run: () => openArchive(r._a.name) }, { label: 'Copy name', icon: 'copy', run: () => copy(r._a.name) }] });
      if (!r.items.length) list.append(h('li', { class: 'empty' }, 'No archives found. Check the game folder in ', h('b', { text: 'Setup' }), '.'));
    } catch (e) {
      list.append(h('li', { class: 'empty', text: e.message }));
    }
  },

  textures(body) {
    const list = h('ul', { class: 'list' });
    const info = h('div', { class: 'faint', style: 'padding:6px 12px 2px;font-size:12px', text: 'Reading the texture dictionaries…' });
    let dicts = null, seq = 0;
    const run = async (p) => {
      const mine = ++seq;
      if (!p.q) {
        if (!dicts) return;
        info.textContent = `${fmtNum(dicts.length)} dictionaries in textures.rpf and data.rpf`;
        list.replaceChildren(...dicts.map((d) => {
          const li = h('li', { class: 'row', onclick: () => { rowsOf(list)().forEach((e) => e.classList.remove('on')); li.classList.add('on'); }, ondblclick: () => openTexDict(d) },
            icon('image'), h('span', { class: 'name', text: d.first || d.o }), h('span', { class: 'meta', text: String(d.count) }));
          li._d = d;
          return li;
        }));
        return;
      }
      const r = await api('/api/textures', p).catch((e) => ({ hits: [], error: e.message }));
      if (mine !== seq) return;
      box.error(r.error);
      info.textContent = r.error ? '' : `${fmtNum(r.hits.length)}${r.more ? '+' : ''} textures`;
      list.replaceChildren(...r.hits.map((t) => {
        const li = h('li', { class: 'row', onclick: () => { rowsOf(list)().forEach((e) => e.classList.remove('on')); li.classList.add('on'); }, ondblclick: () => openTexDict({ a: t.a, o: t.o, first: t.name }, t.i) },
          icon('image'), h('span', { class: 'name' }, ...highlight(t.name, p)), h('span', { class: 'meta', text: t.a.replace('.rpf', '') }));
        li._t = t;
        return li;
      }));
    };
    const open = (r) => (r._d ? openTexDict(r._d) : openTexDict({ a: r._t.a, o: r._t.o, first: r._t.name }, r._t.i));
    navigable(list, { rows: rowsOf(list), open, menu: (r) => [{ label: 'Open', icon: 'image', key: 'Enter', run: () => open(r) }, { label: 'Copy name', icon: 'copy', run: () => copy(r._d?.first || r._t.name) }] });
    const box = searchBox('textures', 'Texture name', run);
    intoList(box.input, list);
    body.append(h('div', { class: 'section sticky' }, box.el, info), list);
    api('/api/textures').then((r) => { dicts = r.dicts; run(box.params()); }).catch((e) => (info.textContent = e.message));
  },

  async video(body) {
    const list = h('ul', { class: 'list' });
    const info = h('div', { class: 'faint', style: 'padding:6px 12px 2px;font-size:12px', text: 'Looking for movies…' });
    const note = h('div', { class: 'section faint hidden' });
    const lazy = new IntersectionObserver((es) => { for (const e of es) if (e.isIntersecting) { e.target.src = e.target.dataset.src; lazy.unobserve(e.target); } }, { root: body, rootMargin: '300px' });
    const shape = (m) => (m.h > m.w * 1.05 ? 'tall' : m.w > m.h * 1.05 ? 'wide' : 'square');
    const run = (p) => {
      if (!VIDEO.all) return;
      const g = matcher(p), re = g && new RegExp(g.source, g.flags.replace('g', ''));
      const len = { short: [0, 15], mid: [15, 60], long: [60, 1e9] }[p.len];
      const out = VIDEO.all.filter((m) => (!p.shape || shape(m) === p.shape) && (!p.sound || (p.sound === '1') === m.audio > 0) && (!len || (m.seconds >= len[0] && m.seconds < len[1]))
        && (!re || re.test(`#${m.k} ${m.w}×${m.h} ${m.w}x${m.h} ${fmtT(m.seconds)}`)));
      if (p.sort === 'long') out.sort((a, b) => b.seconds - a.seconds);
      else if (p.sort === 'big') out.sort((a, b) => b.bytes - a.bytes);
      VIDEO.list = out;
      const mins = Math.round(out.reduce((a, m) => a + m.seconds, 0) / 60);
      info.textContent = `${out.length} of ${VIDEO.all.length} movies, ${mins >= 60 ? `${Math.floor(mins / 60)} h ` : ''}${mins % 60} min`;
      lazy.disconnect();
      list.replaceChildren(...out.map((m) => {
        const img = h('img', { class: 'poster-thumb' + (shape(m) === 'tall' ? ' tall' : ''), alt: '', 'data-src': `/api/movie/poster?o=${m.o}` });
        img.addEventListener('error', () => (img.style.visibility = 'hidden'));
        if (VIDEO.bink2) lazy.observe(img);
        const li = h('li', { class: 'row', title: `${m.w}×${m.h}, ${fx(m.seconds, 1)} s, ${fmtBytes(m.bytes)}`, onclick: () => { rowsOf(list)().forEach((e) => e.classList.remove('on')); li.classList.add('on'); }, ondblclick: () => openMovie(m) },
          img, h('span', { class: 'name' }, `#${m.k}`, m.ready ? h('span', { class: 'ready', title: 'Converted already: plays at once' }) : null, h('span', { class: 'sub', text: `${m.w}×${m.h} · ${fmtT(m.seconds)}` })),
          !m.audio ? h('span', { class: 'badge', title: 'The file holds no sound track', text: 'silent' }) : m.audio > 1 ? h('span', { class: 'badge', title: 'Sound tracks in the file', text: `${m.audio} tracks` }) : null);
        li._m = m;
        return li;
      }));
    };
    const box = searchBox('video', '#12, 720×1280 or 7:54', run, { filters: [
      { id: 'shape', title: 'Picture shape', options: [['', 'Any shape'], ['wide', 'Landscape'], ['tall', 'Portrait (phone)'], ['square', 'Square']] },
      { id: 'sound', title: 'Sound', options: [['', 'With or without sound'], ['1', 'With sound'], ['0', 'Silent']] },
      { id: 'len', title: 'Length', options: [['', 'Any length'], ['short', 'Under 15 s'], ['mid', '15 s to 1 min'], ['long', 'Over 1 min']] },
      { id: 'sort', title: 'Order', options: [['', 'Archive order'], ['long', 'Longest first'], ['big', 'Largest first']] },
    ] });
    navigable(list, { rows: rowsOf(list), open: (r) => openMovie(r._m), menu: (r) => [{ label: 'Play', icon: 'play', key: 'Enter', run: () => openMovie(r._m) }, { label: 'Save as stored (.bk2)…', icon: 'save', run: () => saveAs(`/api/movie/raw?o=${r._m.o}`, `movie-${r._m.k}.bk2`) }, '-', { label: 'Copy offset', icon: 'copy', run: () => copy(r._m.o) }] });
    intoList(box.input, list);
    body.append(h('div', { class: 'section sticky' }, box.el, info), note, list);
    try {
      await VIDEO.load();
      if (!VIDEO.bink2) { note.classList.remove('hidden'); note.replaceChildren(h('span', { style: 'color:var(--amber)', text: 'To play the movies and show their pictures, Vice Lens needs an FFmpeg that decodes Bink 2: choose one in Setup.' })); }
      run(box.params());
    } catch (e) {
      info.textContent = e.message;
    }
  },

  async audio(body) {
    const list = h('ul', { class: 'list' });
    const info = h('div', { class: 'section faint', text: 'Finding the sound banks in audio.rpf… (the first time reads the archive once)' });
    const seg = h('div', { class: 'seg' });
    let music = store.get('audio.music', '');
    let from = 0, total = 0, busy = false;
    const more = async (reset) => {
      if (busy) return;
      busy = true;
      if (reset) { from = 0; list.replaceChildren(); }
      try {
        const r = await api('/api/banks', { from, limit: 400, music: music || null });
        total = r.total;
        info.textContent = `${fmtNum(r.total)} of ${fmtNum(r.all)} banks. Names are hashed in the data; banks are numbered in archive order.`;
        for (const [k, o, bytes, streams, isMusic] of r.items) {
          const li = h('li', { class: 'row', onclick: () => { rowsOf(list)().forEach((e) => e.classList.remove('on')); li.classList.add('on'); }, ondblclick: () => openBank(k, o) },
            icon('wave'), h('span', { class: 'name', text: `Bank #${k}` }), h('span', { class: 'meta', text: `${streams} · ${fmtBytes(bytes)}` }), isMusic ? h('span', { class: 'badge vice', text: 'music' }) : null);
          li._b = { k, o };
          list.append(li);
        }
        from += r.items.length;
      } catch (e) { info.textContent = e.message; }
      busy = false;
    };
    for (const [v, label] of [['', 'All'], ['0', 'Effects & speech'], ['1', 'Music']]) {
      seg.append(h('button', { class: v === music ? 'on' : '', onclick: (e) => { music = v; store.set('audio.music', v); [...seg.children].forEach((b) => b.classList.toggle('on', b === e.currentTarget)); more(true); } }, label));
    }
    navigable(list, { rows: rowsOf(list), open: (r) => openBank(r._b.k, r._b.o), menu: (r) => [{ label: 'Open', icon: 'wave', key: 'Enter', run: () => openBank(r._b.k, r._b.o) }, { label: 'Copy offset', icon: 'copy', run: () => copy(r._b.o) }] });
    body.addEventListener('scroll', () => { if (body.scrollTop + body.clientHeight > body.scrollHeight - 400 && from < total) more(false); });
    body.append(h('div', { class: 'section' }, seg), info, list);
    more(true);
  },

  text(body) {
    const box = searchBox('text', 'Search the game\'s text', (p) => TEXT.search(p), { filters: [
      { id: 'src', title: 'Where to search', options: [['', 'All text'], ['ui', 'Interface'], ['subs', 'Dialogue and posts']] },
    ] });
    TEXT.box = box;
    body.append(h('div', { class: 'section sticky' }, box.el),
      h('div', { class: 'section faint' }, 'The interface, and the other text tables: lines spoken in scenes, calls and on the street, posts and messages. English; by content or by label (its name or hash, e.g. ', h('code', { text: 'PM_INDX_SET' }), '). The first search reads every table once.'));
    TEXT.search(box.params());
    setTimeout(() => box.input.focus(), 0);
  },

  async cars(body) {
    const list = h('ul', { class: 'list' });
    const info = h('div', { class: 'faint', style: 'padding:6px 12px 2px;font-size:12px', text: 'Reading the vehicles…' });
    const run = (p) => {
      if (!CARS.all) return;
      const g = matcher(p), re = g && new RegExp(g.source, g.flags.replace('g', ''));
      const out = CARS.all.filter((c) => !re || re.test(c.name));
      info.textContent = `${out.length} of ${CARS.all.length} vehicles`;
      list.replaceChildren(...out.map((c) => {
        const li = h('li', { class: 'row', onclick: () => { rowsOf(list)().forEach((e) => e.classList.remove('on')); li.classList.add('on'); }, ondblclick: () => openShowroom(c.name) },
          icon('car'), h('span', { class: 'name' }, ...highlight(c.name, p)), h('span', { class: 'meta', title: 'Tuning parts', text: c.parts }));
        li._c = c;
        return li;
      }));
    };
    const box = searchBox('cars', 'Vehicle name', run);
    navigable(list, { rows: rowsOf(list), open: (r) => openShowroom(r._c.name), menu: (r) => [{ label: 'Open in the showroom', icon: 'car', key: 'Enter', run: () => openShowroom(r._c.name) }, { label: 'Open the body as a model', icon: 'cube', run: () => openModel(r._c.h, `${r._c.name}_hi`) }, '-', { label: 'Copy name', icon: 'copy', run: () => copy(r._c.name) }] });
    intoList(box.input, list);
    body.append(h('div', { class: 'section sticky' }, box.el, info), list);
    setTimeout(() => box.input.focus(), 0);
    try { CARS.all = CARS.all || (await api('/api/vehicles')).items; run(box.params()); } catch (e) { info.textContent = e.message; }
  },

  async people(body) {
    const list = h('ul', { class: 'list' });
    const info = h('div', { class: 'faint', style: 'padding:6px 12px 2px;font-size:12px', text: 'Reading the wardrobe…' });
    const mode = h('div', { class: 'seg' });
    const bodySel = h('select', { class: 'pick', title: 'Body' }, ...Object.entries(BODY_NAMES).map(([k, v]) => h('option', { value: k, text: v })));
    const slotSel = h('select', { class: 'pick', title: 'Slot' });
    let tab = store.get('people.mode', 'people');
    bodySel.value = store.get('people.body', 'mr1');
    const run = (p) => {
      const W = WARDROBE.data;
      if (!W) return;
      const g = matcher(p), re = g && new RegExp(g.source, g.flags.replace('g', ''));
      if (tab === 'people') {
        const out = W.people.filter((c) => !re || re.test(c.name));
        info.textContent = `${out.length} of ${W.people.length} characters and outfits`;
        list.replaceChildren(...out.map((c) => {
          const li = h('li', { class: 'row', onclick: () => { rowsOf(list)().forEach((e) => e.classList.remove('on')); li.classList.add('on'); }, ondblclick: () => dressAs(c) },
            icon('person'), h('span', { class: 'name' }, ...highlight(personName(c.name), p)), h('span', { class: 'meta', title: BODY_NAMES[c.body] || c.body, text: `${c.items.length}` }));
          li._open = () => dressAs(c);
          return li;
        }));
        return;
      }
      const body = bodySel.value, slot = slotSel.value;
      const out = W.items.filter((i) => i.body === body && (!slot || i.slot === slot) && (!re || re.test(i.name)));
      info.textContent = `${out.length} items`;
      list.replaceChildren(...out.slice(0, 1500).map((i) => {
        const li = h('li', { class: 'row', title: i.name, onclick: () => { rowsOf(list)().forEach((e) => e.classList.remove('on')); li.classList.add('on'); wear(i); }, ondblclick: () => openModel(i.h, i.name) },
          icon('person'), h('span', { class: 'name' }, ...highlight(i.kind, p), h('span', { class: 'sub', text: i.rest.replace(/_full$/, '').replace(/_/g, ' ') })), h('span', { class: 'meta', text: i.slot }));
        li._open = () => wear(i);
        return li;
      }));
    };
    const box = searchBox('people', 'Name, character or item', run);
    const paintMode = () => {
      mode.replaceChildren(...[['people', 'Characters'], ['items', 'Clothes']].map(([v, l]) => h('button', { class: tab === v ? 'on' : '', onclick: () => { tab = v; store.set('people.mode', v); paintMode(); run(box.params()); } }, l)));
      bodySel.classList.toggle('hidden', tab !== 'items');
      slotSel.classList.toggle('hidden', tab !== 'items');
    };
    bodySel.addEventListener('change', () => { store.set('people.body', bodySel.value); run(box.params()); });
    slotSel.addEventListener('change', () => { store.set('people.slot', slotSel.value); run(box.params()); });
    navigable(list, { rows: rowsOf(list), open: (r) => r._open() });
    intoList(box.input, list);
    body.append(h('div', { class: 'section sticky' }, h('div', { class: 'inline', style: 'margin-bottom:8px;flex-wrap:wrap' }, mode, bodySel, slotSel), box.el, info), list);
    paintMode();
    try {
      await WARDROBE.load();
      slotSel.replaceChildren(h('option', { value: '', text: 'Every slot' }), ...WARDROBE.data.slots.map((x) => h('option', { value: x, text: x })));
      slotSel.value = store.get('people.slot', '');
      run(box.params());
    } catch (e) { info.textContent = e.message; }
  },

  async animals(body) {
    const list = h('ul', { class: 'list' });
    const info = h('div', { class: 'faint', style: 'padding:6px 12px 2px;font-size:12px', text: 'Reading the animals…' });
    const run = (p) => {
      const all = CREATURES.data?.items;
      if (!all) return;
      const g = matcher(p), re = g && new RegExp(g.source, g.flags.replace('g', ''));
      const out = all.filter((c) => (!p.class || c.class === p.class) && (!re || re.test(c.name) || re.test(animalName(c.name)) || c.parts.some((x) => x.look && re.test(x.look))));
      info.textContent = `${out.length} of ${all.length} animals`;
      list.replaceChildren(...out.map((c) => {
        const looks = new Set(c.parts.map((x) => x.look).filter((l) => l && !FUR_LENGTHS.has(l)));
        const li = h('li', { class: 'row', title: c.name, onclick: () => { rowsOf(list)().forEach((e) => e.classList.remove('on')); li.classList.add('on'); }, ondblclick: () => openCreature(c) },
          icon('paw'), h('span', { class: 'name' }, ...highlight(animalName(c.name), p), h('span', { class: 'sub', text: `${c.class.toLowerCase()} · ${c.parts.length} parts${looks.size ? ` · ${looks.size} looks` : ''}` })));
        li._c = c;
        return li;
      }));
    };
    const box = searchBox('animals', 'Animal, breed or look (legendary, police…)', run, { filters: [
      { id: 'class', title: 'Kind', options: [['', 'Every kind'], ['Mammals', 'Mammals'], ['Birds', 'Birds'], ['Reptiles', 'Reptiles'], ['Fish', 'Fish'], ['Amphibians', 'Amphibians'], ['Crustaceans', 'Crustaceans'], ['Insects', 'Insects']] },
    ] });
    navigable(list, { rows: rowsOf(list), open: (r) => openCreature(r._c), menu: (r) => [{ label: 'Open', icon: 'paw', key: 'Enter', run: () => openCreature(r._c) }, { label: 'Copy name', icon: 'copy', run: () => copy(r._c.name) }] });
    intoList(box.input, list);
    body.append(h('div', { class: 'section sticky' }, box.el, info), list);
    setTimeout(() => box.input.focus(), 0);
    try { await CREATURES.load(); run(box.params()); } catch (e) { info.textContent = e.message; }
  },

  setup(body) {
    const st = S.status;
    body.append(h('div', { class: 'section' },
      h('div', { class: 'note ' + (st?.ok ? 'ok' : 'bad'), text: st?.ok ? 'Game folder found.' : 'No game folder yet.' }),
      h('div', { style: 'margin-top:10px' }, h('button', { class: 'btn', onclick: openSetup }, icon('gear'), 'Open setup'))));
    openSetup();
  },
};

// ---------------------------------------------------------------- model view

// Each mesh has its own placement (`mdl`): a vehicle's wheels are one
// rim drawn four times. Painted meshes take the chosen colour and a
// highlight; glass draws see-through.
// Skinning: a vertex follows up to four bones (`j`) by weight (`k`); the
// bones' matrices (pose times inverse bind, column by column) are rows of
// the float texture `bones`.
const SKIN = `
in vec4 j; in vec4 k;
uniform highp sampler2D bones; uniform int skinned;
mat4 bone(float i) { int r = int(i + 0.5); return mat4(texelFetch(bones, ivec2(0, r), 0), texelFetch(bones, ivec2(1, r), 0), texelFetch(bones, ivec2(2, r), 0), texelFetch(bones, ivec2(3, r), 0)); }
vec4 skin(vec3 p) { vec4 l = vec4(p, 1.0); return skinned == 1 ? (bone(j.x) * k.x + bone(j.y) * k.y + bone(j.z) * k.z + bone(j.w) * k.w) * l : l; }`;
const VS = `#version 300 es
in vec3 p; in vec2 t;${SKIN}
uniform mat4 mvp; uniform mat4 mdl;
out vec3 w; out vec2 uv;
void main() { vec4 q = mdl * skin(p); w = q.xyz; uv = t; gl_Position = mvp * q; }`;
const FS = `#version 300 es
precision highp float;
in vec3 w; in vec2 uv;
uniform sampler2D tex; uniform int useTex; uniform vec3 base; uniform vec3 eye;
uniform int usePaint; uniform vec3 paint; uniform float alpha;
out vec4 o;
void main() {
  vec3 n = normalize(cross(dFdx(w), dFdy(w)));
  vec3 v = normalize(eye - w);
  if (dot(n, v) < 0.0) n = -n;
  vec3 l = normalize(vec3(0.45, 0.3, 0.84));
  float d = max(dot(n, l), 0.0);
  float fill = 0.5 + 0.5 * n.z;
  vec4 c = useTex == 1 ? texture(tex, uv) : vec4(usePaint == 1 ? paint : base, 1.0);
  if (c.a < 0.3 && alpha >= 1.0) discard;
  vec3 col = c.rgb * (0.30 + 0.18 * fill + 0.72 * d);
  if (usePaint == 1) col += vec3(0.55) * pow(max(dot(reflect(-l, n), v), 0.0), 48.0) + vec3(0.08) * pow(1.0 - max(dot(n, v), 0.0), 3.0);
  o = vec4(col, alpha);
}`;
const LVS = `#version 300 es
in vec3 p;${SKIN}
uniform mat4 mvp; uniform mat4 mdl; void main() { gl_Position = mvp * mdl * skin(p); }`;
const IDENTITY = new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]);
const LFS = `#version 300 es
precision highp float; uniform vec4 color; out vec4 o; void main() { o = color; }`;

const M4 = {
  mul(a, b) { const o = new Float32Array(16); for (let c = 0; c < 4; c++) for (let r = 0; r < 4; r++) { let s = 0; for (let k = 0; k < 4; k++) s += a[k * 4 + r] * b[c * 4 + k]; o[c * 4 + r] = s; } return o; },
  persp(fov, aspect, n, f) { const t = 1 / Math.tan(fov / 2); return new Float32Array([t / aspect, 0, 0, 0, 0, t, 0, 0, 0, 0, (f + n) / (n - f), -1, 0, 0, (2 * f * n) / (n - f), 0]); },
  look(e, c, u) {
    const sub = (a, b) => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    const norm = (v) => { const l = Math.hypot(...v) || 1; return v.map((x) => x / l); };
    const cross = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    const f = norm(sub(c, e)), s = norm(cross(f, u)), uu = cross(s, f);
    return new Float32Array([s[0], uu[0], -f[0], 0, s[1], uu[1], -f[1], 0, s[2], uu[2], -f[2], 0, -dot(s, e), -dot(uu, e), dot(f, e), 1]);
  },
  // The inverse of a rigid matrix (rotation and translation).
  invRigid(m) {
    const o = new Float32Array(16);
    for (let r = 0; r < 3; r++) for (let c = 0; c < 3; c++) o[c * 4 + r] = m[r * 4 + c];
    for (let r = 0; r < 3; r++) o[12 + r] = -(o[r] * m[12] + o[4 + r] * m[13] + o[8 + r] * m[14]);
    o[15] = 1;
    return o;
  },
  // A turn of `a` radians about axis 0 (x), 1 (y) or 2 (z).
  rot(axis, a) {
    const c = Math.cos(a), s = Math.sin(a), o = new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]);
    const [i, k] = [[1, 2], [2, 0], [0, 1]][axis];
    o[i * 4 + i] = c; o[i * 4 + k] = s; o[k * 4 + i] = -s; o[k * 4 + k] = c;
    return o;
  },
  // The rotation of quaternion (x, y, z, w).
  quat([x, y, z, w]) {
    return new Float32Array([
      1 - 2 * (y * y + z * z), 2 * (x * y + z * w), 2 * (x * z - y * w), 0,
      2 * (x * y - z * w), 1 - 2 * (x * x + z * z), 2 * (y * z + x * w), 0,
      2 * (x * z + y * w), 2 * (y * z - x * w), 1 - 2 * (x * x + y * y), 0,
      0, 0, 0, 1]);
  },
  // Turns about x, then y, then z (degrees), as one matrix.
  euler(d) { const r = Math.PI / 180; return M4.mul(M4.rot(2, d[2] * r), M4.mul(M4.rot(1, d[1] * r), M4.rot(0, d[0] * r))); },
};

// Every 3D viewer made, to film the one shown (`viceLensFilm`).
const VIEWERS = new Set();

class Viewer {
  constructor(canvas) {
    this.canvas = canvas;
    VIEWERS.add(this);
    const gl = (this.gl = canvas.getContext('webgl2', { antialias: true }));
    if (!gl) throw new Error('WebGL 2 is not available in this browser');
    const prog = (vs, fs) => {
      const p = gl.createProgram();
      for (const [type, src] of [[gl.VERTEX_SHADER, vs], [gl.FRAGMENT_SHADER, fs]]) {
        const s = gl.createShader(type);
        gl.shaderSource(s, src);
        gl.compileShader(s);
        if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s));
        gl.attachShader(p, s);
      }
      gl.linkProgram(p);
      return p;
    };
    this.prog = prog(VS, FS);
    this.line = prog(LVS, LFS);
    this.meshes = [];
    this.tex = new Map();
    this.yaw = -0.9; this.pitch = 0.45; this.dist = 10; this.target = [0, 0, 0];
    this.useTex = true; this.wire = false;
    this.bindInput();
  }

  async load(hash, info) {
    this.upload(await bin('/api/model/mesh', { h: hash }), hash, info);
  }

  // Forget the model shown (the preview shows one after another).
  clear() {
    const gl = this.gl;
    for (const t of this.tex.values()) gl.deleteTexture(t);
    for (const b of this.buffers || []) gl.deleteBuffer(b);
    for (const m of this.meshes) { gl.deleteVertexArray(m.vao); gl.deleteVertexArray(m.lvao); }
    if (this.gridVao) gl.deleteVertexArray(this.gridVao);
    this.tex.clear();
    this.want?.clear();
    this.meshes = [];
    this.buffers = [];
    this.gridBuf = 0;
    this.gridVao = null;
    this.lo = this.hi = null;
    this.boneSum = null;
  }

  // The key of a model's shader texture (one viewer may show several models).
  tkey(hash, i) { return `${String(hash).replace(/^0x/, '').padStart(8, '0')}:${i}`; }

  // Add a model's meshes. `opts`: `matrix` (column-major) placing it,
  // `group` (shown and hidden together), `paint` (meshes without a
  // colour map of their own take the paint colour).
  upload(buf, hash, info, opts = {}) {
    const gl = this.gl;
    this.buffers = this.buffers || [];
    const mdl = opts.matrix || IDENTITY;
    const dv = new DataView(buf);
    let o = 8;
    // "VLM2": a model taken apart, its parts first (label, centre,
    // triangles), each mesh then naming its part. "VLM3": parts (maybe
    // none), and each mesh may carry its bones (see `setSkeleton`).
    const magic = String.fromCharCode(...new Uint8Array(buf, 0, 4));
    const apart = magic === 'VLM2' || magic === 'VLM3', v3 = magic === 'VLM3';
    const parts = [];
    if (apart) {
      const np = dv.getUint32(4, true);
      for (let k = 0; k < np; k++) {
        const len = dv.getUint16(o, true);
        const label = new TextDecoder().decode(new Uint8Array(buf, o + 2, len));
        o = (o + 2 + len + 3) & ~3;
        parts.push({ label, centre: [dv.getFloat32(o, true), dv.getFloat32(o + 4, true), dv.getFloat32(o + 8, true)], tris: dv.getUint32(o + 12, true) });
        o += 16;
      }
      if (np) this.parts = parts;
      o += 4;
    }
    const n = dv.getUint32(apart ? o - 4 : 4, true);
    let lo = this.lo ? [...this.lo] : [Infinity, Infinity, Infinity], hi = this.hi ? [...this.hi] : [-Infinity, -Infinity, -Infinity];
    for (let i = 0; i < n; i++) {
      const shader = dv.getUint32(o, true), nv = dv.getUint32(o + 4, true), ni = dv.getUint32(o + 8, true);
      const part = apart ? parts[dv.getUint32(o + 12, true)] : null;
      const skinned = v3 && dv.getUint32(o + 16, true) === 1;
      o += v3 ? 20 : apart ? 16 : 12;
      const pos = new Float32Array(buf, o, nv * 3); o += nv * 12;
      const uv = new Float32Array(buf, o, nv * 2); o += nv * 8;
      const idx = new Uint32Array(buf, o, ni); o += ni * 4;
      let joints = null, weights = null;
      if (skinned) {
        joints = new Uint16Array(buf, o, nv * 4); o += nv * 8;
        weights = new Uint8Array(buf, o, nv * 4); o += nv * 4;
        // A placeholder (hair, brows and fur keep one triangle where their
        // strands would be): its corners on different bones would stretch
        // across the body once it moves.
        if (ni <= 3 && opts.dropPlaceholders) continue;
        // Where each bone's vertices lie (by their heaviest bone), to tell
        // which way a hinged part opens.
        this.boneSum = this.boneSum || new Map();
        for (let q = 0; q < nv; q++) {
          const b = joints[q * 4];
          const s = this.boneSum.get(b) || [0, 0, 0, 0];
          s[0] += pos[q * 3]; s[1] += pos[q * 3 + 1]; s[2] += pos[q * 3 + 2]; s[3]++;
          this.boneSum.set(b, s);
        }
      }
      for (let k = 0; k < nv; k++) {
        const x = pos[k * 3], y = pos[k * 3 + 1], z = pos[k * 3 + 2];
        for (let c = 0; c < 3; c++) {
          const v = mdl[c] * x + mdl[4 + c] * y + mdl[8 + c] * z + mdl[12 + c];
          lo[c] = Math.min(lo[c], v); hi[c] = Math.max(hi[c], v);
        }
      }
      const vao = gl.createVertexArray();
      gl.bindVertexArray(vao);
      const put = (data, loc, size, type = gl.FLOAT, norm = false) => { if (loc < 0) return; const b = gl.createBuffer(); this.buffers.push(b); gl.bindBuffer(gl.ARRAY_BUFFER, b); gl.bufferData(gl.ARRAY_BUFFER, data, gl.STATIC_DRAW); gl.enableVertexAttribArray(loc); gl.vertexAttribPointer(loc, size, type, norm, 0, 0); };
      const bones = (prog) => { if (joints) { put(joints, gl.getAttribLocation(prog, 'j'), 4, gl.UNSIGNED_SHORT); put(weights, gl.getAttribLocation(prog, 'k'), 4, gl.UNSIGNED_BYTE, true); } };
      put(pos, gl.getAttribLocation(this.prog, 'p'), 3);
      put(uv, gl.getAttribLocation(this.prog, 't'), 2);
      bones(this.prog);
      const ib = gl.createBuffer();
      this.buffers.push(ib);
      gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, ib);
      gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, idx, gl.STATIC_DRAW);
      // Edges for the wireframe.
      const edges = new Uint32Array(ni * 2);
      for (let k = 0; k < ni; k += 3) edges.set([idx[k], idx[k + 1], idx[k + 1], idx[k + 2], idx[k + 2], idx[k]], k * 2);
      const lvao = gl.createVertexArray();
      gl.bindVertexArray(lvao);
      put(pos, gl.getAttribLocation(this.line, 'p'), 3);
      bones(this.line);
      const lb = gl.createBuffer();
      this.buffers.push(lb);
      gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, lb);
      gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, edges, gl.STATIC_DRAW);
      gl.bindVertexArray(null);
      const sh = info?.shaders?.[shader];
      const glass = sh?.bucket === 'glass';
      this.meshes.push({ shader, vao, lvao, count: ni, edges: edges.length, mdl, tkey: this.tkey(hash, shader), group: glass ? 'glass' : opts.group || 'model', glass, paint: !!opts.paint && !sh?.size && !glass, color: opts.color,
        part: part?.label, offset: part ? part.centre : opts.offset, tint: opts.tint, layer: opts.layer || 0, skinned, follow: opts.follow ?? null });
    }
    this.lo = lo; this.hi = hi;
    if (!opts.keepFrame) this.frame();
    (info?.shaders || []).forEach((s, i) => { if (s.size && !this.want?.has(this.tkey(hash, i))) this.texture(hash, i); });
    this.counts = { meshes: n, verts: 0 };
  }

  texture(hash, i) { this.setTexture(this.tkey(hash, i), `/api/model/texture?h=${hash}&s=${i}`); }

  // The skeleton skinned meshes follow, as `/api/skeleton` gives it: per
  // bone its name, parent and bind place. A bone's pose is its place
  // relative to its parent, turned by `pose` (bone → a turn in its frame).
  setSkeleton(sk) {
    const bones = sk.bones;
    const bind = bones.map((b) => new Float32Array(b.m));
    const local = bones.map((b, i) => (b.p >= 0 ? M4.mul(M4.invRigid(bind[b.p]), bind[i]) : bind[i]));
    this.sk = { names: bones.map((b) => b.n), parent: bones.map((b) => b.p), bind, inv: bind.map((m) => M4.invRigid(m)), local, index: new Map(bones.map((b, i) => [b.n, i])) };
    this.pose = new Map();
    this.skinDirty = true;
    this.dirty();
  }

  bone(name) { return this.sk?.index.get(name) ?? -1; }

  // Turn bone `i` by matrix `m` in its own frame (null: back to bind).
  setPose(i, m) {
    if (!this.sk || i < 0) return;
    if (m) this.pose.set(i, m); else this.pose.delete(i);
    this.skinDirty = true;
    this.dirty();
  }

  // Where a bone's vertices lie on average, it and the bones under it.
  boneMiddle(i) {
    if (!this.sk || !this.boneSum) return null;
    const s = [0, 0, 0, 0];
    for (const [b, x] of this.boneSum) {
      let a = b;
      for (let d = 0; d < 64 && a >= 0 && a !== i; d++) a = this.sk.parent[a];
      if (a === i) for (let c = 0; c < 4; c++) s[c] += x[c];
    }
    return s[3] ? [s[0] / s[3], s[1] / s[3], s[2] / s[3]] : null;
  }

  // A clip's pose (see `/api/anim/clip`): per bone its local rotation and
  // position at sample `s` (fractional: between two samples). The root's
  // pose is relative to its bind place (the game's clips face the other
  // way); every other bone's replaces its bind place. Null: back to bind.
  setClipPose(clip, s) {
    if (!this.sk) return;
    this.clipLocal = new Map();
    if (clip) {
      const n = clip.samples, s0 = Math.max(0, Math.min(n - 1, Math.floor(s))), s1 = Math.min(n - 1, s0 + 1), w = s - s0;
      for (const b of clip.bones) {
        const bind = this.sk.local[b.i];
        let q = null;
        if (b.r) {
          const a = b.r.subarray(s0 * 4, s0 * 4 + 4), c = b.r.subarray(s1 * 4, s1 * 4 + 4);
          const sg = a[0] * c[0] + a[1] * c[1] + a[2] * c[2] + a[3] * c[3] < 0 ? -1 : 1;
          q = [0, 1, 2, 3].map((k) => a[k] * (1 - w) + c[k] * w * sg);
          const l = Math.hypot(...q) || 1;
          q = q.map((x) => x / l);
        }
        const t = b.t ? [0, 1, 2].map((k) => b.t[s0 * 3 + k] * (1 - w) + b.t[s1 * 3 + k] * w) : null;
        const m = q ? M4.quat(q) : new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]);
        if (!q) for (const k of [0, 1, 2, 4, 5, 6, 8, 9, 10]) m[k] = bind[k];
        if (t) { m[12] = t[0]; m[13] = t[1]; m[14] = t[2]; } else if (this.sk.parent[b.i] >= 0) { m[12] = bind[12]; m[13] = bind[13]; m[14] = bind[14]; }
        this.clipLocal.set(b.i, this.sk.parent[b.i] < 0 ? M4.mul(bind, m) : m);
      }
    }
    this.skinDirty = true;
  }

  // Every bone's matrix (pose times inverse bind) into the bone texture.
  updateSkin() {
    const gl = this.gl, sk = this.sk, n = sk.names.length;
    const world = new Array(n);
    const at = (i) => {
      if (world[i]) return world[i];
      const base = this.clipLocal?.get(i) || sk.local[i];
      const d = this.pose.get(i), l = d ? M4.mul(base, d) : base;
      const p = sk.parent[i];
      return (world[i] = p >= 0 ? M4.mul(at(p), l) : l);
    };
    const data = new Float32Array(n * 16);
    this.skinMats = new Array(n);
    for (let i = 0; i < n; i++) { const s = M4.mul(at(i), sk.inv[i]); this.skinMats[i] = s; data.set(s, i * 16); }
    this.boneTexData(data, n);
    this.skinDirty = false;
  }

  boneTexData(data, n) {
    const gl = this.gl;
    if (!this.boneTex) this.boneTex = gl.createTexture();
    gl.activeTexture(gl.TEXTURE1);
    gl.bindTexture(gl.TEXTURE_2D, this.boneTex);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA32F, 4, n, 0, gl.RGBA, gl.FLOAT, data);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
    gl.activeTexture(gl.TEXTURE0);
  }

  // Drop the meshes of a group (the wheels, when others are chosen).
  removeGroup(g) {
    const gl = this.gl;
    for (const m of this.meshes.filter((x) => x.group === g)) { gl.deleteVertexArray(m.vao); gl.deleteVertexArray(m.lvao); }
    this.meshes = this.meshes.filter((x) => x.group !== g);
    this.dirty();
  }

  // Draw shader `i` with the image at `url` (a slot the user picked, or the
  // default colour map). A later call for the same shader wins.
  setTexture(i, url) {
    const gl = this.gl;
    const img = new Image();
    this.want = this.want || new Map();
    this.want.set(i, url);
    img.onerror = () => { if (this.want.get(i) === url) { this.tex.delete(i); this.dirty(); } };
    img.onload = () => {
      if (this.want.get(i) !== url || this.gone) return;
      const t = gl.createTexture();
      gl.bindTexture(gl.TEXTURE_2D, t);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, img);
      gl.generateMipmap(gl.TEXTURE_2D);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR_MIPMAP_LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.REPEAT);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.REPEAT);
      const old = this.tex.get(i);
      if (old) gl.deleteTexture(old);
      this.tex.set(i, t);
      this.dirty();
    };
    img.src = url;
  }

  frame() {
    if (!this.lo) return;
    this.target = this.lo.map((v, i) => (v + this.hi[i]) / 2);
    const r = Math.hypot(...this.hi.map((v, i) => v - this.lo[i])) / 2 || 1;
    this.dist = r * 2.6;
    this.dirty();
  }

  bindInput() {
    const c = this.canvas;
    let d = null;
    c.addEventListener('contextmenu', (e) => e.preventDefault());
    c.addEventListener('pointerdown', (e) => { d = { x: e.clientX, y: e.clientY, pan: e.button !== 0 || e.shiftKey }; c.setPointerCapture(e.pointerId); });
    c.addEventListener('pointermove', (e) => {
      if (!d) return;
      const dx = e.clientX - d.x, dy = e.clientY - d.y;
      d.x = e.clientX; d.y = e.clientY;
      if (d.pan) {
        const k = this.dist * 0.0016;
        const s = [Math.sin(this.yaw), -Math.cos(this.yaw)];
        this.target[0] -= (s[0] * dx) * k; this.target[1] -= (s[1] * dx) * k; this.target[2] += dy * k;
      } else {
        this.yaw -= dx * 0.008;
        this.pitch = Math.max(-1.5, Math.min(1.5, this.pitch + dy * 0.008));
      }
      this.dirty();
    });
    c.addEventListener('pointerup', () => (d = null));
    c.addEventListener('wheel', (e) => { e.preventDefault(); this.dist *= Math.pow(1.0015, e.deltaY); this.dirty(); }, { passive: false });
  }

  dirty() { cancelAnimationFrame(this.raf); this.raf = requestAnimationFrame(() => this.draw()); }

  draw() {
    const gl = this.gl, c = this.canvas;
    const dpr = window.devicePixelRatio || 1;
    const W = Math.round(c.clientWidth * dpr), H = Math.round(c.clientHeight * dpr);
    if (!W || !H) return;
    if (c.width !== W || c.height !== H) { c.width = W; c.height = H; }
    gl.viewport(0, 0, W, H);
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    gl.enable(gl.DEPTH_TEST);
    const eye = [
      this.target[0] + this.dist * Math.cos(this.pitch) * Math.cos(this.yaw),
      this.target[1] + this.dist * Math.cos(this.pitch) * Math.sin(this.yaw),
      this.target[2] + this.dist * Math.sin(this.pitch),
    ];
    const mvp = M4.mul(M4.persp(0.8, W / H, this.dist * 0.01, this.dist * 50), M4.look(eye, this.target, [0, 0, 1]));
    // Ground grid under the model.
    if (this.lo) this.drawGrid(mvp);
    // The bones: posed, or one bone standing still while there is no
    // skeleton (the bone texture is always bound to unit 1).
    if (this.sk && this.skinDirty) this.updateSkin();
    if (!this.boneTex) this.boneTexData(IDENTITY, 1);
    gl.activeTexture(gl.TEXTURE1);
    gl.bindTexture(gl.TEXTURE_2D, this.boneTex);
    gl.activeTexture(gl.TEXTURE0);
    const posed = !!this.sk;
    gl.useProgram(this.prog);
    const u = (name) => gl.getUniformLocation(this.prog, name);
    gl.uniform1i(u('bones'), 1);
    gl.uniform1i(u('tex'), 0);
    gl.uniformMatrix4fv(u('mvp'), false, mvp);
    gl.uniform3fv(u('eye'), eye);
    gl.uniform3fv(u('base'), [0.62, 0.65, 0.7]);
    gl.uniform3fv(u('paint'), this.paint || [0.62, 0.65, 0.7]);
    gl.enable(gl.POLYGON_OFFSET_FILL);
    gl.polygonOffset(1, 1);
    const hidden = this.hidden || new Set();
    const shown = this.meshes.filter((m) => !hidden.has(m.group) && !(m.part && hidden.has('part:' + m.part)));
    // Taken apart: each part moved out from the middle, along where it is.
    const k = this.explode || 0;
    const placed = (m) => {
      // A model on a bone (a rim on its wheel) moves with it.
      const base = m.follow != null && this.skinMats?.[m.follow] ? M4.mul(this.skinMats[m.follow], m.mdl) : m.mdl;
      if (!k || !m.offset) return base;
      const t = new Float32Array(base);
      t[12] += m.offset[0] * k; t[13] += m.offset[1] * k; t[14] += Math.max(0, m.offset[2]) * k * 0.6;
      return t;
    };
    // Solid first, then glass over it, see-through and not hiding what is behind.
    for (const glass of [false, true]) {
      if (glass) { gl.enable(gl.BLEND); gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA); gl.depthMask(false); }
      gl.uniform1f(u('alpha'), glass ? 0.35 : 1.0);
      for (const m of shown.filter((x) => x.glass === glass)) {
        // A mesh given a colour of its own (a tyre's rubber) wears it.
        const t = !m.color && this.useTex && this.tex.get(m.tkey);
        gl.uniform1i(u('useTex'), t ? 1 : 0);
        gl.uniform1i(u('usePaint'), m.color || (m.paint && (m.tint || this.paint)) ? 1 : 0);
        gl.uniform3fv(u('paint'), m.color || (m.paint && m.tint) || this.paint || [0.62, 0.65, 0.7]);
        gl.uniformMatrix4fv(u('mdl'), false, placed(m));
        gl.uniform1i(u('skinned'), m.skinned && posed ? 1 : 0);
        // Clothes worn over others win where they touch.
        gl.polygonOffset(1, 1 - m.layer * 6);
        if (t) { gl.activeTexture(gl.TEXTURE0); gl.bindTexture(gl.TEXTURE_2D, t); }
        gl.bindVertexArray(m.vao);
        gl.drawElements(gl.TRIANGLES, m.count, gl.UNSIGNED_INT, 0);
      }
      if (glass) { gl.disable(gl.BLEND); gl.depthMask(true); }
    }
    gl.disable(gl.POLYGON_OFFSET_FILL);
    if (this.wire) {
      gl.useProgram(this.line);
      gl.uniformMatrix4fv(gl.getUniformLocation(this.line, 'mvp'), false, mvp);
      gl.uniform4fv(gl.getUniformLocation(this.line, 'color'), [0.31, 0.88, 0.77, 0.55]);
      gl.uniform1i(gl.getUniformLocation(this.line, 'bones'), 1);
      gl.enable(gl.BLEND);
      gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
      for (const m of shown) {
        gl.uniformMatrix4fv(gl.getUniformLocation(this.line, 'mdl'), false, placed(m));
        gl.uniform1i(gl.getUniformLocation(this.line, 'skinned'), m.skinned && posed ? 1 : 0);
        gl.bindVertexArray(m.lvao);
        gl.drawElements(gl.LINES, m.edges, gl.UNSIGNED_INT, 0);
      }
      gl.disable(gl.BLEND);
    }
    gl.bindVertexArray(null);
  }

  drawGrid(mvp) {
    const gl = this.gl;
    if (!this.gridBuf) {
      const size = Math.max(...this.hi.map((v, i) => v - this.lo[i])) * 1.5 || 10;
      const step = Math.pow(10, Math.floor(Math.log10(size / 4)));
      const n = Math.ceil(size / step);
      const z = this.lo[2];
      const cx = (this.lo[0] + this.hi[0]) / 2, cy = (this.lo[1] + this.hi[1]) / 2;
      const v = [];
      for (let i = -n; i <= n; i++) {
        v.push(cx + i * step, cy - n * step, z, cx + i * step, cy + n * step, z);
        v.push(cx - n * step, cy + i * step, z, cx + n * step, cy + i * step, z);
      }
      this.gridVao = gl.createVertexArray();
      gl.bindVertexArray(this.gridVao);
      const b = gl.createBuffer();
      gl.bindBuffer(gl.ARRAY_BUFFER, b);
      gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(v), gl.STATIC_DRAW);
      const loc = gl.getAttribLocation(this.line, 'p');
      gl.enableVertexAttribArray(loc);
      gl.vertexAttribPointer(loc, 3, gl.FLOAT, false, 0, 0);
      this.gridBuf = v.length / 3;
      gl.bindVertexArray(null);
    }
    gl.useProgram(this.line);
    gl.uniformMatrix4fv(gl.getUniformLocation(this.line, 'mvp'), false, mvp);
    gl.uniformMatrix4fv(gl.getUniformLocation(this.line, 'mdl'), false, IDENTITY);
    gl.uniform1i(gl.getUniformLocation(this.line, 'skinned'), 0);
    gl.uniform4fv(gl.getUniformLocation(this.line, 'color'), [0.55, 0.6, 0.68, 0.18]);
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
    gl.bindVertexArray(this.gridVao);
    gl.drawArrays(gl.LINES, 0, this.gridBuf);
    gl.disable(gl.BLEND);
  }

  dispose() {
    this.gone = true;
    this.gl.getExtension('WEBGL_lose_context')?.loseContext();
  }
}

// The model under the pointer on the map, turning at the top of the
// inspector: one WebGL view, reused from model to model.
const PREVIEW = {
  el: null, viewer: null, title: null, sub: null, seq: 0, raf: 0, hash: null,
  ensure() {
    if (this.el) return true;
    this.el = $('#insp-preview');
    const canvas = h('canvas');
    this.title = h('div', { class: 'n' });
    this.sub = h('div', { class: 's' });
    this.el.append(h('div', { class: 'pv-stage' }, canvas),
      h('div', { class: 'pv-meta' }, h('div', { class: 'grow' }, this.title, this.sub),
        h('button', { class: 'icon-btn', title: 'Open the model', onclick: () => this.hash && openModel(this.hash, this.title.textContent) }, icon('cube')),
        h('button', { class: 'icon-btn', title: 'Close the preview (Esc on the map)', onclick: () => this.hide() }, icon('x'))));
    try { this.viewer = new Viewer(canvas); } catch { this.viewer = null; }
    return true;
  },
  // `p`: a placement as the map lists it, [entity, x, y, kind, name].
  show: debounce(function (p) { PREVIEW.load(p); }, 90),
  async load(p) {
    this.ensure();
    const mine = ++this.seq;
    this.el.classList.add('on');
    this.title.textContent = p[4];
    this.sub.textContent = 'Reading…';
    try {
      const place = await api('/api/placement', { i: p[0] });
      if (mine !== this.seq) return;
      if (!place.file) { this.hash = null; this.viewer?.clear(); this.viewer?.dirty(); this.sub.textContent = 'not in the model index'; return; }
      if (place.model === this.hash) { this.sub.textContent = this.subText; return; }
      const [info, buf] = await Promise.all([api('/api/model', { h: place.model }), bin('/api/model/mesh', { h: place.model })]);
      if (mine !== this.seq) return;
      this.hash = place.model;
      const tris = info.meshes.reduce((a, x) => a + x.tris, 0);
      this.subText = `${fmtNum(tris)} triangles · ${fx(info.max[0] - info.min[0])} × ${fx(info.max[1] - info.min[1])} × ${fx(info.max[2] - info.min[2])} m · ${fmtNum(info.placed)} placed`;
      this.sub.textContent = this.subText;
      if (!this.viewer) return;
      this.viewer.clear();
      this.viewer.upload(buf, place.model, info);
      this.viewer.pitch = 0.5;
      this.spin();
    } catch (e) {
      if (mine === this.seq) this.sub.textContent = e.message;
    }
  },
  spin() {
    cancelAnimationFrame(this.raf);
    let last = performance.now();
    const f = (t) => {
      if (!this.el.classList.contains('on') || !this.viewer) return;
      this.viewer.yaw += (t - last) * 0.0006;
      last = t;
      this.viewer.draw();
      this.raf = requestAnimationFrame(f);
    };
    this.raf = requestAnimationFrame(f);
  },
  hide() {
    if (!this.el) return;
    this.seq++;
    cancelAnimationFrame(this.raf);
    this.el.classList.remove('on');
  },
};

// Zoom (wheel) and pan (drag) an image on a stage; `fit` centres it whole.
function panZoom(stage, img, onZoom) {
  let z = 1, x = 0, y = 0, d = null;
  const apply = () => { img.style.transform = `translate(${x}px, ${y}px) scale(${z})`; onZoom?.(z); };
  const fit = () => {
    if (!img.naturalWidth) return;
    z = Math.min(1, (stage.clientWidth - 40) / img.naturalWidth, (stage.clientHeight - 40) / img.naturalHeight);
    x = (stage.clientWidth - img.naturalWidth * z) / 2; y = (stage.clientHeight - img.naturalHeight * z) / 2;
    apply();
  };
  const actual = () => {
    const cx = stage.clientWidth / 2, cy = stage.clientHeight / 2;
    x = cx - ((cx - x) * 1) / z; y = cy - ((cy - y) * 1) / z; z = 1;
    apply();
  };
  stage.addEventListener('wheel', (e) => {
    e.preventDefault();
    const r = stage.getBoundingClientRect();
    const px = e.clientX - r.left, py = e.clientY - r.top;
    const nz = Math.max(0.05, Math.min(32, z * Math.pow(1.0018, -e.deltaY)));
    x = px - ((px - x) * nz) / z; y = py - ((py - y) * nz) / z; z = nz;
    apply();
  }, { passive: false });
  stage.addEventListener('pointerdown', (e) => { if (e.button !== 0 || e.target.closest('button')) return; d = { x: e.clientX - x, y: e.clientY - y }; stage.setPointerCapture(e.pointerId); });
  stage.addEventListener('pointermove', (e) => { if (d) { x = e.clientX - d.x; y = e.clientY - d.y; apply(); } });
  stage.addEventListener('pointerup', () => (d = null));
  img.addEventListener('load', fit);
  return { fit, actual };
}

// Images looked through over a view, without leaving it: ← → (or the
// buttons) step, the wheel zooms, a drag pans, Esc closes. `items`:
// { src, title, meta, actions: [{ label, icon, key, run(item) }] }.
function lightbox(host, items, start) {
  host.querySelector(':scope > .lightbox')?._close();
  let i = start;
  const img = h('img', { alt: '', draggable: 'false' });
  const zoomEl = h('span', { class: 'faint mono' });
  const stage = h('div', { class: 'lb-stage checker' }, img);
  const pz = panZoom(stage, img, (z) => (zoomEl.textContent = `${Math.round(z * 100)}%`));
  const title = h('span', { class: 'title' }), meta = h('span', { class: 'faint mono' }), count = h('span', { class: 'faint mono' });
  const acts = h('span', { class: 'lb-acts' });
  const prev = h('button', { class: 'lb-nav l', title: 'Previous (←)', onclick: () => go(i - 1) }, icon('prev'));
  const next = h('button', { class: 'lb-nav r', title: 'Next (→)', onclick: () => go(i + 1) }, icon('next'));
  const el = h('div', { class: 'lightbox' },
    h('div', { class: 'toolbar' }, title, meta, h('span', { class: 'grow' }), count, zoomEl, h('button', { class: 'btn', onclick: () => pz.fit() }, icon('fit'), 'Fit'), h('button', { class: 'btn', onclick: () => pz.actual() }, '1:1'),
      acts, h('button', { class: 'btn icon-only', title: 'Close (Esc)', onclick: () => close() }, icon('x'))),
    stage, prev, next);
  const go = (k) => {
    i = (k + items.length) % items.length;
    const it = items[i];
    img.src = it.src;
    title.textContent = it.title;
    meta.textContent = it.meta || '';
    count.textContent = items.length > 1 ? `${i + 1} / ${items.length}` : '';
    prev.classList.toggle('hidden', items.length < 2);
    next.classList.toggle('hidden', items.length < 2);
    // The main action with its words, the others as icons.
    acts.replaceChildren(...(it.actions || []).map((a) => h('button', { class: 'btn' + (a.primary ? ' primary' : ' icon-only'), title: a.key ? `${a.label} (${a.key})` : a.label, onclick: () => a.run(it) }, icon(a.icon || 'dot'), a.primary ? a.label : null)));
  };
  const keys = (e) => {
    if (!el.isConnected || /INPUT|TEXTAREA|SELECT/.test(document.activeElement?.tagName)) return;
    if (e.key === 'Escape') close();
    else if (e.key === 'ArrowLeft') go(i - 1);
    else if (e.key === 'ArrowRight') go(i + 1);
    else {
      const a = (items[i].actions || []).find((x) => x.key && x.key.toLowerCase() === e.key.toLowerCase());
      if (!a) return;
      a.run(items[i]);
    }
    e.preventDefault();
    e.stopImmediatePropagation();
  };
  const close = () => { window.removeEventListener('keydown', keys, true); el.remove(); };
  el._close = close;
  window.addEventListener('keydown', keys, true);
  host.append(el);
  go(start);
  return { close, go };
}

// A model in 3D. `opts.replace`: the tab this one takes the place of (a
// variant picked in the toolbar), whose camera angle it keeps.
function openModel(hash, name, opts = {}) {
  hash = String(hash).replace(/^0x/, '').padStart(8, '0');
  const id = 'model:' + hash;
  if (S.tabs.find((t) => t.id === id)) { showTab(id); return; }
  const canvas = h('canvas');
  const msg = h('div', { class: 'center-msg', text: 'Reading the model…' });
  const texBtn = h('button', { class: 'btn icon-only on', title: 'Textures (T)' }, icon('image'));
  const wireBtn = h('button', { class: 'btn icon-only', title: 'Wireframe (W)' }, icon('grid'));
  const mapBtn = h('button', { class: 'btn', title: 'Where the map places it' }, icon('map'), 'Map');
  const exportBtn = h('button', { class: 'btn', title: 'Save the model with the textures shown, as glTF (.glb) for Blender and other tools (E)', disabled: true }, icon('export'), 'Export .glb');
  const variants = h('select', { class: 'pick hidden', title: 'Variants: models named like this one but for a letter (V)' });
  // What the viewer shows: per shader, the texture parameter picked
  // instead of the default colour map.
  const st = { hash, info: null, viewer: null, slots: null, choice: new Map(), texBtn };
  st.stage = h('div', { class: 'stage' }, canvas, msg, h('div', { class: 'help', text: 'Drag to orbit · right-drag or Shift to pan · wheel to zoom' }));
  const view = h('div', { class: 'model' },
    h('div', { class: 'toolbar' }, h('span', { class: 'title', text: name || hash }), variants, h('span', { class: 'grow' }), texBtn, wireBtn,
      h('button', { class: 'btn icon-only', title: 'Frame the model (F)', onclick: () => st.viewer?.frame() }, icon('fit')), mapBtn, exportBtn),
    st.stage);
  const tab = openTab({ id, title: name || hash, icon: 'cube', closable: true, view, state: st,
    show() { st.viewer?.dirty(); if (st.info) showModelInfo(st); },
    close() { st.viewer?.dispose(); window.removeEventListener('keydown', keys); } }, opts.replace);
  const keys = (e) => {
    if (S.active !== id || /INPUT|TEXTAREA|SELECT/.test(document.activeElement?.tagName) || e.ctrlKey || e.metaKey || e.altKey || st.stage.querySelector('.lightbox')) return;
    if (e.key === 'f') st.viewer?.frame();
    else if (e.key === 't') texBtn.click();
    else if (e.key === 'w') wireBtn.click();
    else if (e.key === 'e') exportBtn.click();
    else if ((e.key === 'v' || e.key === 'V') && variants.options.length > 1) {
      const i = variants.selectedIndex, n = variants.options.length;
      variants.selectedIndex = (i + (e.shiftKey ? n - 1 : 1)) % n;
      variants.dispatchEvent(new Event('change'));
    }
  };
  window.addEventListener('keydown', keys);
  texBtn.addEventListener('click', () => { const v = st.viewer; if (!v) return; v.useTex = !v.useTex; texBtn.classList.toggle('on', v.useTex); v.dirty(); });
  wireBtn.addEventListener('click', () => { const v = st.viewer; if (!v) return; v.wire = !v.wire; wireBtn.classList.toggle('on', v.wire); v.dirty(); });
  mapBtn.addEventListener('click', () => st.info && MAP.showModel(hash, st.info.name, st.info.places));
  exportBtn.addEventListener('click', () => {
    if (!st.info) return;
    const t = [...st.choice].map(([s, p]) => `${s}:${p}`).join(',');
    saveAs(`/api/model/export?h=${hash}${t ? `&t=${t}` : ''}`, `${fileName(st.info.name)}.glb`);
  });
  variants.addEventListener('change', () => {
    const o = variants.selectedOptions[0];
    if (!o || o.value === hash) return;
    const v = st.viewer;
    openModel(o.value, o.dataset.name, { replace: id, camera: v && { yaw: v.yaw, pitch: v.pitch } });
  });
  new ResizeObserver(() => st.viewer?.dirty()).observe(view);
  (async () => {
    try {
      st.info = await api('/api/model', { h: hash });
      tab.title = st.info.name;
      renderTabs();
      view.querySelector('.title').textContent = st.info.name;
      if (S.active === id) showModelInfo(st);
      st.viewer = new Viewer(canvas);
      if (opts.camera) Object.assign(st.viewer, opts.camera);
      await st.viewer.load(hash, st.info);
      msg.remove();
      exportBtn.disabled = false;
      const fam = await api('/api/model/family', { h: hash }).catch(() => ({ items: [] }));
      if (fam.items.length > 1) {
        variants.replaceChildren(...fam.items.map((x, i) => h('option', { value: x.h, 'data-name': x.name, text: `${i + 1}/${fam.items.length}  ${x.name}${x.placed ? `  · ${fmtNum(x.placed)} placed` : ''}` })));
        variants.value = hash;
        variants.classList.remove('hidden');
      }
    } catch (e) {
      msg.textContent = e.message;
    }
  })();
}

function showModelInfo(st) {
  const m = st.info;
  const tris = m.meshes.reduce((a, x) => a + x.tris, 0);
  const verts = m.meshes.reduce((a, x) => a + x.verts, 0);
  setStatus('sel', `${m.name}: ${fmtNum(tris)} triangles`);
  const textures = h('div', { class: 'slots' }, h('div', { class: 'faint', text: 'Reading the textures…' }));
  const pickAll = h('select', { class: 'pick', title: 'Show this texture on every shader that has it' });
  const places = h('ul', { class: 'list' }, ...m.places.slice(0, 80).map(([i, x, y, z]) =>
    h('li', { class: 'row', onclick: () => { MAP.goTo(x, y, 0.8); MAP.select(i); } }, icon('pin'), h('span', { class: 'name', text: `${fx(x)}, ${fx(y)}, ${fx(z)}` }), h('span', { class: 'meta', text: `#${i}` }))));
  inspect(
    h('div', { class: 'insp-title' }, h('div', { class: 'n', text: m.name }), h('div', { class: 's' }, h('span', { class: 'badge', text: m.h }), m.placed ? h('span', { class: 'badge ok', text: `${fmtNum(m.placed)} placed` }) : h('span', { class: 'badge', text: 'not placed' }))),
    h('div', { class: 'insp-actions' },
      m.places.length ? h('button', { class: 'btn primary', onclick: () => MAP.showModel(m.h, m.name, m.places) }, icon('map'), 'Show on map') : null,
      m.file ? h('button', { class: 'btn', onclick: () => openBytes(m.file.archive, m.file.offset, m.name) }, icon('hex'), 'Bytes') : null,
      h('button', { class: 'btn', onclick: () => copy(m.name) }, icon('copy'), 'Copy name')),
    h('div', { class: 'insp-h', text: 'Geometry' }),
    kv([['Meshes', m.meshes.length], ['Triangles', fmtNum(tris)], ['Vertices', fmtNum(verts)],
      ['Size', `${fx(m.max[0] - m.min[0])} × ${fx(m.max[1] - m.min[1])} × ${fx(m.max[2] - m.min[2])} m`], ['Radius', `${fx(m.radius)} m`],
      ['LOD distance', m.archetype ? `${fx(m.archetype.lod, 0)} m (archetype)` : 'unknown']]),
    h('div', { class: 'insp-h', text: `Textures · ${m.shaders.length} shader${m.shaders.length === 1 ? '' : 's'}` }),
    h('div', { class: 'insp-pick' }, h('span', { text: 'Show' }), pickAll),
    textures,
    m.file ? h('div', { class: 'insp-h', text: 'Stored in' }) : null,
    m.file ? kv([['Archive', m.file.archive], ['Offset', m.file.offset], ['Entry', m.file.entry ?? 'whole resource']]) : null,
    m.places.length ? h('div', { class: 'insp-h', text: `Placements${m.placed > 80 ? ` · first 80 of ${fmtNum(m.placed)}` : ''}` }) : null,
    places);
  const slotUrl = (si, p, max) => `/api/model/slot?h=${m.h}&s=${si}&p=${p}${max ? `&max=${max}` : ''}`;
  const shownOf = (sh) => st.choice.get(sh.shader) ?? sh.shown;
  // Show parameter `p` on shader `si` (null: the default colour map).
  const apply = (si, p) => {
    const sh = st.slots.shaders[si];
    const key = st.viewer?.tkey(m.h, si);
    if (p == null || p === sh.shown) { st.choice.delete(si); if (sh.shown || m.shaders[si]?.size) st.viewer?.texture(m.h, si); else st.viewer?.setTexture(key, ''); }
    else { st.choice.set(si, p); st.viewer?.setTexture(key, slotUrl(si, p, 2048)); }
    if (st.viewer && !st.viewer.useTex) st.texBtn?.click();
    paint();
  };
  const paint = () => {
    const sh = st.slots.shaders;
    // Every parameter some shader has, for the "show everywhere" choice.
    const seen = new Map();
    for (const x of sh) for (const t of x.slots) if (t.ok && !seen.has(t.p)) seen.set(t.p, t);
    pickAll.replaceChildren(h('option', { value: '', text: 'Colour maps the game picks' }), ...[...seen.values()].map((t) => h('option', { value: t.p, text: `${t.what || 'texture'} · ${t.name}` })));
    const common = st.choice.size && [...st.choice.values()].every((p) => p === [...st.choice.values()][0]) ? [...st.choice.values()][0] : '';
    pickAll.value = common;
    textures.replaceChildren(...sh.map((x) => h('div', {},
      h('div', { class: 'sh' }, h('b', { text: `Shader ${x.shader}` }), `${x.bucket} · ${x.meshes} piece${x.meshes === 1 ? '' : 's'}`,
        st.choice.has(x.shader) ? h('button', { class: 'reset', title: 'Back to the colour map the game picks', onclick: () => apply(x.shader, null) }, 'default') : null),
      x.slots.length ? h('div', { class: 'row-s' }, ...x.slots.map((t) => {
        const on = shownOf(x) === t.p;
        const look = () => browse(x.shader, t.p);
        const label = h('span', { class: 'l', title: `${t.name}${t.what ? ` (${t.what})` : ''}\n${t.w}×${t.h} ${t.format}, ${t.from === 'model' ? 'in the model' : t.from === 'pack' ? 'texture pack' : 'not found'}` },
          t.what || t.name, h('small', { text: t.ok ? `${t.w}×${t.h} ${t.format}` : t.from === 'missing' ? 'not found' : 'BCPack' }));
        if (!t.ok) return h('div', { class: 'slot none' }, h('div', { class: 'img', text: t.from === 'missing' ? 'not in the packs' : 'console codec' }), label);
        return h('div', { class: 'slot' + (on ? ' on' : ''), title: 'Click to look at it; the cube puts it on the model', onclick: look,
          oncontextmenu: (e) => contextMenu(e, [
            { label: 'Look at it', icon: 'image', run: look },
            { label: 'Show on the model', icon: 'cube', run: () => apply(x.shader, t.p) },
            { label: 'Open in a tab', icon: 'image', run: () => openImage(slotUrl(x.shader, t.p), `${m.name} · ${t.name}`, `${t.w}×${t.h} ${t.format}`) },
            { label: 'Save PNG…', icon: 'save', run: () => saveAs(slotUrl(x.shader, t.p), `${fileName(m.name)}-${x.shader}-${t.name}.png`) },
          ]) },
          h('div', { class: 'img', style: `background-image:url(${slotUrl(x.shader, t.p, 128)})` }), label,
          h('button', { class: 'use' + (on ? ' on' : ''), title: on ? 'Shown on the model' : 'Show on the model', onclick: (e) => { e.stopPropagation(); apply(x.shader, t.p); } }, icon('cube')));
      })) : h('div', { class: 'faint', style: 'font-size:11.5px', text: 'no textures' }))));
  };
  // Every texture of the model that decodes, looked through over the
  // model: each can go on the model, be saved, or open in a tab.
  const browse = (si, p) => {
    const all = [];
    for (const x of st.slots.shaders) for (const t of x.slots) if (t.ok) all.push({ x, t });
    const items = all.map(({ x, t }) => ({
      src: slotUrl(x.shader, t.p), title: `${t.what || t.name}`, meta: `shader ${x.shader} · ${t.name} · ${t.w}×${t.h} ${t.format}`, x, t,
      actions: [
        { label: 'Show on the model', icon: 'cube', key: 'Enter', primary: true, run: (it) => apply(it.x.shader, it.t.p) },
        { label: 'Save PNG', icon: 'save', key: 's', run: (it) => saveAs(slotUrl(it.x.shader, it.t.p), `${fileName(m.name)}-${it.x.shader}-${it.t.name}.png`) },
        { label: 'Open in a tab', icon: 'image', run: (it) => openImage(slotUrl(it.x.shader, it.t.p), `${m.name} · ${it.t.name}`, `${it.t.w}×${it.t.h} ${it.t.format}`) },
      ],
    }));
    const k = Math.max(0, all.findIndex(({ x, t }) => x.shader === si && t.p === p));
    if (items.length) lightbox(st.stage, items, k);
  };
  pickAll.addEventListener('change', () => {
    const p = pickAll.value;
    for (const x of st.slots.shaders) apply(x.shader, p && x.slots.some((t) => t.p === p && t.ok) ? p : null);
  });
  (st.slots ? Promise.resolve(st.slots) : api('/api/model/slots', { h: m.h })).then((r) => { st.slots = r; paint(); }).catch((e) => textures.replaceChildren(h('div', { class: 'faint', text: e.message })));
}

// ---------------------------------------------------------------- vehicles

const CARS = { all: null, wheels: null };

// Paints to start from; any colour can be picked.
const PAINTS = [
  ['Black', '#0b0c0e'], ['White', '#e8e8e6'], ['Silver', '#9ea3a8'], ['Gunmetal', '#3d4247'], ['Red', '#a3121c'], ['Candy red', '#d01830'],
  ['Orange', '#e05a12'], ['Yellow', '#e8b812'], ['Lime', '#7cc21c'], ['Green', '#1f5c2e'], ['Teal', '#15898a'], ['Blue', '#1b3fa0'],
  ['Sky blue', '#3d8fd8'], ['Purple', '#5a2a8c'], ['Hot pink', '#e0368f'], ['Beige', '#c8b48a'], ['Brown', '#5c3a22'], ['Gold', '#b88a2a'],
];

const hexRgb = (c) => [1, 3, 5].map((i) => Math.pow(parseInt(c.slice(i, i + 2), 16) / 255, 1.0));

// Where each kind of tuning part is built: most in the vehicle's own space
// (spoilers, skirts, roofs, diffusers…), a few around a bone, the first of
// these the vehicle has; and the part of the body a whole part replaces.
const TUNING = {
  bonnet: { bones: ['bonnet'], replaces: ['Bonnet'] },
  scoop: { bones: ['bonnet'] },
  bumf: { bones: ['bumper_f'], replaces: ['Front bumper'] },
  bumr: { bones: ['bumper_r'], replaces: ['Rear bumper'] },
  splitter: { bones: ['bumper_f'] },
  grille: { bones: ['bumper_f'] },
  bullbar: { bones: ['bumper_f'] },
  exhaust: { bones: ['exhaust'], right: 'exhaust_2', replaces: ['Exhaust, left', 'Exhaust, right'] },
};

// Where a tuning part goes: its bone's place in the vehicle (column-major,
// as the viewer takes it), or the vehicle's own space. Right-hand parts
// (`_r_`) of a kind with a right bone take that one.
function tuneMatrix(car, kind, name) {
  const b = tuneBone(car, kind, name);
  return b ? new Float32Array(car.skeleton[b]) : IDENTITY;
}

// The bone a tuning part hangs on (it moves with it: a bonnet opens), or
// null for parts built in the vehicle's own space.
function tuneBone(car, kind, name) {
  const sk = car.skeleton, t = TUNING[kind];
  if (!t) return null;
  if (t.right && /_r(_|$)/.test(name) && sk[t.right]) return t.right;
  return t.bones.find((b) => sk[b]) || null;
}

// What moves on a vehicle, by bone name: hinged parts (the axis in the
// bone's frame and how far they open, in degrees; which way is worked out
// from where the part lies) and the wheels.
const HINGES = [
  { re: /^door_[dp]side_[fmr]$/, what: 'doors', axis: 2, max: 68, out: 'side' },
  { re: /^bonnet$/, what: 'bonnet', axis: 0, max: 52, out: 'up' },
  { re: /^boot$/, what: 'boot', axis: 0, max: 62, out: 'up' },
];

// A vehicle in the showroom: its body, wheels the user picks (rim and tyre
// from the game's catalogue, on the wheel bones, sized to the vehicle),
// a paint, and the body, glass and wheels shown or hidden.
async function openShowroom(name, replace) {
  const id = 'car:' + name;
  if (S.tabs.find((t) => t.id === id)) { showTab(id); return; }
  const canvas = h('canvas');
  const msg = h('div', { class: 'center-msg', text: 'Bringing the vehicle in…' });
  const st = { name, car: null, viewer: null, paint: store.get('car.paint', '#a3121c'), rim: store.get('car.rim', 'street'), rimN: 0, tyre: store.get('car.tyre', 'car'), size: null, spin: store.get('car.spin', true), raf: 0, seq: 0, tuned: {} };
  const explodeIn = h('input', { type: 'range', min: '0', max: '1.2', step: '0.01', value: '0', title: 'Take the vehicle apart: each part moves out from the middle' });
  const swatches = h('div', { class: 'swatches' }, ...PAINTS.map(([n, c]) => h('button', { class: 'sw', title: n, style: `background:${c}`, onclick: () => setPaint(c) })));
  const custom = h('input', { type: 'color', value: st.paint, title: 'Any colour' });
  const rimSel = h('select', { class: 'pick', title: 'Rim style' });
  const rimLabel = h('span', { class: 'faint mono' });
  const tyreSel = h('select', { class: 'pick', title: 'Tyre' });
  const sizeIn = h('input', { type: 'range', min: '0.24', max: '0.8', step: '0.005', title: 'Wheel size (radius)' });
  const toggles = {};
  const toggle = (g, label) => (toggles[g] = h('button', { class: 'btn on', title: `Show or hide the ${label.toLowerCase()}`, onclick: () => { const v = st.viewer; if (!v) return; v.hidden = v.hidden || new Set(); if (v.hidden.has(g)) v.hidden.delete(g); else v.hidden.add(g); toggles[g].classList.toggle('on', !v.hidden.has(g)); v.dirty(); } }, label));
  const spinBtn = h('button', { class: 'btn icon-only' + (st.spin ? ' on' : ''), title: 'Turn slowly', onclick: () => { st.spin = !st.spin; store.set('car.spin', st.spin); spinBtn.classList.toggle('on', st.spin); spin(); } }, icon('spin'));
  // Moving parts: each hinged kind opens or closes over most of a second;
  // driving turns the wheels, the steering turns the front ones and the
  // steering wheel. All on the vehicle's own bones.
  const motion = { doors: 0, bonnet: 0, boot: 0, want: { doors: 0, bonnet: 0, boot: 0 }, drive: false, steer: 0, roll: 0, hinges: null };
  const moveBtn = (what, label, key) => h('button', { class: 'btn hidden', 'data-move': what, title: `Open or close the ${label.toLowerCase()} (${key})`, onclick: () => toggleMove(what) }, label);
  const moveBtns = [moveBtn('doors', 'Doors', 'D'), moveBtn('bonnet', 'Bonnet', 'B'), moveBtn('boot', 'Boot', 'T')];
  const driveBtn = h('button', { class: 'btn hidden', title: 'Drive: the wheels turn (R)', onclick: () => { motion.drive = !motion.drive; driveBtn.classList.toggle('on', motion.drive); spin(); } }, 'Drive');
  const steerIn = h('input', { type: 'range', min: '-35', max: '35', step: '0.5', value: '0', class: 'hidden', title: 'Steering (double-click: straight)' });
  const toggleMove = (what) => {
    motion.want[what] = motion.want[what] ? 0 : 1;
    for (const b of moveBtns) b.classList.toggle('on', !!motion.want[b.dataset.move]);
    spin();
  };
  steerIn.addEventListener('input', () => { motion.steer = +steerIn.value; applyMotion(); st.viewer?.dirty(); });
  steerIn.addEventListener('dblclick', () => { steerIn.value = '0'; motion.steer = 0; applyMotion(); st.viewer?.dirty(); });
  const moving = () => motion.drive || ['doors', 'bonnet', 'boot'].some((k) => motion[k] !== motion.want[k]);
  const ease = (x) => x * x * (3 - 2 * x);
  // Each hinged bone: its axis, the sign that opens it, how far.
  const hinges = () => {
    const v = st.viewer;
    if (motion.hinges || !v?.sk) return motion.hinges || [];
    motion.hinges = [];
    for (const [name, i] of v.sk.index) {
      const hg = HINGES.find((x) => x.re.test(name));
      if (!hg) continue;
      const mid = v.boneMiddle(i);
      if (!mid) continue;
      const bind = v.sk.bind[i], inv = v.sk.inv[i];
      // Where the part's middle goes when turned a little either way.
      const turned = (a) => {
        const m = M4.mul(bind, M4.mul(M4.rot(hg.axis, a), inv));
        return [0, 1, 2].map((c) => m[c] * mid[0] + m[4 + c] * mid[1] + m[8 + c] * mid[2] + m[12 + c]);
      };
      const measure = (p) => (hg.out === 'up' ? p[2] : p[0] * Math.sign(bind[12] || 1));
      const sign = measure(turned(0.1)) >= measure(turned(-0.1)) ? 1 : -1;
      motion.hinges.push({ i, what: hg.what, axis: hg.axis, max: hg.max * sign });
    }
    for (const b of moveBtns) b.classList.toggle('hidden', !motion.hinges.some((x) => x.what === b.dataset.move));
    return motion.hinges;
  };
  const applyMotion = () => {
    const v = st.viewer;
    if (!v?.sk) return;
    for (const hg of hinges()) {
      const k = ease(motion[hg.what]);
      if (k) v.pose.set(hg.i, M4.rot(hg.axis, (hg.max * k * Math.PI) / 180)); else v.pose.delete(hg.i);
    }
    const r = Math.PI / 180;
    for (const [name, i] of v.sk.index) {
      if (/^wheel_[lr][fmr]$/.test(name)) {
        const front = /f$/.test(name);
        const turn = M4.rot(0, motion.roll);
        v.pose.set(i, front && motion.steer ? M4.mul(M4.rot(2, motion.steer * r), turn) : turn);
      } else if (name === 'steeringwheel') {
        if (motion.steer) v.pose.set(i, M4.rot(1, -motion.steer * 8 * r)); else v.pose.delete(i);
      }
    }
    v.skinDirty = true;
  };
  const stepMotion = (dt) => {
    for (const k of ['doors', 'bonnet', 'boot']) {
      const d = motion.want[k] - motion[k], s = dt / 0.9;
      motion[k] = Math.abs(d) <= s ? motion.want[k] : motion[k] + Math.sign(d) * s;
    }
    // Rolling forwards at about 18 km/h.
    if (motion.drive) motion.roll -= (dt * 5) / (st.size || 0.35);
    applyMotion();
  };
  const stage = h('div', { class: 'stage showroom' }, canvas, msg, h('div', { class: 'help', text: 'Drag to orbit · right-drag or Shift to pan · wheel to zoom' }));
  const view = h('div', { class: 'model' },
    h('div', { class: 'toolbar' }, h('span', { class: 'title', text: name }), h('span', { class: 'grow' }),
      h('span', { class: 'faint', text: 'Apart' }), explodeIn,
      toggle('model', 'Body'), toggle('glass', 'Glass'), toggle('wheels', 'Wheels'), h('span', { class: 'sep' }), ...moveBtns, driveBtn, steerIn, spinBtn,
      h('button', { class: 'btn icon-only', title: 'Frame (F)', onclick: () => st.viewer?.frame() }, icon('fit'))),
    h('div', { class: 'toolbar sub' },
      h('span', { class: 'faint', text: 'Paint' }), swatches, custom, h('span', { class: 'sep' }),
      h('span', { class: 'faint', text: 'Rims' }), rimSel,
      h('button', { class: 'btn icon-only', title: 'Previous rim ([)', onclick: () => stepRim(-1) }, icon('prev')), rimLabel,
      h('button', { class: 'btn icon-only', title: 'Next rim (])', onclick: () => stepRim(1) }, icon('next')),
      h('span', { class: 'faint', text: 'Tyres' }), tyreSel, h('span', { class: 'faint', text: 'Size' }), sizeIn),
    stage);
  explodeIn.addEventListener('input', () => { if (st.viewer) { st.viewer.explode = +explodeIn.value; st.viewer.dirty(); } });
  const setPaint = (c) => { st.paint = c; custom.value = c; store.set('car.paint', c); if (st.viewer) { st.viewer.paint = hexRgb(c); st.viewer.dirty(); } };
  custom.addEventListener('input', () => setPaint(custom.value));
  // One loop for turning the view and moving the parts, running while
  // either is on.
  const spin = () => {
    cancelAnimationFrame(st.raf);
    if (!st.spin && !moving()) return;
    let last = performance.now();
    const f = (t) => {
      if (!st.viewer || S.active !== id) return;
      const dt = Math.min(0.25, (t - last) / 1000);
      last = t;
      if (st.spin) st.viewer.yaw += dt * 0.25;
      stepMotion(dt);
      st.viewer.draw();
      if (st.spin || moving()) st.raf = requestAnimationFrame(f);
    };
    st.raf = requestAnimationFrame(f);
  };
  const keys = (e) => {
    if (S.active !== id || /INPUT|TEXTAREA|SELECT/.test(document.activeElement?.tagName) || e.ctrlKey || e.metaKey || e.altKey) return;
    if (e.key === '[') stepRim(-1);
    else if (e.key === ']') stepRim(1);
    else if (e.key === 'f') st.viewer?.frame();
    else if (e.key === 'd') toggleMove('doors');
    else if (e.key === 'b') toggleMove('bonnet');
    else if (e.key === 't') toggleMove('boot');
    else if (e.key === 'r') driveBtn.click();
    else return;
    e.preventDefault();
  };
  window.addEventListener('keydown', keys);
  openTab({ id, title: name, icon: 'car', closable: true, view,
    show: () => { st.viewer?.dirty(); spin(); if (st.car) carInfo(); },
    hide: () => cancelAnimationFrame(st.raf),
    close: () => { cancelAnimationFrame(st.raf); st.viewer?.dispose(); window.removeEventListener('keydown', keys); } }, replace);
  new ResizeObserver(() => st.viewer?.dirty()).observe(view);
  // Unit wheels (a metre across) scaled so the tyre's outer radius is the
  // wheel's: tyres are 1.58 m across around a 1 m rim.
  const models = new Map();
  const model = async (h0) => {
    if (!models.has(h0)) models.set(h0, Promise.all([api('/api/model', { h: h0 }), bin('/api/model/mesh', { h: h0 })]));
    return models.get(h0);
  };
  const placeWheels = async () => {
    const v = st.viewer, car = st.car, W = CARS.wheels;
    if (!v || !car || !W) return;
    const mine = ++st.seq;
    const rims = W.rims[st.rim] || [];
    st.rimN = Math.max(0, Math.min(rims.length - 1, st.rimN));
    rimLabel.textContent = rims.length ? `${st.rimN + 1}/${rims.length}` : '–';
    const rim = rims[st.rimN], tyre = (W.tyres[st.tyre] || [])[0];
    // Rims in brushed metal, tyres in rubber: their own maps are material
    // data the game shades, not colour.
    const parts = await Promise.all([[rim, [0.55, 0.56, 0.58]], [tyre, [0.05, 0.05, 0.055]]].filter((x) => x[0]).map(([x, color]) => model(x.h).then((m) => [x, color, ...m])));
    if (mine !== st.seq) return;
    v.removeGroup('wheels');
    const r = st.size;
    for (const w of car.wheels) {
      // Left wheels (−x) turned round, their face outwards. The tyre's
      // outer radius in its own metre-wide units.
      const left = w.at[0] < 0;
      const s = r / 0.789;
      const m = new Float32Array([left ? -s : s, 0, 0, 0, 0, left ? -s : s, 0, 0, 0, 0, s, 0, w.at[0], w.at[1], w.at[2], 1]);
      // On the wheel's bone, turning and steering with it.
      for (const [x, color, info, buf] of parts) v.upload(buf, x.h, info, { matrix: m, group: 'wheels', keepFrame: true, color, offset: [w.at[0] * 1.6, w.at[1] * 0.4, 0], follow: v.bone(w.bone) });
    }
    v.dirty();
  };
  const stepRim = (d) => { const n = (CARS.wheels?.rims[st.rim] || []).length; if (!n) return; st.rimN = (st.rimN + d + n) % n; store.set('car.rimN', st.rimN); placeWheels(); };
  rimSel.addEventListener('change', () => { st.rim = rimSel.value; st.rimN = 0; store.set('car.rim', st.rim); placeWheels(); });
  tyreSel.addEventListener('change', () => { st.tyre = tyreSel.value; store.set('car.tyre', st.tyre); placeWheels(); });
  sizeIn.addEventListener('input', debounce(() => { st.size = +sizeIn.value; placeWheels(); }, 60));
  // A tuning part on the vehicle (or the stock part back): on the bone of
  // its kind, the body's own part it replaces hidden.
  const tune = async (kind, part) => {
    const v = st.viewer;
    if (!v) return;
    v.removeGroup('tune:' + kind);
    v.hidden = v.hidden || new Set();
    for (const r of TUNING[kind]?.replaces || []) v.hidden.delete('part:' + r);
    if (!part) { delete st.tuned[kind]; v.dirty(); carInfo(); return; }
    st.tuned[kind] = part.h;
    const [info, buf] = await model(part.h);
    if (st.tuned[kind] !== part.h) return;
    const bone = tuneBone(st.car, kind, part.name);
    v.upload(buf, part.h, info, { matrix: tuneMatrix(st.car, kind, part.name), group: 'tune:' + kind, keepFrame: true, paint: true, follow: bone ? v.bone(bone) : null });
    // A whole part (a bonnet, a bumper across the car) takes the stock
    // one's place; an add-on (a canard) goes on top of it.
    if (info.max[0] - info.min[0] > 0.6 || kind === 'exhaust') for (const r of TUNING[kind]?.replaces || []) v.hidden.add('part:' + r);
    v.dirty();
    carInfo();
  };
  const carInfo = () => {
    const c = st.car, v = st.viewer;
    const kinds = Object.entries(c.parts);
    const hidden = v?.hidden || new Set();
    const parts = (v?.parts || []).slice().sort((a, b) => a.label.localeCompare(b.label));
    const eye = (key, label, extra) => {
      const on = !hidden.has(key);
      return h('li', { class: 'row part' + (on ? '' : ' off'), title: on ? 'Hide it' : 'Show it',
        onclick: () => { v.hidden = v.hidden || new Set(); if (v.hidden.has(key)) v.hidden.delete(key); else v.hidden.add(key); v.dirty(); carInfo(); },
        oncontextmenu: (e) => contextMenu(e, [
          { label: 'Show only this', icon: 'cube', run: () => { v.hidden = new Set([...parts.map((p) => 'part:' + p.label), 'glass', 'wheels', 'model'].filter((k) => k !== key)); v.dirty(); carInfo(); } },
          { label: 'Show everything', icon: 'grid', run: () => { v.hidden = new Set(); v.dirty(); carInfo(); } },
        ]) },
        h('span', { class: 'eye' }, on ? '●' : '○'), h('span', { class: 'name', text: label }), extra ? h('span', { class: 'meta', text: extra }) : null);
    };
    inspect(
      h('div', { class: 'insp-title' }, h('div', { class: 'n', text: c.name }), h('div', { class: 's' }, h('span', { class: 'badge', text: `${c.bones} bones` }), h('span', { class: 'badge', text: `${c.wheels.length} wheels` }))),
      h('div', { class: 'insp-actions' },
        h('button', { class: 'btn', onclick: () => openModel(c.body, `${c.name}_hi`) }, icon('cube'), 'Body as a model'),
        h('button', { class: 'btn', title: 'Every part shown again', onclick: () => { v.hidden = new Set(); v.dirty(); carInfo(); Object.values(toggles).forEach((b) => b.classList.add('on')); } }, 'Show all')),
      kv([['Size', `${fx(c.max[0] - c.min[0])} × ${fx(c.max[1] - c.min[1])} × ${fx(c.max[2] - c.min[2])} m`], ['Wheel radius', `${fx(st.size, 2)} m (estimated; Size sets it)`]]),
      h('div', { class: 'insp-h', text: `Parts · ${parts.length}` }),
      h('div', { class: 'empty', style: 'padding:2px 14px 6px', text: 'Click a part to hide it; right-click to see it alone. "Apart" in the toolbar spreads them out. Parts are the body\'s pieces, named by the bone they sit nearest.' }),
      h('ul', { class: 'list' }, ...parts.map((p) => eye('part:' + p.label, p.label, fmtNum(p.tris))), eye('wheels', 'Wheels'), eye('glass', 'Glass')),
      h('div', { class: 'insp-h', text: `Tuning · ${kinds.reduce((a, [, l]) => a + l.length, 0)} parts` }),
      h('div', { class: 'empty', style: 'padding:2px 14px 6px', text: 'Each kind on its bone; a bonnet, bumper or exhaust takes the place of the stock one. Right-click a choice to open it as a model.' }),
      ...kinds.map(([k, list]) => {
        const sel = h('select', { class: 'pick', title: k }, h('option', { value: '', text: 'Stock' }), ...list.map((p) => h('option', { value: p.h, text: p.name.slice(c.name.length + 1) })));
        sel.value = st.tuned[k] || '';
        sel.addEventListener('change', () => tune(k, list.find((p) => p.h === sel.value)));
        sel.addEventListener('contextmenu', (e) => { const p = list.find((x) => x.h === sel.value); if (p) contextMenu(e, [{ label: `Open ${p.name}`, icon: 'cube', run: () => openModel(p.h, p.name) }]); });
        return h('div', { class: 'insp-pick' }, h('span', { class: 'tk', text: k }), sel);
      }));
  };
  try {
    const [car, wheels] = await Promise.all([api('/api/vehicle', { name }), CARS.wheels ? Promise.resolve(CARS.wheels) : api('/api/wheels')]);
    CARS.wheels = wheels;
    st.car = car;
    rimSel.replaceChildren(...Object.entries(wheels.rims).filter(([k, v]) => v.length > 1 && !/bicycle|motorbike|train|aerocover|spare|misc/.test(k)).map(([k, v]) => h('option', { value: k, text: `${k.replace(/_/g, ' ')} (${v.length})` })));
    if (!wheels.rims[st.rim]) st.rim = rimSel.options[0]?.value;
    rimSel.value = st.rim;
    st.rimN = store.get('car.rimN', 0);
    tyreSel.replaceChildren(...Object.keys(wheels.tyres).filter((k) => !/bicycle|motorbike|aircraft|quad/.test(k)).map((k) => h('option', { value: k, text: k.replace(/_/g, ' ') })));
    if (!wheels.tyres[st.tyre]) st.tyre = 'car';
    tyreSel.value = st.tyre;
    // A wheel's radius from the body's height (no handling data read
    // yet): about a third of a metre on a sports car, more on trucks.
    st.size = Math.max(0.28, Math.min(0.6, (car.max[2] - car.min[2]) * 0.26 + 0.08));
    sizeIn.value = st.size;
    // The body taken apart into its pieces (doors, seats, bonnet…), each
    // vertex on the bone it moves with.
    const [info, buf, skel] = await Promise.all([api('/api/model', { h: car.body }), bin('/api/model/mesh', { h: car.body, apart: 1, skel: car.body }), api('/api/skeleton', { h: car.body }).catch(() => null)]);
    st.viewer = new Viewer(canvas);
    st.viewer.paint = hexRgb(st.paint);
    // Filming steps the moving parts with the frames (`viceLensFilm`).
    st.viewer.onFilm = (dt) => stepMotion(dt);
    if (skel) st.viewer.setSkeleton(skel);
    st.viewer.upload(buf, car.body, info, { group: 'model', paint: true });
    hinges();
    const rolls = car.wheels.length > 0 && !!skel;
    driveBtn.classList.toggle('hidden', !rolls);
    steerIn.classList.toggle('hidden', !rolls);
    st.viewer.pitch = 0.32;
    st.viewer.yaw = -0.7;
    msg.remove();
    await placeWheels();
    st.viewer.frame();
    if (S.active === id) { carInfo(); spin(); }
  } catch (e) {
    msg.textContent = e.message;
  }
}

// ---------------------------------------------------------------- people

const BODY_NAMES = { mr1: 'Men', fr1: 'Women', mb1: 'Men, bigger', fb1: 'Women, bigger', mo1: 'Men, older', fo1: 'Women, older', mk1: 'Boys', mc1: 'Boys, younger', mi1: 'Babies', fc1: 'Girls' };

const WARDROBE = {
  data: null,
  async load() { if (!this.data) this.data = await api('/api/wardrobe'); return this.data; },
};

// A character's name from its prefix: `cs_cesar_navarro` → "Cesar Navarro";
// the protagonists' outfits and crowd faces say so.
function personName(p) {
  const m = p.match(/^player_story(\d+)_(\d+)$/);
  if (m) return `Protagonist ${+m[2] + 1} (story ${m[1]})`;
  const words = p.replace(/^cs_/, '').replace(/^hc_/, 'crowd: ').replace(/^sc_/, 'special: ').replace(/^u_[mf]_[my]_/, 'unique: ').replace(/_/g, ' ');
  return words.replace(/\b[a-z]/g, (c) => c.toUpperCase());
}

// Colours for what wears no colour map of its own: clothes are coloured
// by masks and palettes the game applies, so each slot gets a plain one.
const SLOT_TINT = {
  Head: [0.78, 0.6, 0.5], Eyes: [0.86, 0.85, 0.82], 'Eye surface': [0.9, 0.9, 0.9], Body: [0.78, 0.6, 0.5], Mannequin: [0.85, 0.84, 0.8], 'Whole person (older style)': [0.78, 0.6, 0.5],
  Hair: [0.16, 0.11, 0.08], 'Facial hair': [0.16, 0.11, 0.08], 'Brows and lashes': [0.16, 0.11, 0.08], 'Body hair': [0.2, 0.14, 0.1],
  Underwear: [0.9, 0.9, 0.88], Top: [0.3, 0.5, 0.6], Jacket: [0.22, 0.22, 0.26], 'Dress and full body': [0.6, 0.25, 0.35],
  Legs: [0.2, 0.27, 0.42], Socks: [0.9, 0.9, 0.88], Shoes: [0.12, 0.12, 0.13], Hat: [0.5, 0.2, 0.2], Glasses: [0.08, 0.08, 0.1],
  Mask: [0.3, 0.3, 0.3], Neck: [0.85, 0.7, 0.3], 'Ears and piercings': [0.85, 0.7, 0.3], 'Wrists and hands': [0.75, 0.75, 0.78],
  Bags: [0.4, 0.28, 0.18], 'Belts and holsters': [0.15, 0.12, 0.1], Armour: [0.25, 0.27, 0.22],
};

// How far out each slot is worn: outer layers win where layers touch.
const SLOT_LAYER = { Underwear: 1, Socks: 1, 'Body hair': 1, Top: 2, Legs: 2, 'Dress and full body': 2, Shoes: 3, Jacket: 4, Neck: 5, Bags: 5, 'Belts and holsters': 5, Armour: 5, Hat: 5, Glasses: 6, Mask: 5, 'Wrists and hands': 5 };

// The skeleton people of each body are skinned on: the game's crowd bodies.
const BODY_SKELETON = { mr1: 'ambient_malebody', fr1: 'ambient_femalebody', mb1: 'ambient_malebb', fb1: 'ambient_femalebb', mo1: 'ambient_malebody', fo1: 'ambient_femalebody', mk1: 'ambient_kidbody', mc1: 'ambient_kidbody', fc1: 'ambient_kidbody', mi1: 'ambient_infantbody' };

const SKELETONS = new Map();
// A skeleton by its model's name (null when it has none).
function skeletonOf(name) {
  if (!SKELETONS.has(name)) SKELETONS.set(name, api('/api/skeleton', { name }).catch(() => null));
  return SKELETONS.get(name);
}

// The pose section of an inspector: pick one of the skeleton's main bones
// and turn it about its own axes; every model on the skeleton follows.
function posePanel(v) {
  if (!v?.sk) return [];
  v.poseEuler = v.poseEuler || new Map();
  const main = [...v.sk.index].filter(([n]) => /^SKEL_/.test(n) && !/_REV|Toe[0-9]|Finger[0-9]|Root$/.test(n));
  if (!main.length) return [];
  if (v.poseBone == null || !main.some(([, i]) => i === v.poseBone)) v.poseBone = (main.find(([n]) => /^SKEL_Head$|^SKEL_Neck/.test(n)) || main[0])[1];
  const pretty = (n) => n.replace(/^SKEL_/, '').replace(/_/g, ' ');
  const sel = h('select', { class: 'pick' }, ...main.map(([n, i]) => h('option', { value: i, text: pretty(n) + (v.poseEuler.has(i) ? ' •' : '') })));
  sel.value = v.poseBone;
  const outs = [];
  const sliders = ['Pitch (x)', 'Roll (y)', 'Turn (z)'].map((label, k) => {
    const out = h('span', { class: 'faint mono', style: 'width:42px;text-align:right' });
    const r = h('input', { type: 'range', min: '-150', max: '150', step: '1', style: 'flex:1', title: `${label} of the bone, in degrees (double-click: 0)` });
    const sync = () => { const e = v.poseEuler.get(v.poseBone) || [0, 0, 0]; r.value = e[k]; out.textContent = `${e[k]}°`; };
    const set = (x) => {
      const e = [...(v.poseEuler.get(v.poseBone) || [0, 0, 0])];
      e[k] = x;
      if (e.every((d) => !d)) { v.poseEuler.delete(v.poseBone); v.setPose(v.poseBone, null); } else { v.poseEuler.set(v.poseBone, e); v.setPose(v.poseBone, M4.euler(e)); }
      sync();
    };
    r.addEventListener('input', () => set(+r.value));
    r.addEventListener('dblclick', () => set(0));
    outs.push(sync);
    sync();
    return h('div', { class: 'insp-pick' }, h('span', { class: 'tk', text: label }), r, out);
  });
  sel.addEventListener('change', () => { v.poseBone = +sel.value; outs.forEach((f) => f()); });
  const reset = (all) => {
    for (const i of all ? [...v.poseEuler.keys()] : [v.poseBone]) { v.poseEuler.delete(i); v.setPose(i, null); }
    outs.forEach((f) => f());
  };
  return [
    h('div', { class: 'insp-h', text: `Pose · ${v.sk.names.length} bones` }),
    h('div', { class: 'empty', style: 'padding:2px 14px 6px', text: 'Turn a bone about its own axes: the body and everything worn follow it, as the game skins them (each vertex on up to four bones).' }),
    h('div', { class: 'insp-pick' }, h('span', { class: 'tk', text: 'Bone' }), sel),
    ...sliders,
    h('div', { class: 'insp-actions' }, h('button', { class: 'btn', onclick: () => reset(false) }, 'Reset bone'), h('button', { class: 'btn', onclick: () => reset(true) }, 'Reset pose')),
  ];
}

// Play the viewer's clip while `active()` (its tab is shown); stops
// otherwise, and the tab starts it again when shown.
function playLoop(v, active) {
  const P = v?.player;
  if (!P) return;
  cancelAnimationFrame(P.raf);
  if (!P.clip || !P.playing) return;
  let last = performance.now();
  const f = (now) => {
    if (!P.clip || !P.playing || v.gone || !active()) return;
    P.t = (P.t + Math.min(0.25, (now - last) / 1000) * P.speed) % Math.max(P.clip.length, 1e-3);
    last = now;
    v.setClipPose(P.clip, P.t * P.clip.fps);
    v.draw();
    P.onTime?.();
    P.raf = requestAnimationFrame(f);
  };
  P.raf = requestAnimationFrame(f);
}

// The clip that played last, for recording it.
const LAST_CLIP = { v: null, P: null };

// Record the clip that played last, as the page draws it: every sample
// (30 a second) drawn and sent to `/api/capture`, which keeps frames only
// when the server was started with VICE_LENS_CAPTURE. `turn`: radians the
// view turns over the recording. Returns the frames sent.
window.viceLensRecord = async (name, loops = 1, turn = 0) => {
  const { v, P } = LAST_CLIP;
  if (!v || !P?.clip) return 'no clip';
  P.playing = false;
  cancelAnimationFrame(P.raf);
  const per = P.clip.samples - 1, total = per * loops + 1;
  const yaw = v.yaw;
  for (let n = 0; n < total; n++) {
    v.yaw = yaw + (turn * n) / total;
    v.setClipPose(P.clip, n % per === 0 && n > 0 && n === total - 1 ? per : n % per);
    v.draw();
    const url = v.canvas.toDataURL('image/jpeg', 0.92);
    const bytes = Uint8Array.from(atob(url.split(',')[1]), (c) => c.charCodeAt(0));
    const r = await fetch(`/api/capture?name=${encodeURIComponent(name)}&n=${n}`, { method: 'POST', body: bytes });
    if (!r.ok) return `refused: ${await r.text()}`;
  }
  v.yaw = yaw;
  return total;
};

// A frame of `canvas` to `/api/capture` (kept only when the server was
// started with VICE_LENS_CAPTURE).
async function sendFrame(canvas, name, n) {
  const url = canvas.toDataURL('image/jpeg', 0.94);
  const bytes = Uint8Array.from(atob(url.split(',')[1]), (c) => c.charCodeAt(0));
  const r = await fetch(`/api/capture?name=${encodeURIComponent(name)}&n=${n}`, { method: 'POST', body: bytes });
  return r.ok ? null : `refused: ${await r.text()}`;
}

// Film the 3D viewer shown (the largest canvas on screen): `n` frames, the
// view turning by `turn` radians, tilting by `pitch` and its distance
// scaled by `zoom` over them; a clip playing on it plays on at 30 frames a
// second from where it is, and a vehicle's doors move on (`onFilm`).
// Returns the frames sent.
window.viceLensFilm = async (name, n, { turn = 0, pitch = 0, zoom = 1 } = {}) => {
  const area = (x) => x.canvas.clientWidth * x.canvas.clientHeight;
  const v = [...VIEWERS].filter((x) => x.canvas.offsetParent && !x.gone).sort((a, b) => area(b) - area(a))[0];
  if (!v) return 'no viewer shown';
  const P = v.player?.clip ? v.player : null;
  if (P) { P.playing = false; cancelAnimationFrame(P.raf); }
  const [yaw, pit, dist, t0] = [v.yaw, v.pitch, v.dist, P ? P.t : 0];
  for (let k = 0; k < n; k++) {
    const f = k / Math.max(n - 1, 1);
    v.yaw = yaw + turn * f;
    v.pitch = pit + pitch * f;
    v.dist = dist * Math.pow(zoom, f);
    if (P) v.setClipPose(P.clip, ((t0 + k / 30) % Math.max(P.clip.length, 1e-3)) * P.clip.fps);
    v.onFilm?.(1 / 30);
    v.draw();
    const no = await sendFrame(v.canvas, name, k);
    if (no) return no;
  }
  [v.yaw, v.pitch, v.dist] = [yaw, pit, dist];
  v.dirty();
  return n;
};

// Film the map from its view now to `to` ({cx, cy, mpp}: the middle and
// metres a pixel) over `n` frames, the zoom even on screen (metres a pixel
// eased geometrically); each frame waits for its tiles (up to `wait` ms).
// `style` switches the map's style first.
window.viceLensMapFilm = async (name, n, to, style, wait = 4000) => {
  const m = MAP;
  if (!m?.meta?.ok) return 'no map';
  if (style) await m.setStyle(style);
  const [cx, cy, mpp] = [m.cx, m.cy, m.mpp];
  const ease = (f) => f * f * (3 - 2 * f);
  for (let k = 0; k < n; k++) {
    const f = ease(k / Math.max(n - 1, 1));
    const z = Math.pow(to.mpp / mpp, f);
    m.mpp = mpp * z;
    // The middle moves so that it lands with the zoom (at the same pace).
    const g = mpp === to.mpp ? f : (1 - z) / (1 - to.mpp / mpp);
    m.cx = cx + (to.cx - cx) * g;
    m.cy = cy + (to.cy - cy) * g;
    const until = performance.now() + wait;
    for (;;) {
      m.draw();
      const pending = [...m.tiles.values()].filter((t) => !t.ok && !t.bad);
      if (!pending.length || performance.now() > until) break;
      await Promise.race([Promise.all(pending.map((t) => t.ready)), new Promise((r) => setTimeout(r, until - performance.now()))]);
    }
    const no = await sendFrame(m.canvas, name, k);
    if (no) return no;
  }
  return n;
};

// The animations section of an inspector: the game's clip dictionaries
// found by name, a dictionary's clips, and the clip playing on the
// viewer's skeleton. `query`: what to look for first (an animal's kind);
// `skip`: dictionaries left out (creatures', for people).
function animPanel(v, active, query, skip) {
  if (!v?.sk || !v.skHash) return [];
  const P = (v.player = v.player || { clip: null, t: 0, playing: false, raf: 0, speed: 1, q: query || '', dict: null, clips: null });
  const list = h('ul', { class: 'list' });
  const note = h('div', { class: 'empty', style: 'padding:2px 14px 6px' });
  const input = h('input', { type: 'text', class: 'pick', style: 'flex:1', placeholder: 'Search the game\'s animations (eat, look, sleep…)', value: P.q });
  const playBtn = h('button', { class: 'btn' }, P.playing ? 'Pause' : 'Play');
  const time = h('input', { type: 'range', min: '0', max: '1000', step: '1', value: '0', style: 'flex:1', title: 'Time in the clip' });
  const clock = h('span', { class: 'faint mono', style: 'min-width:86px;text-align:right' });
  const now = h('div', { class: 'faint mono', style: 'padding:2px 14px;word-break:break-all' });
  const showTime = () => {
    if (!P.clip) { clock.textContent = ''; now.textContent = 'No clip playing.'; return; }
    time.value = Math.round((P.t / Math.max(P.clip.length, 1e-3)) * 1000);
    clock.textContent = `${P.t.toFixed(2)} / ${P.clip.length.toFixed(2)} s`;
    now.textContent = `${P.clip.name}${P.clip.face ? ` with ${P.clip.face}` : ''} · ${P.clip.bones.length} bones${P.clip.whole ? '' : ' · partly: a block of its animation is missing'}`;
  };
  P.onTime = showTime;
  playBtn.addEventListener('click', () => { if (!P.clip) return; P.playing = !P.playing; playBtn.textContent = P.playing ? 'Pause' : 'Play'; playLoop(v, active); });
  time.addEventListener('input', () => {
    if (!P.clip) return;
    P.t = (+time.value / 1000) * P.clip.length;
    v.setClipPose(P.clip, P.t * P.clip.fps);
    v.dirty();
    showTime();
  });
  const stop = h('button', { class: 'btn', title: 'Stop and stand at rest' }, 'Stop');
  stop.addEventListener('click', () => { P.playing = false; P.clip = null; cancelAnimationFrame(P.raf); v.setClipPose(null); v.dirty(); playBtn.textContent = 'Play'; showTime(); });
  // Clip `c` (index `i`, or `c` in a motion's list) of dictionary `d`
  // (offset `o`, archive `a`).
  const play = async (d, c) => {
    now.textContent = 'Reading the clip…';
    try {
      const r = await api('/api/anim/clip', { o: d.o, a: d.a, c: c.c ?? c.i, skel: v.skHash });
      // The face plays in a clip of its own beside the body's
      // (`idle_a_face.clip`): its bones join the body's, resampled to them.
      const face = d === P.dict && P.clips?.find((x) => x.name === c.name.replace(/\.clip$/, '_face.clip'));
      if (face) {
        const f = await api('/api/anim/clip', { o: d.o, a: d.a, c: face.i, skel: v.skHash }).catch(() => null);
        const have = new Set(r.bones.map((b) => b.i));
        for (const b of f?.bones || []) {
          if (have.has(b.i)) continue;
          const re = (arr, k) => arr && Array.from({ length: r.samples }, (_, s) => arr.slice(Math.min(Math.round((s * (f.samples - 1)) / Math.max(r.samples - 1, 1)), f.samples - 1) * k).slice(0, k)).flat();
          r.bones.push({ i: b.i, r: re(b.r, 4), t: re(b.t, 3) });
        }
        if (f?.bones.length) r.face = face.name;
      }
      for (const b of r.bones) { if (b.r) b.r = new Float32Array(b.r); if (b.t) b.t = new Float32Array(b.t); }
      P.clip = r;
      P.t = 0;
      P.playing = true;
      LAST_CLIP.v = v;
      LAST_CLIP.P = P;
      playBtn.textContent = 'Pause';
      playLoop(v, active);
      showTime();
    } catch (e) { now.textContent = e.message; }
  };
  const dicts = async () => {
    P.q = input.value;
    P.dict = null;
    try {
      const r = await api('/api/anims', { q: P.q, limit: 200 });
      const items = r.items.filter((x) => !skip || !skip.test(x.name)).slice(0, 80);
      note.textContent = `${fmtNum(r.total)} dictionaries match, of ${fmtNum(r.listed)}: ${fmtNum(r.with_clips)} hold clips, the others motions (clips of other dictionaries the game plays together). Pick one.`;
      list.replaceChildren(...items.map((x) => h('li', { class: 'row', title: x.name + (x.a ? ` (${x.a})` : ''), onclick: () => clips(x) },
        icon('play'), h('span', { class: 'name' }, x.name.replace(/@/g, ' › '), h('span', { class: 'sub', text: (x.clips ? `${x.clips} clips` : 'motions') + (x.a ? ` · ${x.a.split('/').pop()}` : '') })))));
    } catch (e) { note.textContent = e.message; list.replaceChildren(); }
  };
  // A motion's clip: where it comes from, its length and its parameters
  // (first → last value over the clip).
  const source = (m, s) => {
    if (!s.clip) {
      return h('li', { class: 'row', style: 'opacity:.55;padding-left:22px', title: `clip ${s.key} of dictionary ${s.dkey}` },
        h('span', { class: 'name' }, `clip ${s.key}`, h('span', { class: 'sub', text: s.dict ? `not in ${s.dict}` : 'in a dictionary not listed' })));
    }
    const params = s.params.map((p) => `${p[0].toFixed(2)}→${p[1].toFixed(2)}`).join(', ');
    return h('li', { class: 'row', style: 'padding-left:22px', title: `${s.clip.name} in ${s.dict}`, onclick: () => play(s, s.clip) },
      icon('play'), h('span', { class: 'name' }, s.clip.name, h('span', { class: 'sub', text: `${s.clip.length.toFixed(2)} s${s.clip.whole ? '' : ' · partly'} · ${s.dict.split('@').slice(-2).join('@')}${params ? ` · ${params}` : ''}` })));
  };
  const clips = async (d) => {
    P.dict = d;
    try {
      const r = await api('/api/anim/clips', { o: d.o, a: d.a });
      P.clips = r.items;
      note.textContent = d.name + (r.motions.length ? ` · ${r.motions.length} motions: each plays clips of other dictionaries together, picked and kept in step by parameters such as how far is left to go (the game's own blending is not done here: play each clip)` : '');
      list.replaceChildren(h('li', { class: 'row', onclick: () => dicts() }, icon('prev'), h('span', { class: 'name', text: 'Back to the dictionaries' })),
        ...r.items.map((c) => h('li', { class: 'row', title: c.name, onclick: () => play(d, c) },
          icon('play'), h('span', { class: 'name' }, c.name, h('span', { class: 'sub', text: `${c.length.toFixed(2)} s${c.whole ? '' : ' · partly'}` })))),
        ...r.motions.flatMap((m) => [
          h('li', { class: 'row', style: 'cursor:default' }, h('span', { class: 'name' }, m.name, h('span', { class: 'sub', text: `motion · ${m.sources.length} clips` }))),
          ...m.sources.map((s) => source(m, s)),
        ]));
    } catch (e) { note.textContent = e.message; }
  };
  input.addEventListener('input', debounce(dicts, 250));
  if (P.dict) clips(P.dict); else dicts();
  showTime();
  return [
    h('div', { class: 'insp-h', text: 'Animations' }),
    h('div', { class: 'insp-pick' }, input),
    note,
    h('div', { style: 'max-height:260px;overflow:auto' }, list),
    h('div', { class: 'insp-pick' }, playBtn, stop, time, clock),
    now,
  ];
}

// The wardrobe: a person put together from separate models (all in one
// pose), one item per slot; `W.worn` maps slot → item.
const W = { tab: null, viewer: null, worn: new Map(), body: 'mr1', who: null, models: new Map(), seq: 0 };

function wardrobeTab() {
  if (W.tab && S.tabs.includes(W.tab)) { showTab('wardrobe'); return W.tab; }
  const canvas = h('canvas');
  const msg = h('div', { class: 'center-msg', text: 'Pick a character or a piece of clothing on the left.' });
  const skin = h('button', { class: 'btn' + (store.get('people.skin', true) ? ' on' : ''), title: 'A shop mannequin under the clothes, with bare hands and feet (the game has no plain bare body: its bodies are worn by its creatures)' }, 'Mannequin');
  skin.addEventListener('click', () => { store.set('people.skin', !store.get('people.skin', true)); skin.classList.toggle('on', store.get('people.skin', true)); redress(); });
  const stage = h('div', { class: 'stage' }, canvas, msg, h('div', { class: 'help', text: 'Drag to orbit · right-drag or Shift to pan · wheel to zoom' }));
  const title = h('span', { class: 'title', text: 'Wardrobe' });
  const view = h('div', { class: 'model' }, h('div', { class: 'toolbar' }, title, h('span', { class: 'grow' }), skin,
    h('button', { class: 'btn', title: 'Take everything off', onclick: () => { W.worn.clear(); W.who = null; redress(); } }, icon('x'), 'Undress'),
    h('button', { class: 'btn icon-only', title: 'Frame (F)', onclick: () => W.viewer?.frame() }, icon('fit'))), stage);
  W.tab = openTab({ id: 'wardrobe', title: 'Wardrobe', icon: 'person', closable: true, view,
    show: () => { W.viewer?.dirty(); wardrobeInfo(); playLoop(W.viewer, () => S.active === 'wardrobe'); },
    close: () => { W.viewer?.dispose(); W.viewer = null; W.tab = null; } });
  W.title = title;
  W.msg = msg;
  W.canvas = canvas;
  new ResizeObserver(() => W.viewer?.dirty()).observe(view);
  return W.tab;
}

// A model's meshes on skeleton `skel` (a hash), or as they are without one.
const wardrobeModel = (hash, skel) => {
  const key = `${hash}:${skel || ''}`;
  if (!W.models.has(key)) W.models.set(key, Promise.all([api('/api/model', { h: hash }), bin('/api/model/mesh', skel ? { h: hash, skel } : { h: hash })]));
  return W.models.get(key);
};

// Draw what is worn, and the bare body under it if asked.
async function redress() {
  const mine = ++W.seq;
  const items = [...W.worn.values()];
  // Under a character's own clothes nothing is added; under loose clothes,
  // a shop mannequin (the figure kind) and bare hands and feet.
  if (store.get('people.skin', true) && !W.who) {
    const Wd = WARDROBE.data;
    for (const kind of ['mannequinupperbodyfigure', 'mannequinlowerbodyfigure', 'mannequinheadfigure', 'hand', 'feet']) {
      if ([...W.worn.values()].some((i) => i.kind === kind)) continue;
      const base = Wd?.items.find((i) => i.body === W.body && i.kind === kind && (kind.startsWith('mannequin') || i.rest.includes('nude')));
      if (base) items.push(base);
    }
  }
  // Everything on the body's skeleton, to pose and animate it.
  const sk = await skeletonOf(BODY_SKELETON[W.body] || 'ambient_malebody');
  const loaded = await Promise.all(items.map((i) => wardrobeModel(i.h, sk?.h).then((m) => [i, ...m]).catch(() => null)));
  if (mine !== W.seq || !W.tab) return;
  if (!W.viewer) { W.viewer = new Viewer(W.canvas); W.viewer.pitch = 0.15; W.viewer.yaw = -1.57; }
  const v = W.viewer;
  if (sk && v.skHash !== sk.h) { v.setSkeleton(sk); v.skHash = sk.h; v.poseEuler = new Map(); }
  v.clear();
  for (const x of loaded.filter(Boolean)) {
    const [i, info, buf] = x;
    v.upload(buf, i.h, info, { keepFrame: true, paint: true, tint: SLOT_TINT[i.slot] || [0.7, 0.68, 0.62], group: 'slot:' + i.slot, layer: SLOT_LAYER[i.slot] || 0, dropPlaceholders: true });
  }
  if (loaded.some(Boolean)) { W.msg.remove(); v.frame(); }
  W.title.textContent = W.who ? personName(W.who.name) : `Wardrobe · ${BODY_NAMES[W.body] || W.body}`;
  wardrobeInfo();
}

function wear(item) {
  wardrobeTab();
  if (item.body !== W.body) { W.worn.clear(); W.who = null; W.body = item.body; }
  if (W.worn.get(item.slot)?.h === item.h) W.worn.delete(item.slot); else W.worn.set(item.slot, item);
  redress();
}

// A character: the first of its items in every slot; the rest to choose.
function dressAs(c) {
  wardrobeTab();
  W.who = c;
  W.body = c.body;
  W.worn.clear();
  for (const i of c.items) if (!W.worn.has(i.slot)) W.worn.set(i.slot, i);
  redress();
}

function wardrobeInfo() {
  if (S.active !== 'wardrobe') return;
  const c = W.who;
  const slots = WARDROBE.data?.slots || [];
  const worn = [...W.worn.entries()].sort((a, b) => slots.indexOf(a[0]) - slots.indexOf(b[0]));
  const choices = c ? slots.map((s) => [s, c.items.filter((i) => i.slot === s)]).filter(([, l]) => l.length > 1) : [];
  inspect(
    h('div', { class: 'insp-title' }, h('div', { class: 'n', text: c ? personName(c.name) : 'Wardrobe' }), h('div', { class: 's' }, h('span', { class: 'badge', text: BODY_NAMES[W.body] || W.body }), c ? h('span', { class: 'badge', text: c.name }) : null)),
    h('div', { class: 'empty', style: 'padding:4px 14px 6px', text: 'Clothes are coloured in the game by masks and palettes it applies at runtime; here each slot has a plain colour. Click an item on the left to put it on or take it off.' }),
    h('div', { class: 'insp-h', text: `Worn · ${worn.length}` }),
    h('ul', { class: 'list' }, ...worn.map(([s, i]) => h('li', { class: 'row', title: i.name },
      h('span', { class: 'swatch', style: `background:rgb(${(SLOT_TINT[s] || [0.7, 0.68, 0.62]).map((x) => Math.round(x * 255)).join(',')})` }),
      h('span', { class: 'name' }, s, h('span', { class: 'sub', text: i.name })),
      h('button', { class: 'icon-btn', title: 'Open as a model', onclick: () => openModel(i.h, i.name) }, icon('cube')),
      h('button', { class: 'icon-btn', title: 'Take it off', onclick: () => { W.worn.delete(s); redress(); } }, icon('x'))))),
    choices.length ? h('div', { class: 'insp-h', text: 'This character\'s other items' }) : null,
    ...choices.map(([s, list]) => {
      const sel = h('select', { class: 'pick' }, ...list.map((i) => h('option', { value: i.h, text: i.name.slice(c.name.length + 1).replace(/_full$/, '') })));
      sel.value = W.worn.get(s)?.h || list[0].h;
      sel.addEventListener('change', () => { W.worn.set(s, list.find((i) => i.h === sel.value)); redress(); });
      return h('div', { class: 'insp-pick' }, h('span', { class: 'tk', text: s }), sel);
    }),
    ...animPanel(W.viewer, () => S.active === 'wardrobe', '', /creatures/),
    ...posePanel(W.viewer));
}

// ---------------------------------------------------------------- animals

const CREATURES = {
  data: null,
  async load() { if (!this.data) this.data = await api('/api/creatures'); return this.data; },
};
// Fur lengths are a part's version, not a look of their own.
const FUR_LENGTHS = new Set(['short', 'medium', 'long']);
// Parts that are put on (worn, or a state): left off until a look asks.
const CREATURE_EXTRAS = new Set(['costume', 'harness', 'generic harness', 'collar', 'hat', 'badge', 'ribbon', 'rope', 'cape', 'flower', 'lace', 'loop', 'bandana', 'jewellery', 'dress', 'leash']);

// `crm_dog_chihuahua_f_00` → "Dog chihuahua, female 00".
function animalName(n) {
  const m = n.match(/^cr[a-z]_(.+)_([mfu])_(\d\d)$/);
  if (!m) return n;
  const s = m[1].replace(/_/g, ' ');
  return `${s[0].toUpperCase()}${s.slice(1)}, ${{ m: 'male', f: 'female', u: '' }[m[2]]} ${m[3]}`.replace(/,\s+(\d)/, ', $1');
}

// An animal put together from its parts, one per slot; a look takes the
// parts made for it where it has them.
function openCreature(c, replace) {
  const id = 'animal:' + c.name;
  if (S.tabs.find((t) => t.id === id)) { showTab(id); return; }
  const canvas = h('canvas');
  const msg = h('div', { class: 'center-msg', text: 'Bringing the animal in…' });
  const stage = h('div', { class: 'stage' }, canvas, msg, h('div', { class: 'help', text: 'Drag to orbit · right-drag or Shift to pan · wheel to zoom' }));
  const slots = [...new Set(c.parts.map((p) => p.slot))];
  const looks = [...new Set(c.parts.map((p) => p.look).filter((l) => l && !FUR_LENGTHS.has(l)))].sort();
  const st = { viewer: null, worn: new Map(), look: '', seq: 0, models: new Map() };
  // The parts a look wears: per slot, the one made for the look, else the
  // plain one (medium fur before short and long); extras only for a look.
  const dress = (look) => {
    st.look = look;
    st.worn.clear();
    for (const s of slots) {
      const ps = c.parts.filter((p) => p.slot === s);
      const mine = look && ps.find((p) => p.look === look);
      if (mine) { st.worn.set(s, mine); continue; }
      if (CREATURE_EXTRAS.has(s)) continue;
      const plain = ps.find((p) => p.look === '') || ps.find((p) => p.look === 'medium') || ps.find((p) => FUR_LENGTHS.has(p.look)) || (look ? null : ps.find((p) => !/corpse|blood|dirt/.test(p.look)));
      if (plain) st.worn.set(s, plain);
    }
    draw();
  };
  // Every part on the animal's skeleton (in the model named as the animal).
  const model = (hash, skel) => {
    if (!st.models.has(hash)) st.models.set(hash, Promise.all([api('/api/model', { h: hash }), bin('/api/model/mesh', skel ? { h: hash, skel } : { h: hash })]));
    return st.models.get(hash);
  };
  const draw = async () => {
    const mine = ++st.seq;
    const sk = await skeletonOf(c.name);
    const loaded = await Promise.all([...st.worn.values()].map((p) => model(p.h, sk?.h).then((m) => [p, ...m]).catch(() => null)));
    if (mine !== st.seq) return;
    if (!st.viewer) { st.viewer = new Viewer(canvas); st.viewer.pitch = 0.3; st.viewer.yaw = -0.9; if (sk) { st.viewer.setSkeleton(sk); st.viewer.skHash = sk.h; } }
    const v = st.viewer;
    const first = !v.meshes.length;
    v.clear();
    for (const x of loaded.filter(Boolean)) {
      const [p, inf, buf] = x;
      v.upload(buf, p.h, inf, { keepFrame: true, group: 'slot:' + p.slot, dropPlaceholders: true });
    }
    msg.remove();
    if (first) v.frame(); else v.dirty();
    if (S.active === id) creatureInfo();
  };
  const creatureInfo = () => {
    inspect(
      h('div', { class: 'insp-title' }, h('div', { class: 'n', text: animalName(c.name) }), h('div', { class: 's' }, h('span', { class: 'badge', text: c.class }), h('span', { class: 'badge', text: c.name }))),
      looks.length ? h('div', { class: 'insp-h', text: `Looks · ${looks.length}` }) : null,
      looks.length ? h('div', { class: 'looks' }, h('button', { class: 'btn' + (st.look ? '' : ' on'), onclick: () => dress('') }, 'Plain'),
        ...looks.map((l) => h('button', { class: 'btn' + (st.look === l ? ' on' : ''), onclick: () => dress(l) }, l))) : null,
      h('div', { class: 'insp-h', text: `Parts · ${slots.length}` }),
      h('div', { class: 'empty', style: 'padding:2px 14px 6px', text: 'Fur and feathers drawn as strands are a kind of geometry not read yet; they show as nothing.' }),
      ...slots.map((s) => {
        const ps = c.parts.filter((p) => p.slot === s);
        const sel = h('select', { class: 'pick' }, h('option', { value: '', text: 'None' }), ...ps.map((p) => h('option', { value: p.h, text: p.name.slice(c.name.length + 1) })));
        sel.value = st.worn.get(s)?.h || '';
        sel.addEventListener('change', () => { const p = ps.find((x) => x.h === sel.value); if (p) st.worn.set(s, p); else st.worn.delete(s); draw(); });
        sel.addEventListener('contextmenu', (e) => { const p = ps.find((x) => x.h === sel.value); if (p) contextMenu(e, [{ label: `Open ${p.name}`, icon: 'cube', run: () => openModel(p.h, p.name) }]); });
        return h('div', { class: 'insp-pick' }, h('span', { class: 'tk', text: s }), sel);
      }),
      ...animPanel(st.viewer, () => S.active === id, c.name.split('_')[1] || ''),
      ...posePanel(st.viewer));
  };
  const view = h('div', { class: 'model' }, h('div', { class: 'toolbar' }, h('span', { class: 'title', text: animalName(c.name) }), h('span', { class: 'grow' }),
    h('button', { class: 'btn icon-only', title: 'Frame (F)', onclick: () => st.viewer?.frame() }, icon('fit'))), stage);
  openTab({ id, title: animalName(c.name), icon: 'paw', closable: true, view, show: () => { st.viewer?.dirty(); creatureInfo(); playLoop(st.viewer, () => S.active === id); }, close: () => st.viewer?.dispose() }, replace);
  new ResizeObserver(() => st.viewer?.dirty()).observe(view);
  dress('');
}

// ---------------------------------------------------------------- archives

function openArchive(name) {
  const id = 'archive:' + name;
  if (S.tabs.find((t) => t.id === id)) { showTab(id); return; }
  const body = h('tbody');
  const count = h('span', { class: 'faint mono' });
  const wrap = h('div', { class: 'table-wrap' });
  const kindSel = h('select', { class: 'pick', title: 'Only files of this kind' }, h('option', { value: '', text: 'Any kind' }));
  let p = { q: '' }, kind = '', from = 0, total = 0, loading = false, again = false;
  const filter = searchBox('archive', 'Filter by name, kind or offset', (np) => { p = np; load(true); }, { value: '' });
  kindSel.addEventListener('change', () => { kind = kindSel.value; kindSel.classList.toggle('set', !!kind); load(true); });
  const view = h('div', { class: 'page' },
    h('div', { class: 'toolbar' }, filter.el, kindSel, count, h('span', { class: 'grow' })),
    wrap);
  const table = h('table', { class: 'grid' }, h('thead', {}, h('tr', {}, h('th', { text: 'Offset' }), h('th', { text: 'Size' }), h('th', { text: 'Kind' }), h('th', { text: 'Name' }))), body);
  const more = h('div', { class: 'more' });
  const load = async (reset) => {
    if (loading) { again = again || reset; return; }
    loading = true;
    if (reset) { from = 0; body.replaceChildren(); wrap.scrollTop = 0; }
    try {
      const r = await api('/api/archive/rows', { a: name, ...p, kind: kind || null, from, limit: 300 }).catch((e) => { filter.error(e.message); return { indexed: true, total: 0, items: [] }; });
      if (r.kinds && kindSel.options.length === 1) kindSel.append(...r.kinds.map(([k, n]) => h('option', { value: k, text: `${k} (${fmtNum(n)})` })));
      if (!r.indexed) {
        wrap.replaceChildren(h('div', { class: 'center-msg' }, h('div', {},
          h('div', { style: 'font-size:15px;color:var(--text);margin-bottom:6px', text: `${name} is not listed yet` }),
          'Listing reads the whole archive once (minutes for the largest) and keeps the list in the cache.', h('br'), h('br'),
          h('button', { class: 'btn primary', onclick: async (e) => { e.target.disabled = true; await api('/api/archive/index', { a: name }, { method: 'POST' }); toast(`Listing ${name}…`); } }, 'List files'))));
        const done = (ev) => { if (ev.detail.what.endsWith(name)) { window.removeEventListener('job-done', done); wrap.replaceChildren(table, more); load(true); } };
        window.addEventListener('job-done', done);
        return;
      }
      total = r.total;
      for (const [off, size, kind, nm] of r.items) {
        const tr = h('tr', { onclick: () => { for (const x of body.children) x.classList.remove('on'); tr.classList.add('on'); fileInfo(name, off, size, kind, nm); }, ondblclick: () => openBytes(name, off, nm || off) },
          h('td', { text: off }), h('td', { class: 'num', text: fmtBytes(size) }), h('td', { class: 'dim', text: kind }), h('td', {}, ...highlight(nm, p)));
        tr._f = { off, size, kind, nm };
        body.append(tr);
      }
      from += r.items.length;
      count.textContent = `${fmtNum(total)} files`;
      more.replaceChildren(from < total ? h('button', { class: 'btn', onclick: () => load(false) }, `Show more (${fmtNum(total - from)} left)`) : h('span', { class: 'faint', text: total ? 'End of list' : 'Nothing matches' }));
    } finally {
      loading = false;
      if (again) { again = false; load(true); }
    }
  };
  wrap.append(table, more);
  navigable(wrap, {
    rows: () => [...body.children],
    select: (r) => fileInfo(name, r._f.off, r._f.size, r._f.kind, r._f.nm),
    open: (r) => openBytes(name, r._f.off, r._f.nm || r._f.off),
    menu: (r) => [
      { label: 'Open bytes', icon: 'hex', key: 'Enter', run: () => openBytes(name, r._f.off, r._f.nm || r._f.off) },
      { label: 'Save file…', icon: 'save', run: () => saveAs(`/api/file?a=${encodeURIComponent(name)}&o=${r._f.off}`, fileName(r._f.nm || `${name}-${r._f.off}`)) },
      { label: 'Open model', icon: 'cube', disabled: !/\.#(dr|ft)$/.test(r._f.nm), run: () => openModelByName(r._f.nm) },
      '-',
      { label: 'Copy offset', icon: 'copy', run: () => copy(r._f.off) },
      { label: 'Copy name', icon: 'copy', disabled: !r._f.nm, run: () => copy(r._f.nm) },
    ],
  });
  intoList(filter.input, wrap);
  wrap.addEventListener('scroll', () => { if (wrap.scrollTop + wrap.clientHeight > wrap.scrollHeight - 300 && from < total) load(false); });
  openTab({ id, title: name, icon: 'files', closable: true, view });
  load(true);
}

function fileInfo(archive, off, size, kind, name) {
  setStatus('sel', `${archive} ${off}`);
  inspect(
    h('div', { class: 'insp-title' }, h('div', { class: 'n', text: name || off }), h('div', { class: 's', text: kind })),
    h('div', { class: 'insp-actions' },
      h('button', { class: 'btn primary', onclick: () => openBytes(archive, off, name || off) }, icon('hex'), 'Open bytes'),
      name && /\.#(dr|ft)$/.test(name) ? h('button', { class: 'btn', onclick: () => openModelByName(name) }, icon('cube'), 'Open model') : null,
      h('button', { class: 'btn', title: 'Save the file as the archive holds it (inflated)', onclick: () => saveAs(`/api/file?a=${encodeURIComponent(archive)}&o=${off}`, fileName(name || `${archive}-${off}`)) }, icon('save'), 'Save file'),
      h('button', { class: 'btn', onclick: () => copy(off) }, icon('copy'), 'Copy offset')),
    kv([['Archive', archive], ['Offset', off], ['Stored size', fmtBytes(size)], ['Kind', kind], ['Name', name || 'unknown']]));
}

async function openModelByName(name) {
  const stem = name.split('.#')[0];
  const r = await api('/api/models', { q: stem, limit: 5 });
  const m = r.items.find((x) => x.name === stem) || r.items[0];
  if (m) openModel(m.h, m.name); else toast('Not in the model index');
}

// ---------------------------------------------------------------- bytes

function openBytes(archive, off, title) {
  const id = `bytes:${archive}:${off}`;
  if (S.tabs.find((t) => t.id === id)) { showTab(id); return; }
  const rows = h('div', { class: 'rows' });
  const scroller = h('div', { class: 'hex' }, rows);
  const seg = h('div', { class: 'seg' });
  const go = h('input', { class: 'text plain', placeholder: 'Go to offset (hex)', style: 'width:170px', spellcheck: 'false' });
  const sizeEl = h('span', { class: 'faint mono' });
  let part = 'file', len = 0;
  const saveBtn = h('button', { class: 'btn', title: 'Save the part shown (the file, or a resource\'s virtual or physical segment)', onclick: () => saveAs(`/api/file?a=${encodeURIComponent(archive)}&o=${off}&part=${part}`, `${fileName(title)}${part === 'file' ? '' : `.${part}`}.bin`) }, icon('save'), 'Save');
  const view = h('div', { class: 'page' }, h('div', { class: 'toolbar' }, seg, go, sizeEl, h('span', { class: 'grow' }), h('span', { class: 'faint mono', text: `${archive} ${off}` }), saveBtn), scroller);
  const chunks = new Map();
  const CH = 65536, RH = 20;
  const chunk = (k) => {
    const key = part + k;
    if (!chunks.has(key)) {
      chunks.set(key, null);
      bin('/api/bytes', { a: archive, o: off, part, at: k * CH, len: CH }).then((b) => { chunks.set(key, new Uint8Array(b)); paint(); });
    }
    return chunks.get(key);
  };
  const hex2 = (b) => b.toString(16).padStart(2, '0');
  const paint = () => {
    rows.style.height = Math.ceil(len / 16) * RH + 'px';
    const first = Math.floor(scroller.scrollTop / RH), n = Math.ceil(scroller.clientHeight / RH) + 1;
    const out = [];
    for (let r = first; r < first + n && r * 16 < len; r++) {
      const at = r * 16, data = chunk(Math.floor(at / CH));
      let hexs = '', asc = '';
      const zero = [];
      for (let i = 0; i < 16; i++) {
        if (at + i >= len) { hexs += '   '; continue; }
        const b = data ? data[(at % CH) + i] : null;
        if (b == null) { hexs += '·· '; asc += ' '; continue; }
        hexs += hex2(b) + (i === 7 ? '  ' : ' ');
        asc += b >= 32 && b < 127 ? String.fromCharCode(b) : '.';
        if (b === 0) zero.push(i);
      }
      out.push(h('div', { class: 'r', style: `top:${r * RH}px` }, h('span', { class: 'o', text: at.toString(16).padStart(8, '0') }), h('span', { class: zero.length === 16 ? 'z' : '', text: hexs }), h('span', { class: 'a', text: asc })));
    }
    rows.replaceChildren(...out);
  };
  scroller.addEventListener('scroll', () => requestAnimationFrame(paint));
  new ResizeObserver(paint).observe(scroller);
  go.addEventListener('keydown', (e) => {
    if (e.key !== 'Enter') return;
    const v = parseInt(go.value.replace(/^0x/, ''), 16);
    if (Number.isFinite(v)) scroller.scrollTop = Math.floor(v / 16) * RH;
  });
  openTab({ id, title: `${title}`, hint: `${archive} ${off}`, icon: 'hex', closable: true, view });
  api('/api/blob', { a: archive, o: off }).then((b) => {
    const parts = [['file', 'File', b.file], ['virt', 'Virtual', b.virt], ['phys', 'Physical', b.phys]].filter((p) => p[2] > 0);
    const pick = (p) => { part = p[0]; len = p[2]; sizeEl.textContent = `${fmtNum(len)} bytes`; for (const x of seg.children) x.classList.toggle('on', x.dataset.p === p[0]); scroller.scrollTop = 0; paint(); };
    seg.replaceChildren(...parts.map((p) => h('button', { 'data-p': p[0], title: `${fmtBytes(p[2])}`, onclick: () => pick(p) }, p[1])));
    pick(parts.find((p) => p[0] === 'virt') || parts[0]);
    inspect(h('div', { class: 'insp-title' }, h('div', { class: 'n', text: title }), h('div', { class: 's', text: b.kind })),
      kv([['Archive', archive], ['Offset', off], ['File', fmtBytes(b.file)], ['Virtual', b.virt ? fmtBytes(b.virt) : '—'], ['Physical', b.phys ? fmtBytes(b.phys) : '—']]));
  }).catch((e) => rows.replaceChildren(h('div', { class: 'empty', text: e.message })));
}

// ---------------------------------------------------------------- textures

const texUrl = (a, o, i, max) => `/api/texture?a=${encodeURIComponent(a)}&o=${o}&i=${i}${max ? `&max=${max}` : ''}`;

// Save a file the server makes: through a native dialog in the app,
// as a download in a browser.
function saveAs(path, name) {
  if (window.ipc) window.ipc.postMessage(`save:${name}|${path}`);
  else { const a = h('a', { href: path, download: name }); document.body.append(a); a.click(); a.remove(); }
}

function openTexDict(d, focus) {
  const id = `texdict:${d.a}:${d.o}`;
  const old = S.tabs.find((t) => t.id === id);
  if (old) { showTab(id); if (focus != null) old.focus?.(focus); return; }
  const grid = h('div', { class: 'gallery' });
  const count = h('span', { class: 'faint mono' });
  const view = h('div', { class: 'page' }, h('div', { class: 'toolbar' }, h('span', { class: 'title', text: d.first || d.o }), count, h('span', { class: 'grow' }), h('span', { class: 'faint mono', text: `${d.a} ${d.o}` })), h('div', { class: 'gallery-wrap' }, grid));
  const wrapEl = view.querySelector('.gallery-wrap');
  const lazy = new IntersectionObserver((es) => { for (const e of es) if (e.isIntersecting) { const t = e.target; t._img.src = t._src; lazy.unobserve(t); } }, { root: wrapEl, rootMargin: '200px' });
  const pick = (el) => texInfo(d, el._t);
  const tab = openTab({ id, title: d.first || d.o, icon: 'image', closable: true, view, close: () => lazy.disconnect() });
  navigable(grid, {
    rows: () => [...grid.children], grid: true, select: pick,
    open: (el) => openImage(texUrl(d.a, d.o, el._t.i), el._t.name, `${el._t.w}×${el._t.h} ${el._t.format}`),
    menu: (el) => [
      { label: 'Open', icon: 'image', key: 'Enter', run: () => openImage(texUrl(d.a, d.o, el._t.i), el._t.name, `${el._t.w}×${el._t.h} ${el._t.format}`) },
      { label: 'Save PNG…', icon: 'save', run: () => saveAs(texUrl(d.a, d.o, el._t.i), el._t.name + '.png') },
      '-',
      { label: 'Copy name', icon: 'copy', run: () => copy(el._t.name) },
    ],
  });
  tab.focus = (i) => { const el = grid.children[i]; if (!el) return; [...grid.children].forEach((x) => x.classList.remove('on')); el.classList.add('on'); el.scrollIntoView({ block: 'center' }); pick(el); };
  api('/api/texdict', { a: d.a, o: d.o }).then((r) => {
    count.textContent = `${r.items.length} textures`;
    grid.replaceChildren(...r.items.map((t) => {
      const img = h('img', { alt: '', draggable: 'false' });
      img.addEventListener('error', () => { img.remove(); el.classList.add('none'); el.prepend(h('em', { text: `${t.format}: not decoded` })); });
      const el = h('div', { class: 'thumb', title: `${t.name}\n${t.w}×${t.h} ${t.format}`, onclick: () => { [...grid.children].forEach((x) => x.classList.remove('on')); el.classList.add('on'); pick(el); }, ondblclick: () => openImage(texUrl(d.a, d.o, t.i), t.name, `${t.w}×${t.h} ${t.format}`) },
        img, h('span', { text: t.name }));
      el._t = t;
      el._img = img;
      el._src = texUrl(d.a, d.o, t.i, 256);
      lazy.observe(el);
      return el;
    }));
    if (focus != null) tab.focus(focus);
    grid.focus();
  }).catch((e) => grid.replaceChildren(h('div', { class: 'empty', text: e.message })));
}

function texInfo(d, t) {
  const src = texUrl(d.a, d.o, t.i);
  setStatus('sel', `${t.name}  ${t.w}×${t.h}`);
  inspect(
    h('div', { class: 'insp-title' }, h('div', { class: 'n', text: t.name }), h('div', { class: 's' }, h('span', { class: 'badge', text: t.format }), `${t.w}×${t.h}`)),
    h('div', { class: 'preview checker' }, h('img', { src: texUrl(d.a, d.o, t.i, 512), alt: '' })),
    h('div', { class: 'insp-actions' },
      h('button', { class: 'btn primary', onclick: () => openImage(src, t.name, `${t.w}×${t.h} ${t.format}`) }, icon('image'), 'Open'),
      h('button', { class: 'btn', onclick: () => saveAs(src, t.name + '.png') }, icon('save'), 'Save PNG'),
      h('button', { class: 'btn', onclick: () => copy(t.name) }, icon('copy'), 'Copy name')),
    kv([['Size', `${t.w} × ${t.h}`], ['Format', t.format], ['Mip levels', t.mips], ['Dictionary', `${d.a} ${d.o}`], ['Index', t.i]]));
}

// An image with zoom (wheel), pan (drag), fit and 1:1.
function openImage(src, title, meta) {
  const id = 'image:' + src;
  if (S.tabs.find((t) => t.id === id)) { showTab(id); return; }
  const img = h('img', { src, alt: title, draggable: 'false' });
  const stage = h('div', { class: 'imgstage checker' }, img);
  const zoomEl = h('span', { class: 'faint mono' });
  const { fit, actual } = panZoom(stage, img, (z) => (zoomEl.textContent = `${Math.round(z * 100)}%`));
  stage.addEventListener('contextmenu', (e) => contextMenu(e, [
    { label: 'Fit', icon: 'fit', run: fit },
    { label: 'Actual size', icon: 'image', run: actual },
    '-',
    { label: 'Save PNG…', icon: 'save', run: () => saveAs(src, title + '.png') },
    { label: 'Copy name', icon: 'copy', run: () => copy(title) },
  ]));
  const view = h('div', { class: 'page' },
    h('div', { class: 'toolbar' }, h('span', { class: 'title', text: title }), h('span', { class: 'faint mono', text: meta || '' }), h('span', { class: 'grow' }), zoomEl,
      h('button', { class: 'btn', onclick: fit }, icon('fit'), 'Fit'), h('button', { class: 'btn', onclick: actual }, '1:1'),
      h('button', { class: 'btn', onclick: () => saveAs(src, title + '.png') }, icon('save'), 'Save PNG')),
    stage);
  openTab({ id, title, icon: 'image', closable: true, view, show: () => requestAnimationFrame(fit) });
}

// ---------------------------------------------------------------- video

// The movies, once listed; `list` is the side panel's filtered order, which
// the player's next and previous follow.
const VIDEO = {
  all: null, list: null, bink2: false,
  async load() {
    if (!this.all) { const r = await api('/api/movies'); this.all = r.items; this.bink2 = r.bink2; }
    return this.all;
  },
  order() { return this.list || this.all || []; },
};

// A movie: converted to MP4 once as it opens (progress on the poster),
// then played with the keys a video player has. `replace`: the tab this
// one takes the place of (next and previous).
function openMovie(m, replace) {
  const id = 'movie:' + m.o;
  if (S.tabs.find((t) => t.id === id)) { showTab(id); return; }
  const tracks = m.tracks || [];
  const status = h('span', { class: 'faint mono status' });
  const stage = h('div', { class: 'videostage' });
  const poster = `/api/movie/poster?o=${m.o}`;
  const still = h('img', { class: 'poster', src: poster, alt: '' });
  still.addEventListener('error', () => still.remove());
  const bar = h('i'), what = h('div', { class: 'what', text: 'Preparing…' }), sub = h('div', { class: 'sub' });
  const cover = h('div', { class: 'cover' }, h('div', { class: 'box glass' }, what, h('div', { class: 'bar' }, bar), sub));
  const flash = h('div', { class: 'flash glass' });
  stage.append(still, cover, flash);
  // A movie with several sound tracks (other mixes or voice-overs) picks one.
  const pick = tracks.length > 1 ? h('select', { class: 'pick', title: 'Sound track' }, ...tracks.map((tid, i) => h('option', { value: i, text: `Track ${i + 1} (id ${tid})` }))) : null;
  const speed = h('select', { class: 'pick', title: 'Speed ([ and ])' }, ...[0.25, 0.5, 0.75, 1, 1.25, 1.5, 2].map((v) => h('option', { value: v, text: `${v}×` })));
  speed.value = '1';
  let loop = store.get('video.loop', false);
  const loopBtn = h('button', { class: 'btn icon-only' + (loop ? ' on' : ''), title: 'Loop (O)' }, icon('loop'));
  const nav = (d) => {
    const list = VIDEO.order();
    const i = list.findIndex((x) => x.o === m.o);
    const next = i < 0 ? null : list[i + d];
    if (next) openMovie(next, id); else toast(d > 0 ? 'This is the last movie in the list' : 'This is the first movie in the list');
  };
  const prevBtn = h('button', { class: 'btn icon-only', title: 'Previous movie (P)', onclick: () => nav(-1) }, icon('prev'));
  const nextBtn = h('button', { class: 'btn icon-only', title: 'Next movie (N)', onclick: () => nav(1) }, icon('next'));
  const saveFrame = h('button', { class: 'btn', disabled: true, title: 'Save the frame shown, at the movie\'s full size (S)' }, icon('image'), 'Frame');
  const saveMp4 = h('button', { class: 'btn', disabled: true, title: 'Save the MP4 Vice Lens made (picture at most 1280 wide, with the sound track chosen)' }, icon('save'), 'MP4');
  const saveBk2 = h('button', { class: 'btn', title: 'Save the movie as the game stores it (Bink 2)', onclick: () => saveAs(`/api/movie/raw?o=${m.o}`, `movie-${m.k}.bk2`) }, icon('save'), '.bk2');
  const view = h('div', { class: 'page' }, h('div', { class: 'toolbar' }, prevBtn, nextBtn, h('span', { class: 'title', title: `${m.w}×${m.h} · ${fmtT(m.seconds)} · ${m.fps} fps`, text: `Movie #${m.k}` }),
    h('span', { class: 'grow' }), status, pick, speed, loopBtn, h('span', { class: 'sep' }), saveFrame, saveMp4, saveBk2), stage);
  let video = null, url = null, flashT = 0;
  const say = (t) => { flash.textContent = t; flash.classList.add('on'); clearTimeout(flashT); flashT = setTimeout(() => flash.classList.remove('on'), 900); };
  const track = () => (pick ? +pick.value : 0);
  const soundText = (r) => {
    if (!tracks.length) return r.note ? `no sound in the file; ${r.note}` : 'no sound in the file';
    if (!r.sound) return `no sound: ${r.note || 'not converted'}`;
    const which = tracks.length > 1 ? `track ${track() + 1} of ${tracks.length}` : 'its track';
    if (r.picture === false) return `sound only: ${r.note}${r.silent ? `; ${which} is silent in the file` : ''}`;
    const sound = r.silent ? `${which} is silent in the file` : tracks.length > 1 ? `sound: ${which}` : 'with sound';
    return r.note ? `${sound}; ${r.note}` : sound;
  };
  const fail = (why) => {
    what.replaceChildren(h('span', { class: 'bad', text: why }));
    sub.textContent = '';
    bar.parentElement.classList.add('hidden');
    if (pick) pick.disabled = false;
  };
  const play = (r) => {
    const at = video ? video.currentTime : 0;
    const resume = video ? !video.paused : true;
    const v = h('video', { src: r.url, controls: true, preload: 'auto', playsinline: true, tabindex: '-1' });
    v.volume = store.get('video.volume', 1);
    v.muted = store.get('video.muted', false);
    v.loop = loop;
    v.playbackRate = +speed.value;
    v.addEventListener('volumechange', () => { store.set('video.volume', v.volume); store.set('video.muted', v.muted); });
    v.addEventListener('loadedmetadata', () => { if (at) v.currentTime = at; if (resume) v.play().catch(() => say('Space plays')); }, { once: true });
    if (video) video.replaceWith(v); else { still.remove(); cover.remove(); stage.prepend(v); }
    video = v;
    url = r.url;
    m.ready = true;
    status.textContent = soundText(r);
    saveMp4.disabled = false;
    saveFrame.disabled = r.picture === false;
    if (pick) pick.disabled = false;
  };
  const prepare = async () => {
    if (pick) pick.disabled = true;
    const r = await api('/api/movie/prepare', { o: m.o, t: track() }, { method: 'POST' }).catch((e) => ({ error: e.message }));
    if (r.ready) { play(r); return; }
    if (r.error) { if (video) { status.textContent = r.error; if (pick) pick.disabled = false; } else fail(r.error); return; }
    if (video) status.textContent = 'Adding that sound track…';
    else { what.textContent = 'Converting to MP4 (only the first time)…'; sub.textContent = 'Starting…'; }
  };
  const onJobs = (ev) => {
    const j = ev.detail.find((x) => x.what === `Converting movie ${m.o}`);
    if (!j || j.finished || video) return;
    const pct = j.total ? Math.min(100, Math.floor((100 * j.done) / j.total)) : 0;
    bar.style.width = pct + '%';
    sub.textContent = `${pct}% · frame ${fmtNum(j.done)} of ${fmtNum(j.total)}`;
  };
  const onDone = (ev) => { if (ev.detail.what.endsWith(m.o)) prepare(); };
  window.addEventListener('jobs', onJobs);
  window.addEventListener('job-done', onDone);
  pick?.addEventListener('change', prepare);
  speed.addEventListener('change', () => { if (video) video.playbackRate = +speed.value; });
  loopBtn.addEventListener('click', () => { loop = !loop; store.set('video.loop', loop); loopBtn.classList.toggle('on', loop); if (video) video.loop = loop; say(loop ? 'Loop on' : 'Loop off'); });
  saveMp4.addEventListener('click', () => url && saveAs(url, `movie-${m.k}${tracks.length > 1 ? `-track${track() + 1}` : ''}.mp4`));
  saveFrame.addEventListener('click', () => { if (!video) return; const t = video.currentTime; saveAs(`/api/movie/frame?o=${m.o}&t=${t.toFixed(3)}`, `movie-${m.k}-${fmtT(t).replace(':', 'm')}s.png`); });
  stage.addEventListener('dblclick', () => toggleFull());
  const toggleFull = () => {
    if (document.fullscreenElement || document.webkitFullscreenElement) (document.exitFullscreen || document.webkitExitFullscreen).call(document);
    else (stage.requestFullscreen || stage.webkitRequestFullscreen)?.call(stage);
  };
  const keys = (e) => {
    if (S.active !== id || e.defaultPrevented || e.ctrlKey || e.metaKey || e.altKey) return;
    if (/INPUT|TEXTAREA|SELECT/.test(document.activeElement?.tagName) || e.target.closest?.('#side, #insp, .menu, #palette')) return;
    const k = e.key, v = video;
    if (k === 'n' || k === 'N') nav(1);
    else if (k === 'p' || k === 'P') nav(-1);
    else if (!v) return;
    else if (k === ' ' || k === 'k') { if (v.paused) v.play(); else v.pause(); }
    else if (k === 'ArrowLeft' || k === 'ArrowRight' || k === 'j' || k === 'l') { const d = { ArrowLeft: -5, ArrowRight: 5, j: -10, l: 10 }[k]; v.currentTime = Math.max(0, Math.min(v.duration || 0, v.currentTime + d)); say(`${fmtT(v.currentTime)} / ${fmtT(v.duration)}`); }
    else if (k === ',' || k === '.') { v.pause(); v.currentTime = Math.max(0, v.currentTime + (k === ',' ? -1 : 1) / (m.fps || 30)); say(`frame ${Math.round(v.currentTime * m.fps)}`); }
    else if (k === 'Home') v.currentTime = 0;
    else if (k === 'End') v.currentTime = Math.max(0, (v.duration || 0) - 0.05);
    else if (k === 'ArrowUp' || k === 'ArrowDown') { v.muted = false; v.volume = Math.max(0, Math.min(1, v.volume + (k === 'ArrowUp' ? 0.1 : -0.1))); say(`Volume ${Math.round(v.volume * 100)}%`); }
    else if (k === 'm') { v.muted = !v.muted; say(v.muted ? 'Muted' : 'Sound on'); }
    else if (k === 'f') toggleFull();
    else if (k === '[' || k === ']') { const o = [...speed.options], i = Math.max(0, Math.min(o.length - 1, speed.selectedIndex + (k === ']' ? 1 : -1))); speed.selectedIndex = i; v.playbackRate = +speed.value; say(`Speed ${speed.value}×`); }
    else if (k === 'o') loopBtn.click();
    else if (k === 's') saveFrame.click();
    else if (/^[0-9]$/.test(k)) { v.currentTime = ((v.duration || 0) * +k) / 10; say(`${fmtT(v.currentTime)} / ${fmtT(v.duration)}`); }
    else return;
    e.preventDefault();
  };
  window.addEventListener('keydown', keys);
  const sound = !tracks.length ? 'none in the file' : tracks.length === 1 ? `one track (id ${tracks[0]})` : `${tracks.length} tracks (ids ${tracks.join(', ')})`;
  const key = (k, d) => [h('kbd', { text: k }), h('span', { text: d })];
  openTab({ id, title: `Movie #${m.k}`, icon: 'film', closable: true, view,
    hide: () => video?.pause(),
    close: () => { video?.pause(); video?.removeAttribute('src'); window.removeEventListener('keydown', keys); window.removeEventListener('jobs', onJobs); window.removeEventListener('job-done', onDone); },
    show: () => inspect(h('div', { class: 'insp-title' }, h('div', { class: 'n', text: `Movie #${m.k}` }), h('div', { class: 's', text: 'Bink 2' })),
      kv([['Offset', m.o], ['Stored size', fmtBytes(m.bytes)], ['Picture', `${m.w} × ${m.h}`], ['Frames', `${fmtNum(m.frames)} at ${m.fps} fps`], ['Length', `${fx(m.seconds, 1)} s`], ['Sound', sound]]),
      h('div', { class: 'insp-h', text: 'Keys' }),
      h('div', { class: 'keys' }, ...key('Space', 'play, pause'), ...key('← →', '5 s back, on'), ...key('J L', '10 s back, on'), ...key(', .', 'a frame back, on'), ...key('0–9', 'jump to 0–90 %'),
        ...key('↑ ↓', 'volume'), ...key('M', 'mute'), ...key('[ ]', 'slower, faster'), ...key('O', 'loop'), ...key('F', 'full screen'), ...key('S', 'save the frame'), ...key('N P', 'next, previous movie'))) }, replace);
  prepare();
}

// ---------------------------------------------------------------- sound

function openBank(k, o) {
  const id = 'bank:' + o;
  if (S.tabs.find((t) => t.id === id)) { showTab(id); return; }
  const body = h('tbody');
  const player = h('audio', { controls: true });
  const now = h('span', { class: 'faint mono' });
  const wrap = h('div', { class: 'table-wrap', style: 'top:86px' });
  const saveBtn = h('button', { class: 'btn', disabled: true, title: 'Save the stream picked as Ogg Opus' }, icon('save'), 'Save stream');
  const view = h('div', { class: 'page' },
    h('div', { class: 'toolbar' }, h('span', { class: 'title', text: `Bank #${k}` }), h('span', { class: 'faint mono', text: `audio.rpf ${o}` }), h('span', { class: 'grow' }), now, saveBtn),
    h('div', { class: 'toolbar', style: 'height:48px' }, player),
    wrap);
  const url = (i) => `/api/sound?o=${o}&i=${i}`;
  const picked = () => body.querySelector('tr.on')?._s;
  const syncSave = () => { const s = picked(); saveBtn.disabled = !s?.playable; };
  saveBtn.addEventListener('click', () => { const s = picked(); if (s?.playable) saveAs(url(s.i), `bank${k}-${s.id}.opus`); });
  const play = (tr) => {
    if (!tr._s.playable) { toast('This stream does not play yet (music or another codec)'); return; }
    [...body.children].forEach((x) => x.classList.toggle('on', x === tr));
    player.src = url(tr._s.i);
    player.play().catch(() => {});
    now.textContent = `stream ${tr._s.i} · ${tr._s.id}`;
    syncSave();
  };
  // Playing through a bank: the next stream starts when one ends.
  player.addEventListener('ended', () => { const on = body.querySelector('tr.on'); const next = on?.nextElementSibling; if (next && next._s.playable) play(next); });
  const table = h('table', { class: 'grid' }, h('thead', {}, h('tr', {}, h('th', { text: '#' }), h('th', { text: 'Id (hash)' }), h('th', { text: 'Length' }), h('th', { text: 'Rate' }), h('th', { text: 'Codec' }), h('th', { text: 'Size' }), h('th', { text: '' }))), body);
  wrap.append(table);
  navigable(wrap, {
    rows: () => [...body.children],
    open: play,
    select: syncSave,
    menu: (tr) => [
      { label: 'Play', icon: 'play', key: 'Enter', disabled: !tr._s.playable, run: () => play(tr) },
      { label: 'Save as Ogg Opus…', icon: 'save', disabled: !tr._s.playable, run: () => saveAs(url(tr._s.i), `bank${k}-${tr._s.id}.opus`) },
      '-',
      { label: 'Copy id', icon: 'copy', run: () => copy(tr._s.id) },
    ],
  });
  openTab({ id, title: `Bank #${k}`, icon: 'wave', closable: true, view, hide: () => player.pause() });
  api('/api/bank', { o }).then((r) => {
    body.replaceChildren(...r.items.map((s) => {
      const tr = h('tr', { onclick: () => { [...body.children].forEach((x) => x.classList.remove('on')); tr.classList.add('on'); syncSave(); }, ondblclick: () => play(tr) },
        h('td', { class: 'num', text: s.i }), h('td', { text: s.id }), h('td', { class: 'num', text: s.seconds ? `${fx(s.seconds, 2)} s` : '—' }), h('td', { class: 'num', text: s.rate ? `${fmtNum(s.rate)} Hz` : '—' }),
        h('td', { class: 'dim', text: `0x${s.codec.toString(16).padStart(2, '0')}` }), h('td', { class: 'num', text: fmtBytes(s.bytes) }),
        h('td', {}, s.playable ? h('span', { class: 'badge ok', text: 'Opus' }) : h('span', { class: 'badge', text: r.music ? 'music' : 'not yet' })));
      tr._s = s;
      return tr;
    }));
    inspect(h('div', { class: 'insp-title' }, h('div', { class: 'n', text: `Bank #${k}` }), h('div', { class: 's', text: r.music ? 'music (multi-channel)' : 'effects and speech' })),
      kv([['Archive', 'audio.rpf'], ['Offset', o], ['Streams', r.items.length], ['Playable', r.items.filter((s) => s.playable).length]]),
      h('div', { class: 'empty', text: 'Enter or double-click plays; ↓ then Enter for the next. A stream plays on into the next one.' }));
    wrap.focus();
  }).catch((e) => body.replaceChildren(h('tr', {}, h('td', { text: e.message }))));
}

// ---------------------------------------------------------------- text

// The game's text in a table: the search's matches, or one subtitle table
// whole (a scene or a conversation).
const TEXT = {
  p: { q: '' },
  table: null,
  view: null,
  body: null,
  info: null,
  chip: null,
  box: null,
  // What the table holds, as request parameters.
  params() { return this.table ? { src: 'subs', table: this.table } : this.p; },
  tab() {
    if (!this.view) {
      this.body = h('tbody');
      this.info = h('span', { class: 'faint mono' });
      this.chip = h('span');
      this.view = h('div', { class: 'page' }, h('div', { class: 'toolbar' }, h('span', { class: 'title', text: 'Text' }), this.chip, this.info, h('span', { class: 'grow' }),
        h('button', { class: 'btn', title: 'Save every matching line (not only those shown) as a tab-separated file', onclick: () => saveAs(`/api/text?${new URLSearchParams(Object.entries({ ...this.params(), format: 'tsv' }).filter(([, v]) => v != null && v !== ''))}`, this.table ? `text-table-${this.table}.tsv` : 'text.tsv') }, icon('save'), 'Export .tsv')),
        h('div', { class: 'table-wrap' }, h('table', { class: 'grid' }, h('thead', {}, h('tr', {}, h('th', { text: 'Label' }), h('th', { text: 'From' }), h('th', { text: 'Text' }))), this.body)));
      const wrap = this.view.querySelector('.table-wrap');
      navigable(wrap, {
        rows: () => [...this.body.children],
        open: (r) => copy(r._t[1]),
        menu: (r) => [
          { label: 'Copy text', icon: 'copy', key: 'Enter', run: () => copy(r._t[1]) },
          { label: 'Copy label', icon: 'copy', run: () => copy(r._t[0]) },
          r._t[2] ? { label: `Show all of table ${r._t[2]}`, icon: 'text', run: () => this.openTable(r._t[2]) } : null,
        ],
      });
      this.run();
    }
    return { id: 'text', title: 'Text', icon: 'text', closable: true, view: this.view, close: () => { this.view = null; } };
  },
  search(p) {
    this.p = p;
    this.table = null;
    if (!this.view) { openTab(this.tab()); return; }
    showTab('text');
    this.run();
  },
  // From the palette: search the text for `q`.
  find(q) {
    store.set('search.text', { ...store.get('search.text', {}), q, re: false });
    this.p = { q };
    setActivity('text');
  },
  openTable(k) {
    this.table = k;
    if (!this.view) openTab(this.tab()); else showTab('text');
    this.run();
  },
  run: debounce(async function () {
    const t = TEXT, p = t.params(), mine = (t.seq = (t.seq || 0) + 1);
    t.info.textContent = 'Searching…';
    t.chip.replaceChildren(t.table ? h('span', { class: 'chip' }, `Table ${t.table}`, h('button', { title: 'Back to the search', onclick: () => { t.table = null; t.run(); } }, icon('x'))) : '');
    try {
      const r = await api('/api/text', { ...p, limit: 1500 });
      if (mine !== t.seq) return;
      t.box?.error('');
      t.info.textContent = `${fmtNum(r.total)} line${r.total === 1 ? '' : 's'}${r.total > r.items.length ? `, first ${fmtNum(r.items.length)}` : ''} of ${fmtNum(r.loaded)}`;
      t.info.title = `${fmtNum(r.ui)} interface lines; ${fmtNum(r.subs)} lines of dialogue and posts in ${fmtNum(r.tables)} tables`;
      t.body.replaceChildren(...r.items.map(([label, text, src]) => {
        const shown = cleanText(text);
        const tr = h('tr', { onclick: () => { [...t.body.children].forEach((x) => x.classList.remove('on')); tr.classList.add('on'); }, ondblclick: () => copy(shown), title: 'Double-click to copy' },
          h('td', { class: 'dim', text: label }),
          h('td', {}, src ? h('a', { href: '#', class: 'dim', title: 'Show the whole table (a scene, a conversation, a thread)', onclick: (e) => { e.preventDefault(); t.openTable(src); } }, `table ${src}`) : h('span', { class: 'faint', text: 'interface' })),
          h('td', { class: 'wrap' }, ...highlight(shown, t.table ? null : p)));
        tr._t = [label, shown, src];
        return tr;
      }));
      t.view.querySelector('.table-wrap').scrollTop = 0;
    } catch (e) {
      if (mine !== t.seq) return;
      t.info.textContent = '';
      t.box?.error(e.message);
    }
  }, 120),
};

// The game's markup made readable: line breaks, timing and colour codes.
function cleanText(s) {
  return s.replace(/~n~/g, '\n').replace(/~sl:[^~]*~/g, ' ').replace(/~(?:[a-z]|h[a-z]?|s|ws|1b|2b)~/gi, '').replace(/ {2,}/g, ' ').trim();
}

// ---------------------------------------------------------------- setup

async function openSetup() {
  const id = 'setup';
  const st = (S.status = await api('/api/status').catch(() => S.status));
  const input = h('input', { class: 'text plain', value: st?.game || '', placeholder: '/path/to/the/game/folder', spellcheck: 'false' });
  const note = h('div');
  const save = async () => {
    const r = await api('/api/setup', { game: input.value }, { method: 'POST' }).catch((e) => ({ ok: false, error: e.message }));
    note.replaceChildren(h('div', { class: 'note ' + (r.ok ? 'ok' : 'bad'), text: r.ok ? 'Saved. Restart Vice Lens to read from the new folder.' : r.error }));
  };
  const browse = (what) => window.ipc ? h('button', { class: 'btn', onclick: () => window.ipc.postMessage(`pick-${what === 'ffmpeg' ? 'file' : 'folder'}:${what}`) }, icon('folder'), 'Browse…') : null;
  const ff = h('input', { class: 'text plain', placeholder: 'ffmpeg (next to Vice Lens, else from the PATH)', spellcheck: 'false' });
  const ffNote = h('div');
  const ffShow = (r) => ffNote.replaceChildren(h('div', { class: 'note ' + (r.bink2 ? 'ok' : 'bad'), text: r.bink2 ? `Plays Bink 2: ${r.ffmpeg}` : r.ffmpeg ? `${r.ffmpeg} does not decode Bink 2; movies will not play.` : 'No FFmpeg found; movies will not play.' }));
  // How the app window draws (Linux only: WebKitGTK's GPU or shared-memory path).
  const drawSeg = h('div', { class: 'seg' });
  const drawNote = h('div', { class: 'faint', style: 'margin-top:8px' });
  const drawShow = (r) => {
    drawSeg.replaceChildren(...[['compatible', 'Shared memory'], ['gpu', 'GPU']].map(([v, l]) =>
      h('button', { class: r.renderer === v ? 'on' : '', onclick: async () => drawShow(await api('/api/settings', { renderer: v })) }, l)));
    drawNote.textContent = `Drawing now: ${r.drawing === 'gpu' ? 'on the GPU' : 'through shared memory'}. A change takes effect when Vice Lens starts again.`;
  };
  api('/api/settings').then((r) => { ff.value = r.ffmpeg || ''; ffShow(r); drawShow(r); });
  const saveFf = async () => { const r = await api('/api/settings', { ffmpeg: ff.value.trim() }).catch((e) => ({ ok: false, error: e.message })); if (r.ok) ffShow(r); else ffNote.replaceChildren(h('div', { class: 'note bad', text: r.error })); };
  window.lensPicked = (what, path) => { if (!path) return; if (what === 'game') input.value = path; else if (what === 'ffmpeg') { ff.value = path; saveFf(); } };
  const caches = h('tbody', {}, ...(st?.caches || []).map((c) => h('tr', {},
    h('td', {}, c.present ? h('span', { class: 'badge ok', text: 'ready' }) : h('span', { class: 'badge warn', text: 'missing' })),
    h('td', { text: c.what }), h('td', { class: 'dim', text: c.about }), h('td', { class: 'num', text: c.present ? fmtBytes(c.bytes) : '' }))));
  const view = h('div', { class: 'page' }, h('div', { class: 'page-inner' },
    h('h2', { text: 'Setup' }),
    h('p', { text: 'Vice Lens reads your own copy of the game: the Xbox Series data of version 1.0.2.1. Nothing from the game ships with Vice Lens, and nothing in the folder is changed.' }),
    h('h3', { text: 'Game folder' }),
    h('div', { class: 'inline' }, input, browse('game'), h('button', { class: 'btn primary', onclick: save }, 'Save')),
    note,
    h('h3', { text: 'Videos' }),
    h('p', { text: 'The movies are Bink 2. Stock FFmpeg builds do not decode it; a build with the bink2video decoder does (for example one from the mobipeg project).' }),
    h('div', { class: 'inline' }, ff, browse('ffmpeg'), h('button', { class: 'btn primary', onclick: saveFf }, 'Save')),
    ffNote,
    st && !st.ok ? h('div', { class: 'note bad', text: 'levels.rpf is not in this folder.' }) : null,
    window.ipc ? h('h3', { text: 'Drawing' }) : null,
    window.ipc ? h('p', { text: 'How the window\'s web view (WebKitGTK) draws. Shared memory works everywhere. The GPU paces frames more evenly; if the window closes as it opens, Vice Lens goes back to shared memory by itself. Either way WebKitGTK may hold the page to 60 frames a second (it does on NVIDIA under Wayland); for the screen\'s full rate, open Vice Lens in your browser with the button in the top bar.' }) : null,
    window.ipc ? h('div', {}, drawSeg, drawNote) : null,
    h('h3', { text: 'Archives' }),
    h('div', { class: 'card' }, h('table', { class: 'grid' }, h('tbody', {}, ...(st?.archives || []).map((a) => h('tr', {}, h('td', { text: a.name }), h('td', { class: 'num', text: fmtBytes(a.bytes) })))))),
    h('h3', { text: 'Caches' }),
    h('p', {}, 'The map, the placements and the model index are read from your copy once (about an hour on a fast disk; about 2 GB) and kept in ', h('code', { text: st?.cache || '~/.cache/vice-lens' }), '. Anything missing is made; what is there is kept.'),
    h('div', { class: 'inline', style: 'margin-bottom:12px' },
      h('button', { class: 'btn primary', disabled: !st?.ok || (st?.caches || []).every((c) => c.present), onclick: async (e) => { e.currentTarget.disabled = true; const r = await api('/api/caches/build', {}, { method: 'POST' }); toast(r.started ? 'Building the caches; progress is in the status bar.' : 'Already building.'); } }, (st?.caches || []).every((c) => c.present) ? 'All caches are ready' : 'Build missing caches'),
      h('span', { class: 'faint', text: 'Progress shows in the status bar; the app stays usable.' })),
    h('div', { class: 'card' }, h('table', { class: 'grid' }, caches))));
  window.addEventListener('job-done', (ev) => { if (ev.detail.what.startsWith('Caches') && S.active === 'setup') openSetup(); }, { once: true });
  const old = S.tabs.find((t) => t.id === id);
  if (old) closeTabForce(old);
  openTab({ id, title: 'Setup', icon: 'gear', closable: true, view });
}

function closeTabForce(t) {
  const i = S.tabs.indexOf(t);
  if (i >= 0) { S.tabs.splice(i, 1); t.view.remove(); }
}

window.lensToast = toast;

// ---------------------------------------------------------------- start

async function start() {
  initShell();
  MAP = new MapView();
  openTab({ id: 'map', title: 'Map', icon: 'map', closable: false, view: MAP.view, show: () => MAP.redraw() });
  setActivity(store.get('activity', 'map') === 'setup' ? 'map' : store.get('activity', 'map'));
  try {
    S.status = await api('/api/status');
    $('#version').textContent = 'v' + S.status.version;
    const pill = $('#game-pill');
    pill.classList.add(S.status.ok ? 'ok' : 'bad');
    pill.lastChild.textContent = S.status.ok ? S.status.game.split('/').filter(Boolean).pop() : 'Choose game folder';
    pill.title = S.status.ok ? S.status.game : 'No game folder: open Setup';
    if (!S.status.ok) openSetup();
  } catch { /* the server will tell on the next call */ }
  MAP.init();
  inspect(h('div', { class: 'empty' }, 'Select a placement on the map, a model, or a file to see its details here.', h('br'), h('br'), h('kbd', { text: 'Ctrl K' }), ' searches everything.'));
  if (location.hash === '#probe') probe();
}

start();
