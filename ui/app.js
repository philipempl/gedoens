// The gedoens interface.
// It shows things and passes clicks on. Decisions are made in the backend.
(() => {
const invoke = window.__TAURI__.core.invoke;

const ICONS = {
  file: '<path d="M5.5 2.5h6L15 6v11a.5.5 0 0 1-.5.5h-9A.5.5 0 0 1 5 17V3a.5.5 0 0 1 .5-.5z"/><path d="M11.5 2.5V6H15"/>',
  lock: '<rect x="4.5" y="9" width="11" height="8" rx="2.2"/><path d="M7 9V6.5a3 3 0 0 1 6 0V9"/>',
  copy: '<rect x="7" y="7" width="10" height="10" rx="2.2"/><path d="M4 13V5a2 2 0 0 1 2-2h7"/>',
  pen: '<path d="M12.5 4.5l3 3L7 16H4v-3z"/>',
  trash: '<path d="M4 6h12M8 6V4h4v2M6 6l.7 10.5h6.6L14 6"/>',
  plus: '<path d="M10 4v12M4 10h12"/>',
  search: '<circle cx="9" cy="9" r="5.5"/><path d="M13.2 13.2L17 17"/>',
  theme: '<circle cx="10" cy="10" r="7"/><path d="M10 3v14a7 7 0 0 0 0-14z" fill="currentColor"/>',
  home: '<path d="M3.5 9.5L10 4l6.5 5.5V16a.5.5 0 0 1-.5.5h-3.5V12h-5v4.5H4a.5.5 0 0 1-.5-.5z"/>',
  folder: '<path d="M2.5 6a1.5 1.5 0 0 1 1.5-1.5h3.5l2 2H16A1.5 1.5 0 0 1 17.5 8v6.5A1.5 1.5 0 0 1 16 16H4a1.5 1.5 0 0 1-1.5-1.5z"/>',
  back: '<path d="M12 4l-6 6 6 6"/>',
  chev: '<path d="M8 5l5 5-5 5"/>',
  check: '<path d="M4.5 10.5L8 14l7.5-8"/>',
  warn: '<path d="M10 3.5l7 12.5H3z"/><path d="M10 8.5v3.2M10 14v.01"/>',
  eye: '<path d="M2.5 10s2.7-5 7.5-5 7.5 5 7.5 5-2.7 5-7.5 5-7.5-5-7.5-5z"/><circle cx="10" cy="10" r="2.3"/>',
  team: '<circle cx="7.5" cy="7" r="2.8"/><path d="M2.5 16c.4-2.8 2.4-4.3 5-4.3s4.6 1.5 5 4.3"/><path d="M13 4.5a2.6 2.6 0 0 1 0 5M15 11.900c1.500.6 2.300 2 2.500 4.100"/>',
  x: '<path d="M5 5l10 10M15 5L5 15"/>',
  git: '<circle cx="6" cy="5" r="2"/><circle cx="6" cy="15" r="2"/><circle cx="14" cy="8" r="2"/><path d="M6 7v6M14 10c0 3-3 3-6 4"/>',
  ext: '<path d="M11 4h5v5M16 4l-7 7M14 12v3.5a.5.5 0 0 1-.5.5h-9a.5.5 0 0 1-.5-.5v-9a.5.5 0 0 1 .5-.5H8"/>',
  compare: '<rect x="3" y="4" width="5.5" height="12" rx="1.5"/><rect x="11.5" y="4" width="5.5" height="12" rx="1.5"/><path d="M5.75 8h0M14.25 8h0M5.75 12h0M14.25 12h0" stroke-width="2.4"/>',
  updown: '<path d="M7 8l3-3 3 3M7 12l3 3 3-3"/>',
  key: '<circle cx="7" cy="12.5" r="3.5"/><path d="M9.5 10L16 3.5M13.5 6l2 2"/>',
};
const icon = (name) => `<svg viewBox="0 0 20 20" aria-hidden="true">${ICONS[name]}</svg>`;
const esc = (text) => String(text ?? '').replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
const $ = (id) => document.getElementById(id);
const base = (path) => path.split('/').pop();
const plural = (n, one, many) => `${n} ${n === 1 ? one : many}`;
const nameOf = (person) => (person.me ? 'You' : person.name || person.short);
const areaName = (area) => (area.folder ? `${area.folder}/${area.name}` : area.name);

const state = {
  view: 'start', keys: [], keyStatus: null, folder: '', overview: null, file: null, access: null,
  open: null, value: '', editing: false, adding: false, copied: false,
  sheet: null, palette: false, filter: '', draft: '', collapsed: new Set(), shared: [], compare: null, cmode: 'same', cfolder: '', cfilter: '', changes: null, git: null, published: null, update: null, enter: true, just: null, undo: null, working: false,
};

// ---------- Backend ----------

let pending = 0;
async function call(command, args) {
  pending += 1;
  const slow = setTimeout(() => document.body.classList.add('busy'), 400);
  try {
    return await invoke(command, args);
  } catch (error) {
    toast(String(error));
    throw error;
  } finally {
    clearTimeout(slow);
    pending -= 1;
    if (pending === 0) document.body.classList.remove('busy');
  }
}

/**
 * Values used in more than one file. Comparing means decrypting every file, so it
 * runs in the background and never blocks the page; only places come back, no values.
 */
let sharedRun = 0;
async function loadShared() {
  const run = ++sharedRun;
  try {
    const shared = await invoke('shared_values');
    if (run !== sharedRun) return;
    state.shared = shared;
    // Do not pull the field out from under someone who is typing.
    if (document.activeElement && document.activeElement.tagName === 'INPUT') state.sharedPending = true;
    else draw();
  } catch (e) { /* no folder or no access: nothing to compare */ }
}

/** File names short enough to read, with the folder only where names would clash. */
function shortNames(files) {
  const bases = files.map(base);
  return files.map((f, i) => {
    if (bases.indexOf(bases[i]) === bases.lastIndexOf(bases[i])) return bases[i];
    const parts = f.split('/');
    return parts.slice(-2).join('/');
  });
}

async function loadChanges() {
  try {
    const changes = await invoke('changes');
    const before = state.changes;
    state.changes = changes;
    if (state.view === 'changes') syncGitForm();
    const same = before && JSON.stringify(before) === JSON.stringify(changes);
    if (same) return;
    if (document.activeElement && document.activeElement.tagName === 'INPUT') state.sharedPending = true;
    else draw();
  } catch (e) { /* not a repository or no folder */ }
}

function sharedAt(file, path) {
  const here = dotted(path);
  const hit = state.shared.find((s) => s.places.some((p) => p.file === file && dotted(p.path) === here));
  return hit ? hit.places.filter((p) => !(p.file === file && dotted(p.path) === here)) : null;
}

function sharedTask(s) {
  const files = [...new Set(s.places.map((p) => p.file))];
  const folders = [...new Set(files.map((f) => (f.includes('/') ? f.slice(0, f.lastIndexOf('/')) : '')))];
  const keys = [...new Set(s.places.map((p) => dotted(p.path)))];
  return {
    title: `Same value in ${shortNames(files).join(' and ')}`,
    detail: `${folders.length === 1 && folders[0] ? `${folders[0]} · ` : ''}${keys.length === 1 ? keys[0] : s.places.map((p) => `${dotted(p.path)} in ${shortNames(files)[files.indexOf(p.file)]}`).join(', ')}`,
    label: 'Show', act: 'open', file: s.places[0].file,
  };
}

async function refresh() {
  loadShared(); loadChanges();
  state.overview = await call('overview');
  if (state.view === 'file') state.file = await call('file', { file: state.file.path });
  if (state.view === 'access' || (state.view === 'home' && state.overview.unnamed)) state.access = await call('access');
  if (state.view === 'compare') state.compare = await call('compare');
}


async function openFolder(folder) {
  state.folder = await call('open_folder', { folder });
  try { localStorage.setItem('gedoens.folder', folder); } catch (e) { /* works without storage too */ }
  state.overview = await call('overview');
  if (state.overview.unnamed) state.access = await call('access');
  state.shared = []; loadShared(); state.changes = null; loadChanges();
  go('home');
}

function go(view) {
  Object.assign(state, { view, open: null, draft: '', value: '', editing: false, adding: false, enter: true });
  $('pane').scrollTo(0, 0);
}

// ---------- Messages ----------

function toast(message, undo) {
  clearTimeout(toast.timer);
  state.undo = undo || null;
  $('toast').innerHTML = `<div class="toast"><span>${esc(message)}</span>${undo ? '<button data-act="undo">Undo</button>' : ''}</div>`;
  toast.timer = setTimeout(() => { $('toast').innerHTML = ''; state.undo = null; }, undo ? 6000 : 3200);
}

// ---------- Views ----------

function topBar() {
  if (state.view === 'start') return '';
  const left = state.view === 'file'
    ? `<button class="icon" data-act="go" data-view="home" aria-label="Back to files" style="margin-left:-10px">${icon('back')}</button><span class="crumb">Files</span>`
    : `<span class="wm">gedoens</span><span class="crumb fine" style="margin-left:10px">${esc(state.folder)}</span>`;
  return `<header class="top"><div class="wrap">${left}<span class="sp"></span>
    <button class="icon" data-act="palette" aria-label="Search" title="Search">${icon('search')}</button>
    <button class="icon" data-act="theme" aria-label="Light or dark" title="Light or dark">${icon('theme')}</button>
    <button class="icon" data-act="pick" aria-label="Open another folder" title="Open another folder" style="margin-right:-10px">${icon('folder')}</button>
  </div></header>`;
}

function sidebar() {
  if (state.view === 'start' || !state.overview) return '';
  const o = state.overview;
  const current = state.view === 'file' && state.file ? state.file.path : null;
  const total = o.groups.reduce((n, g) => n + g.files.length, 0);
  const fileItem = (f) => `<button class="sitem ${current === f.path ? 'on' : ''}" data-act="open" data-file="${esc(f.path)}" title="${esc(f.path)}">
    ${icon(f.readable ? 'file' : 'lock')}<span class="n">${esc(f.name)}</span>${f.conflicted || (f.readable && f.drifted) ? '<span class="dot" title="Needs you"></span>' : ''}</button>`;
  return `<button class="brand" data-act="pick" title="Open another folder">
      <span class="mark">${icon('lock')}</span>
      <span class="nm"><b>${esc(state.folder)}</b><span class="fn">${plural(total, 'secret file', 'secret files')}</span></span>${icon('updown')}</button>
    <button class="searchbox" data-act="palette">${icon('search')}<span>Search</span><kbd>⌘K</kbd></button>
    <button class="sitem ${state.view === 'home' ? 'on' : ''}" data-act="go" data-view="home">${icon('home')}<span class="n">Overview</span>${o.issues.length + state.shared.length ? `<span class="count">${o.issues.length + state.shared.length}</span>` : ''}</button>
    <button class="sitem ${state.view === 'access' ? 'on' : ''}" data-act="go" data-view="access">${icon('team')}<span class="n">Access</span>${o.unnamed ? `<span class="count">${o.unnamed}</span>` : ''}</button>
    <button class="sitem ${state.view === 'compare' ? 'on' : ''}" data-act="go" data-view="compare">${icon('compare')}<span class="n">Compare</span>${state.shared.length ? `<span class="count">${state.shared.length}</span>` : ''}</button>
    ${state.changes && state.changes.isRepository ? `<button class="sitem ${state.view === 'changes' ? 'on' : ''}" data-act="go" data-view="changes">${icon('git')}<span class="n">Changes</span>${state.changes.files.length ? `<span class="count dirtyc">${state.changes.files.length}</span>` : ''}</button>` : ''}
    ${o.groups.map((group) => `<h2>${esc(group.folder || 'In the folder itself')}</h2>${group.files.map(fileItem).join('')}`).join('')}
    <span class="sp"></span>
    <div class="sfoot">
      <button class="sitem" data-act="pickKey">${icon('key')}<span class="n">Key folder</span></button>
      <button class="sitem" data-act="theme">${icon('theme')}<span class="n">Light or dark</span></button>
      ${updateCard()}
      <div class="ver"><span class="wm">gedoens</span><span>v${esc(state.update ? state.update.current : '')}</span></div>
    </div>`;
}

/** The update, when there is one: downloading quietly, then one click to restart. */
function updateCard() {
  const u = state.update;
  if (!u) return '';
  if (u.state === 'downloading') {
    return `<div class="upd"><span class="t"><b>Updating to ${esc(u.version)}</b><span class="fine">Downloading in the background${u.percent !== null && u.percent !== undefined ? ` · ${u.percent}%` : ''}</span></span>
      <span class="bar"><i style="width:${u.percent || 4}%"></i></span></div>`;
  }
  if (u.state === 'ready') {
    return `<div class="upd ready"><span class="t"><b>gedoens ${esc(u.version)} is ready</b><span class="fine">Restart to finish. Unsaved edits are lost.</span></span>
      <button class="btn pri sm" data-act="restartUpdate">Restart</button></div>`;
  }
  return '';
}

let updateTimer = null;
async function pollUpdate() {
  try {
    const before = state.update && `${state.update.state}${state.update.percent}`;
    state.update = await invoke('update_status');
    const after = `${state.update.state}${state.update.percent}`;
    if (before !== after && !(document.activeElement && document.activeElement.tagName === 'INPUT')) draw();
  } catch (e) { /* the demo or an old backend: no updates */ }
  clearTimeout(updateTimer);
  updateTimer = setTimeout(pollUpdate, state.update && state.update.state === 'downloading' ? 1500 : 60000);
}

function dock() {
  if (state.view === 'start') return '';
  const unnamed = state.overview ? state.overview.unnamed : 0;
  return `<div class="dock"><nav aria-label="Sections">
    <button class="${state.view === 'home' || state.view === 'file' ? 'on' : ''}" data-act="go" data-view="home">${icon('file')}Files</button>
    <button class="${state.view === 'compare' ? 'on' : ''}" data-act="go" data-view="compare">${icon('compare')}Compare</button>
    <button class="${state.view === 'access' ? 'on' : ''}" data-act="go" data-view="access">${icon('team')}Access${unnamed ? `<span class="count">${unnamed}</span>` : ''}</button>
  </nav></div>`;
}

function startView() {
  const k = state.keyStatus || { foundIn: [], searched: [] };
  const where = (paths) => paths.map((path) => `<code>${esc(path)}</code>`).join('');
  const body = state.keys.length
    ? `<div class="seal">${icon('check')}</div><h1>Found your key.</h1>
       <p>gedoens uses ${plural(state.keys.length, 'key', 'keys')} from</p><div class="paths">${where(k.foundIn)}</div>
       <button class="btn pri big" data-act="pick">Open folder</button>
       <button class="link" data-act="pickKey">Use another key folder</button>`
    : `<div class="seal">${icon('lock')}</div><h1>No key found.</h1>
       <p>If your key lives somewhere else, pick its folder once. gedoens remembers it.</p>
       <div class="pair"><button class="btn pri big" data-act="pickKey">Choose key folder</button>
       <button class="btn big" data-act="genkey">Create a new key</button></div>
       <details class="searched"><summary>Where gedoens looked</summary><div class="paths">${where(k.searched)}</div></details>`;
  return `<div class="start view ${state.enter ? 'enter' : ''}">${body}<div class="fine">Everything stays on your machine.</div></div>`;
}

function driftText(drift) {
  return drift.missing.length
    ? `${drift.missing.map(nameOf).join(', ')} still missing`
    : `${drift.extra.map(nameOf).join(', ')} should not be in it`;
}

function taskOf(issue) {
  if (issue.kind === 'conflict') {
    return { title: `${base(issue.file)} has a conflict`, detail: 'Two versions do not fit together after the pull', label: 'Merge', act: 'merge', file: issue.file };
  }
  if (issue.kind === 'drift') {
    return { title: `${base(issue.file)} has drifted`, detail: driftText(issue.drift), label: 'Align', act: 'align', file: issue.file };
  }
  return { title: 'A key has no name', detail: issue.person.short, label: 'Name it', act: 'go', view: 'access' };
}

function taskCard(task, primary) {
  return `<div class="task"><div class="t"><b>${esc(task.title)}</b><span>${esc(task.detail)}</span></div>
    <button class="btn ${primary ? 'pri' : ''}" data-act="${task.act}" ${task.file ? `data-file="${esc(task.file)}"` : ''} ${task.view ? `data-view="${task.view}"` : ''}>${task.label}</button></div>`;
}

/** Quick naming: every key nobody has named yet, with where it can read. */
function namingList(o) {
  const people = o.issues.filter((x) => x.kind === 'unnamed').map((x) => x.person);
  if (!people.length) return '';
  const a = state.access;
  const reach = (person) => {
    const row = a && a.people.find((p) => p.recipient === person.recipient);
    if (!row) return '';
    const areas = a.areas.filter((_, i) => row.areas[i]).map(areaName);
    return areas.length ? `reads ${areas.join(', ')}` : 'in no rule, only in files';
  };
  return `<section class="block"><div class="bh"><h2>Name your keys</h2><span class="fine">${plural(people.length, 'key has', 'keys have')} no name yet</span></div>
    <div class="list namel">${people.map((person) => `<div class="item nrow">
      <span class="ava un">?</span>
      <span class="nk"><span class="key">${esc(person.short)}</span><span class="fine">${esc(reach(person))}</span></span>
      <input class="field" data-name-for="${esc(person.recipient)}" placeholder="Who is this?" autocomplete="off" spellcheck="false">
      <button class="btn" data-act="name" data-recipient="${esc(person.recipient)}">Save</button></div>`).join('')}</div></section>`;
}

function homeView() {
  const o = state.overview;
  const fileCount = o.groups.reduce((n, g) => n + g.files.length, 0);
  if (!fileCount) {
    return `<div class="view ${state.enter ? 'enter' : ''}"><div class="empty"><h1>Nothing here yet.</h1>
      <p class="lead" style="margin:12px auto 0">This folder has no files encrypted with SOPS.</p>
      <button class="btn pri" data-act="pick">Open another folder</button></div></div>`;
  }
  const tasks = o.issues.filter((x) => x.kind !== 'unnamed').map(taskOf);
  if (state.shared.length) {
    const folders = [...new Set(state.shared.map((s) => { const f = s.places[0].file; return f.includes('/') ? f.slice(0, f.lastIndexOf('/')) : ''; }))];
    tasks.push({ title: `${plural(state.shared.length, 'secret value is', 'secret values are')} used in more than one environment`,
      detail: folders.join(', '), label: 'Compare', act: 'go', view: 'compare' });
  }
  const open = tasks.length + (o.unnamed ? 1 : 0);
  const circumference = 2 * Math.PI * 26;
  const share = o.checksTotal ? o.checksPassed / o.checksTotal : 1;
  const headline = open === 0 ? 'All in tune.' : open === 1 ? 'One thing is open.' : `${open} things are open.`;
  const status = (f) => (f.conflicted ? 'Conflict' : !f.readable ? 'Not readable for you' : f.drifted ? 'Drifted' : 'In tune');
  return `<div class="view ${state.enter ? 'enter' : ''}">
    <div class="hero">
      <svg class="ring" viewBox="0 0 64 64" aria-hidden="true"><circle class="bgc" cx="32" cy="32" r="26"/><circle class="fg" cx="32" cy="32" r="26" stroke-dasharray="${circumference}" stroke-dashoffset="${circumference * (1 - share)}"/></svg>
      <div><h1>${headline}</h1><p>${o.checksPassed} of ${o.checksTotal} checks passed</p></div>
    </div>
    ${namingList(o)}
    ${tasks.length ? `<section class="block"><div class="bh"><h2>Open items</h2></div>${tasks.map((task, i) => taskCard(task, i === 0 && !o.unnamed)).join('')}</section>` : ''}
    <div class="narrow">${o.groups.map((group) => `<h2>${esc(group.folder || 'In the folder itself')}</h2><div class="list">${group.files.map((f) => `
      <button class="item" data-act="open" data-file="${esc(f.path)}">${icon(f.readable ? 'file' : 'lock')}
        <span class="t"><span class="n">${esc(f.name)}</span><span class="s ${f.conflicted || (f.readable && f.drifted) ? 'a' : ''}">${status(f)}</span></span>${icon('chev')}</button>`).join('')}</div>`).join('')}</div>
    ${open ? '' : '<p class="lead">Every file matches its rule, every key has a name, and no secret is shared between environments.</p>'}
  </div>`;
}

/** Defaults for the commit form, kept while the user types. */
function syncGitForm() {
  const c = state.changes;
  if (!c) return;
  const paths = c.files.map((f) => f.path);
  if (!state.git) {
    const day = new Date().toISOString().slice(0, 10);
    const folders = [...new Set(paths.map((p) => (p.includes('/') ? p.split('/').slice(-2, -1)[0] : 'root')))];
    const slug = (folders.length === 1 ? folders[0] : 'update').toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '');
    state.git = {
      branch: `secrets/${day}-${slug || 'update'}`,
      newBranch: true,
      message: paths.length === 1 ? `Update secrets in ${paths[0]}` : `Update secrets in ${folders.join(', ')}`,
      mr: true,
      selected: new Set(paths),
    };
  } else {
    // New files are selected, vanished ones dropped.
    for (const p of paths) if (!state.git.known || !state.git.known.has(p)) state.git.selected.add(p);
    for (const p of [...state.git.selected]) if (!paths.includes(p)) state.git.selected.delete(p);
  }
  state.git.known = new Set(paths);
}

function changesView() {
  const c = state.changes;
  if (!c) return `<div class="view"><p class="fine">Looking at Git …</p></div>`;
  if (!c.isRepository) return `<div class="view ${state.enter ? 'enter' : ''}"><div class="empty"><h1>Not a Git repository.</h1><p class="lead" style="margin:12px auto 0">Open the folder of a repository to commit secret changes from here.</p></div></div>`;
  syncGitForm();
  const g = state.git;
  const result = state.published ? `<div class="done">${icon('check')}<div class="t"><b>Pushed to ${esc(state.published.branch)}</b>
      <span class="fine">Commit ${esc(state.published.commit)}${state.published.note ? ` · ${esc(state.published.note)}` : ''}</span></div>
      ${state.published.mergeRequestUrl ? `<button class="btn pri" data-act="openMr">${icon('ext')}Open merge request</button>` : ''}</div>` : '';
  const head = `<div class="eyebrow">${icon('git')} ${esc(c.branch || '')}${c.target ? ` → ${esc(c.target)}` : ''}</div><h1>Changes</h1>
    <p class="lead">Secret files that differ from the last commit. Only these can be committed from here; everything else in the repository stays as it is.</p>`;
  if (!c.files.length) {
    return `<div class="view ${state.enter ? 'enter' : ''}">${head}${result}<p class="fine nohit">No secret changes. Everything is committed.${c.others ? ` ${plural(c.others, 'other file has', 'other files have')} changes, gedoens leaves them alone.` : ''}</p></div>`;
  }
  const sign = { added: '+', changed: '~', removed: '−' };
  const files = c.files.map((f) => `<label class="cfile">
      <input type="checkbox" data-git-file="${esc(f.path)}" ${g.selected.has(f.path) ? 'checked' : ''}>
      <span class="t"><span class="p"><span class="mono">${esc(f.path)}</span><span class="tag st-${f.status}">${f.status}</span></span>
      <span class="keys">${f.keys.map((k) => `<span class="kc k-${k.kind}" title="${k.kind}">${sign[k.kind]} ${esc(dotted(k.path))}</span>`).join('')}
        ${f.reencrypted ? '<span class="fine">re-encrypted only (recipients or data key)</span>' : ''}
        ${!f.readable ? '<span class="fine">not readable for you, values not compared</span>' : ''}
        ${f.status === 'deleted' ? '<span class="fine">file removed</span>' : ''}</span></span></label>`).join('');
  const onTarget = !g.newBranch && c.branch === c.target;
  const warnAhead = g.newBranch && c.ahead > 0
    ? `<p class="warnp">${icon('warn')}<span>You are on <b>${esc(c.branch)}</b>, ${plural(c.ahead, 'commit', 'commits')} ahead of ${esc(c.target)}. The new branch starts here, so its merge request carries ${c.ahead === 1 ? 'that commit' : 'those commits'} too. Switch to ${esc(c.target)} first if you want only the secrets.</span></p>` : '';
  const count = g.selected.size;
  return `<div class="view ${state.enter ? 'enter' : ''}">${head}${result}
    <section class="block"><div class="bh"><h2>${plural(c.files.length, 'changed secret file', 'changed secret files')}</h2>${c.others ? `<span class="fine">${plural(c.others, 'other changed file stays', 'other changed files stay')} untouched</span>` : ''}</div>
      <div class="list">${files}</div></section>
    <section class="block"><div class="bh"><h2>Commit</h2></div>
      <div class="gform">
        <div class="segs"><button class="seg ${g.newBranch ? 'on' : ''}" data-act="gitBranchMode" data-new="1">New branch</button><button class="seg ${g.newBranch ? '' : 'on'}" data-act="gitBranchMode" data-new="0">Current branch <span class="c">${esc(c.branch || '')}</span></button></div>
        ${g.newBranch ? `<label class="l" for="gBranch">Branch</label><input class="field mono" id="gBranch" value="${esc(g.branch)}" autocomplete="off" spellcheck="false">` : ''}
        <label class="l" for="gMsg">Message</label><input class="field" id="gMsg" value="${esc(g.message)}" autocomplete="off">
        <label class="check"><input type="checkbox" id="gMr" ${g.mr && !onTarget && c.target ? 'checked' : ''} ${onTarget || !c.target ? 'disabled' : ''}>Open a merge request into <b>${esc(c.target || '—')}</b></label>
        ${warnAhead}
        <div class="rowbtn"><span class="fine sp">${plural(count, 'file', 'files')} selected</span><button class="btn pri" data-act="gitAsk" ${count ? '' : 'disabled'}>${icon('git')}Commit &amp; push</button></div>
      </div></section>
  </div>`;
}

/**
 * How one variable looks across the files of its folder, in words:
 * the same value everywhere it appears, different values, or only in one file.
 */
function verdict(row, files) {
  const present = row.cells.map((c, k) => (c.present ? k : -1)).filter((k) => k >= 0);
  if (present.length < 2) return { kind: 'only', label: `Only in ${base(files[present[0]] || '')}` };
  const groups = present.map((k) => row.cells[k].same);
  const same = groups[0] !== null && groups.every((g) => g === groups[0]);
  return same ? { kind: 'same', label: present.length === files.length ? 'Same value' : `Same in ${present.length} files` } : { kind: 'diff', label: 'Different' };
}

function compareView() {
  const c = state.compare;
  const query = state.cfilter.trim().toLowerCase();
  const folders = c.folders.filter((f) => !state.cfolder || f.folder === state.cfolder);
  const chip = (value, label) => `<button class="seg ${state.cfolder === value ? 'on' : ''}" data-act="cfolder" data-folder="${esc(value)}">${esc(label)}</button>`;
  let sameTotal = 0;
  const sections = folders.map((f) => {
    const judged = f.rows.map((r) => ({ r, v: verdict(r, f.files) }));
    sameTotal += judged.filter((x) => x.v.kind === 'same').length;
    const rows = judged.filter(({ r, v }) => (state.cmode === 'all' || v.kind === 'same') && (!query || dotted(r.path).toLowerCase().includes(query)));
    if (!rows.length) return '';
    // Other variables in this folder that hold one of this row's values.
    const alsoIn = (row) => {
      const ids = new Set(row.cells.map((x) => x.same).filter((x) => x !== null));
      return f.rows.filter((o) => o !== row && o.cells.some((x) => ids.has(x.same))).map((o) => dotted(o.path));
    };
    const names = shortNames(f.files);
    const sameCount = judged.filter((x) => x.v.kind === 'same').length;
    return `<section class="block"><div class="bh"><h2 class="mono">${esc(f.folder || 'In the folder itself')}</h2>
        <span class="fine">${sameCount ? `${plural(sameCount, 'variable has', 'variables have')} the same value in ${f.files.length > 2 ? 'several files' : 'both files'}` : 'every value differs'}</span></div>
      <div class="dtable cmp"><table>
        <colgroup><col class="ck">${f.files.map(() => '<col class="cf">').join('')}<col class="cv"></colgroup>
        <thead><tr><th>Variable</th>${f.files.map((file, k) => `<th><button class="fh" data-act="open" data-file="${esc(file)}">${esc(names[k])}</button></th>`).join('')}<th>Compared</th></tr></thead>
        <tbody>${rows.map(({ r, v }) => {
          const also = alsoIn(r);
          return `<tr class="trow v-${v.kind}"><td class="tk" title="${esc(dotted(r.path))}">${r.path.length > 1 ? `<span class="pre">${esc(dotted(r.path.slice(0, -1)))}.</span>` : ''}${esc(String(r.path[r.path.length - 1]))}</td>
          ${r.cells.map((x, k) => (x.present
            ? `<td class="cc"><button class="has" data-act="jump" data-file="${esc(f.files[k])}" data-var="${esc(dotted(r.path))}" title="Open in ${esc(names[k])}">${icon('lock')}</button></td>`
            : '<td class="cc"><span class="none" title="Not in this file">—</span></td>')).join('')}
          <td class="vd"><span class="pill p-${v.kind}">${v.kind === 'same' ? icon('warn') : ''}${esc(v.label)}</span>${also.length ? `<span class="also" title="${esc(also.join(', '))}">also in ${esc(also.length === 1 ? also[0].split('.').pop() : `${also.length} other variables`)}</span>` : ''}</td></tr>`;
        }).join('')}</tbody>
      </table></div></section>`;
  }).join('');
  return `<div class="view ${state.enter ? 'enter' : ''}">
    <h1>Same values</h1>
    <p class="lead">Secrets with capital names, compared between the files of each folder. “Same value” means preview and production use the identical secret. Values are compared, never shown.</p>
    <div class="tools">
      <label class="filter">${icon('search')}<input id="cfilter" value="${esc(state.cfilter)}" placeholder="Filter variables" autocomplete="off" spellcheck="false"></label>
      <div class="segs"><button class="seg ${state.cmode === 'same' ? 'on' : ''}" data-act="cmode" data-mode="same">Same value <span class="c">${sameTotal}</span></button><button class="seg ${state.cmode === 'all' ? 'on' : ''}" data-act="cmode" data-mode="all">All variables</button></div>
    </div>
    ${c.folders.length > 1 ? `<div class="chips">${chip('', 'All folders')}${c.folders.map((f) => chip(f.folder, f.folder.split('/').pop() || '/')).join('')}</div>` : ''}
    ${sections || `<p class="fine nohit">${c.folders.length ? (state.cmode === 'same' && !query ? 'No secret has the same value in two environments.' : 'Nothing matches.') : 'No secrets with capital names found.'}</p>`}
    ${c.skipped.length ? `<p class="fine">Left out, not readable for you: ${c.skipped.map(esc).join(', ')}</p>` : ''}
  </div>`;
}

const dotted = (path) => path.map((step, n) => (typeof step === 'number' ? `[${step}]` : `${n ? '.' : ''}${step}`)).join('');

const dirty = () => state.open !== null && state.draft !== state.value;

function rowOf(entry, i) {
  const open = state.open === i;
  // Inside a section the first step is the heading, so only the rest is shown.
  const inner = entry.path.length > 1 ? entry.path.slice(1) : entry.path;
  const last = inner[inner.length - 1];
  const prefix = inner.length > 1 ? dotted(inner.slice(0, -1)) + (typeof last === 'number' ? '' : '.') : '';
  const leaf = typeof last === 'number' ? `[${last}]` : String(last);
  const changed = open && dirty();
  const others = sharedAt(state.file.path, entry.path);
  const warning = others ? `<span class="warnb" title="${esc(`Same value as ${others.map((p) => `${dotted(p.path)} in ${p.file}`).join(', ')}`)}">${icon('warn')}same as ${esc((() => { const fs = [state.file.path, ...new Set(others.map((p) => p.file))]; return shortNames(fs).slice(1).filter((n, i, all) => all.indexOf(n) === i).join(', '); })())}</span>` : '';
  const value = open
    ? `<input class="cellin" id="cellEdit" value="${esc(state.draft)}" autocomplete="off" spellcheck="false" aria-label="Value of ${esc(dotted(entry.path))}">`
    : `<button class="mask" data-act="reveal" data-i="${i}" aria-label="Show ${esc(dotted(entry.path))}">••••••••</button>`;
  const tools = open
    ? `<span class="confirm"><button class="btn pri sm" data-act="save" ${changed ? '' : 'disabled'}>Save</button><button class="btn sm" data-act="discard" ${changed ? '' : 'disabled'}>Discard</button></span>
       <button class="icon sm" data-act="copy" title="Copy (clears after 30 s)" aria-label="Copy">${icon(state.copied ? 'check' : 'copy')}</button>
       <button class="icon sm" data-act="remove" title="Delete" aria-label="Delete">${icon('trash')}</button>
       <button class="icon sm" data-act="hide" title="Hide (Esc)" aria-label="Hide">${icon('x')}</button>`
    : `<button class="icon sm ghost" data-act="reveal" data-i="${i}" title="Show" aria-label="Show">${icon('eye')}</button>`;
  return `<tr class="trow ${open ? 'open' : ''} ${changed ? 'dirty' : ''}" data-row="${i}">
    <td class="tk" title="${esc(dotted(entry.path))}"><span class="kn">${prefix ? `<span class="pre">${esc(prefix)}</span>` : ''}${esc(leaf)}</span>${warning}</td>
    <td class="tv">${value}</td><td class="ta">${tools}</td></tr>`;
}

/** Leaves grouped by their first step. Values at the top level come first. */
function sectionsOf(entries, filter) {
  const query = filter.trim().toLowerCase();
  const sections = new Map([['', []]]);
  entries.forEach((entry, i) => {
    if (entry.group) return;
    if (query && !dotted(entry.path).toLowerCase().includes(query)) return;
    const key = entry.path.length > 1 ? String(entry.path[0]) : '';
    if (!sections.has(key)) sections.set(key, []);
    sections.get(key).push(i);
  });
  return [...sections].filter(([, rows]) => rows.length);
}

function avatars(people) {
  const shown = people.slice(0, 5);
  const more = people.length - shown.length;
  const names = people.map((p) => (p.name || p.me ? nameOf(p) : p.short)).join(', ');
  return `<button class="stack" data-act="go" data-view="access" title="${esc(names)}">${shown.map((p) => {
    const named = p.name || p.me;
    return `<span class="ava ${named ? '' : 'un'}">${named ? esc(nameOf(p)[0]) : '?'}</span>`;
  }).join('')}${more > 0 ? `<span class="ava more">+${more}</span>` : ''}<span class="lbl">${plural(people.length, 'key', 'keys')} can read this</span></button>`;
}

function fileView() {
  const f = state.file;
  let note = '';
  if (f.conflicted) {
    note = taskCard({ title: 'Conflict after the pull', detail: 'Two versions do not fit together', label: 'Merge', act: 'merge', file: f.path }, true);
  } else if (!f.readable) {
    note = taskCard({ title: 'This is gibberish to you.', detail: 'Your key is not in this file', label: 'Ask for access', act: 'ask' }, true);
  } else if (f.drift) {
    note = taskCard({ title: 'Drifted', detail: driftText(f.drift), label: 'Align', act: 'align', file: f.path }, true);
  }
  const editable = f.readable && !f.conflicted;
  const folder = f.path.includes('/') ? f.path.slice(0, f.path.lastIndexOf('/')) : '';
  const values = f.entries.filter((e) => !e.group).length;
  const sections = sectionsOf(f.entries, state.filter);
  const adding = state.adding
    ? `<div class="addbox"><input class="field mono" id="newKey" placeholder="Name, for example api_token or alloy.pushUrl" autocomplete="off" spellcheck="false"><input class="field mono" id="newValue" placeholder="Value" autocomplete="off" spellcheck="false">
       <div class="acts"><button class="btn pri" data-act="addSave">Add</button><button class="btn" data-act="cancel">Cancel</button></div></div>`
    : '';
  const tools = editable && values ? `<div class="tools">
      <label class="filter">${icon('search')}<input id="filter" value="${esc(state.filter)}" placeholder="Filter ${values} values" autocomplete="off" spellcheck="false"></label>
      <button class="btn" data-act="foldAll">${icon('updown')}${state.collapsed.size ? 'Expand all' : 'Collapse all'}</button>
      <button class="btn" data-act="add">${icon('plus')}Add</button></div>` : '';
  const searching = state.filter.trim() !== '';
  const body = sections.length
    ? `<div class="dtable"><table>
        <colgroup><col class="ck"><col><col class="ca"></colgroup>
        <thead><tr><th>Key</th><th>Value</th><th></th></tr></thead>
        ${sections.map(([name, rows]) => {
          const shut = name && !searching && state.collapsed.has(name);
          const head = name ? `<tr class="tsec ${shut ? 'shut' : ''}"><th colspan="3"><button data-act="fold" data-name="${esc(name)}" aria-expanded="${!shut}">${icon('chev')}<span class="nm">${esc(name)}</span><span class="c">${rows.length}</span></button></th></tr>` : '';
          return `<tbody>${head}${shut ? '' : rows.map((i) => rowOf(f.entries[i], i)).join('')}</tbody>`;
        }).join('')}
      </table></div>`
    : editable && values ? `<p class="fine nohit">Nothing matches “${esc(state.filter)}”.</p>` : '';
  return `<div class="view ${state.enter ? 'enter' : ''}">
    <div class="fhead"><div class="eyebrow">${esc(folder)}</div><h1>${esc(f.name)}</h1>
      <div class="meta">${avatars(f.readers)}${f.area ? `<span class="tag">${esc(f.area)}</span>` : ''}</div></div>
    ${note}
    ${tools}
    ${adding}
    ${body}
    ${editable && !values && !state.adding ? `<button class="add" data-act="add">${icon('plus')}Add entry</button>` : ''}
  </div>`;
}

function personCell(person) {
  const named = person.name || person.me;
  return `<th scope="row"><button class="pr" data-act="person" data-recipient="${esc(person.recipient)}" title="${esc(person.recipient)}">
    <span class="ava ${named ? '' : 'un'}">${named ? esc(nameOf(person)[0]) : '?'}</span>
    <span class="pt"><span class="pn ${named ? '' : 'un'}">${named ? esc(nameOf(person)) : 'Unnamed'}</span><span class="key">${esc(person.short)}</span></span></button></th>`;
}

/** Areas in folder order: folders in the order they first appear, rules within them as written. */
function areaColumns(areas) {
  const order = [];
  for (const area of areas) if (!order.includes(area.folder ?? '')) order.push(area.folder ?? '');
  return order.map((folder) => ({ folder, areas: areas.filter((a) => (a.folder ?? '') === folder) }));
}

function accessTable(a) {
  const groups = areaColumns(a.areas);
  const flat = groups.flatMap((g) => g.areas);
  const firsts = new Set(groups.map((g) => g.areas[0]));
  // You first, then everyone with a name, then the keys nobody has named yet.
  const people = [...a.people].sort((x, y) => (y.me - x.me) || (!!y.name - !!x.name) || nameOf(x).localeCompare(nameOf(y)));
  const head1 = groups.map((g) => `<th colspan="${g.areas.length}" class="fgrp" scope="colgroup">${esc(g.folder || 'other')}</th>`).join('');
  const head2 = flat.map((area) => `<th class="acol ${firsts.has(area) ? 'first' : ''}" scope="col" title="${esc(`${area.pattern || 'all files'} · ${plural(area.files, 'file', 'files')}`)}">
    <span class="an">${area.editable ? '' : icon('lock')}${esc(area.name)}</span></th>`).join('');
  const rows = people.map((p) => `<tr>${personCell(p)}${flat.map((area) => {
    const i = a.areas.indexOf(area);
    const on = p.areas[i];
    const label = `${nameOf(p)} ${on ? 'can' : 'cannot'} read ${areaName(area)}`;
    return `<td class="${firsts.has(area) ? 'first' : ''}"><button class="cell ${on ? 'on' : ''} ${area.editable ? '' : 'lock'}" data-act="toggleAccess" data-recipient="${esc(p.recipient)}" data-area="${i}" aria-pressed="${on}" aria-label="${esc(label)}" title="${esc(label)}">${on ? icon('check') : ''}</button></td>`;
  }).join('')}</tr>`).join('');
  return `<div class="matrix"><table>
    <thead><tr><th class="corner" rowspan="2" scope="col">Key</th>${head1}</tr><tr>${head2}</tr></thead>
    <tbody>${rows}</tbody></table></div>`;
}

function accessView() {
  const a = state.access;
  if (!a.hasPolicy) {
    return `<div class="view ${state.enter ? 'enter' : ''}"><div class="empty"><h1>No rules yet.</h1>
      <p class="lead" style="margin:12px auto 0">This folder has no .sops.yaml. Once it has one, you manage who can read what here.</p></div></div>`;
  }
  const unnamed = a.people.filter((p) => !p.name && !p.me).length;
  const locked = !a.areas.some((area) => area.editable);
  const notes = [
    locked ? taskCard({ title: 'You cannot change anything here', detail: 'Your key is in none of these areas', label: 'Ask for access', act: 'ask' }, true) : '',
    unnamed ? `<p class="fine hint">${plural(unnamed, 'key has', 'keys have')} no name yet. Click a key to name it.</p>` : '',
  ].join('');
  return `<div class="view ${state.enter ? 'enter' : ''}">
    <h1>Who can read what</h1>
    <p class="lead">One row per key, one column per rule from .sops.yaml, grouped by folder. Click a box to grant or revoke. gedoens updates the files.</p>
    ${notes}
    ${accessTable(a)}
    <button class="btn wide" data-act="invite">${icon('plus')}Invite someone</button>
  </div>`;
}

function paletteItems(query) {
  const files = state.overview ? state.overview.groups.flatMap((g) => g.files.map((f) => ({ title: f.name, hint: g.folder, act: 'open', file: f.path, icon: 'file' }))) : [];
  const commands = [
    { title: 'Access', act: 'go', view: 'access', icon: 'team' },
    { title: 'Invite someone', act: 'invite', icon: 'plus' },
    { title: 'Open another folder', act: 'pick', icon: 'folder' },
    { title: 'Light or dark', act: 'theme', icon: 'theme' },
    { title: 'Choose key folder', act: 'pickKey', icon: 'lock' },
    { title: 'Check for updates', act: 'checkUpdate', icon: 'updown' },
  ];
  const hits = [...files, ...commands].filter((x) => `${x.title} ${x.hint || ''}`.toLowerCase().includes(query.toLowerCase()));
  if (!hits.length) return '<div class="fine" style="padding:14px">Nothing found. Search looks at names, never at values.</div>';
  return hits.map((x) => `<button class="item" data-act="${x.act}" ${x.file ? `data-file="${esc(x.file)}"` : ''} ${x.view ? `data-view="${x.view}"` : ''}>${icon(x.icon)}<span class="t"><span class="n">${esc(x.title)}</span></span>${x.hint ? `<span class="h">${esc(x.hint)}</span>` : ''}</button>`).join('');
}

function layer() {
  const sheet = state.sheet;
  if (state.palette) {
    return `<div class="scrim topa" data-act="close"><div class="pal" role="dialog" aria-label="Search"><input id="query" placeholder="File or command" autocomplete="off"><div class="lst" id="hits">${paletteItems('')}</div></div></div>`;
  }
  if (sheet === 'invite') {
    const areas = state.access.areas.filter((area) => area.editable);
    return `<div class="scrim" data-act="close"><div class="sheet" role="dialog" aria-label="Invite someone"><h3>Invite someone</h3><p>Ask them for their public key. gedoens does the rest.</p>
      <label class="l" for="inviteName">Name</label><input class="field" id="inviteName" autocomplete="off">
      <label class="l" for="inviteKey">Public Key</label><input class="field mono" id="inviteKey" placeholder="age1…" autocomplete="off" spellcheck="false">
      <label class="l">May read</label>${areas.map((area, i) => `<label class="check"><input type="checkbox" value="${area.index}" ${i === 0 ? 'checked' : ''}>${esc(areaName(area))}</label>`).join('')}
      <div class="err" id="inviteError"></div>
      <div class="rowbtn"><button class="btn" data-act="close">Cancel</button><button class="btn pri" data-act="inviteSave">Invite</button></div></div></div>`;
  }
  if (sheet && sheet.revoke) {
    const { person, area } = sheet.revoke;
    return `<div class="scrim" data-act="close"><div class="sheet" role="dialog" aria-label="Revoke access"><h3>Remove ${esc(nameOf(person))} from ${esc(areaName(area))}</h3>
      <p>${plural(area.files, 'file is', 'files are')} re-encrypted. Old versions in Git stay readable, so rotate the secrets themselves afterwards.</p>
      <div class="rowbtn"><button class="btn" data-act="close">Cancel</button><button class="btn pri" data-act="revokeSave">Revoke access</button></div></div></div>`;
  }
  if (sheet && sheet.publish) {
    const c = state.changes, g = state.git;
    const branch = g.newBranch ? g.branch : c.branch;
    const mr = g.mr && c.target && !(!g.newBranch && c.branch === c.target);
    return `<div class="scrim" data-act="close"><div class="sheet" role="dialog" aria-label="Commit and push"><h3>Commit and push?</h3>
      <p>${plural(g.selected.size, 'secret file', 'secret files')} go into one commit on ${g.newBranch ? 'the new branch' : ''} <b class="mono">${esc(branch)}</b>, pushed to origin${mr ? `, with a merge request into <b>${esc(c.target)}</b>` : ''}.</p>
      <div class="fullkey">${[...g.selected].map(esc).join('<br>')}</div>
      <p class="fine">“${esc(g.message)}”</p>
      <div class="rowbtn"><button class="btn" data-act="close">Cancel</button><button class="btn pri" data-act="gitPublish">${icon('git')}Commit &amp; push</button></div></div></div>`;
  }
  if (sheet && sheet.person) {
    const person = state.access.people.find((p) => p.recipient === sheet.person);
    if (!person) return '';
    const canRemove = !person.me && state.access.areas.some((area, i) => area.editable && person.areas[i]);
    const title = person.me ? 'Your key' : person.name || 'Unnamed key';
    const naming = person.me ? '' : `<label class="l" for="personName">Name</label>
      <div class="namer"><input class="field" id="personName" data-name-for="${esc(person.recipient)}" value="${esc(person.name || '')}" placeholder="Who does it belong to?" autocomplete="off">
      <button class="btn pri" data-act="name" data-recipient="${esc(person.recipient)}">Save</button></div>`;
    return `<div class="scrim" data-act="close"><div class="sheet" role="dialog" aria-label="${esc(title)}"><h3>${esc(title)}</h3>
      <div class="fullkey">${esc(person.recipient)}</div>${naming}
      <div class="rowbtn">${person.me ? `<button class="btn pri" data-act="myKey">${icon('copy')}Copy public key</button>` : ''}
      ${canRemove ? `<button class="btn" data-act="kick" data-recipient="${esc(person.recipient)}">${icon('x')}Remove everywhere</button>` : ''}
      <button class="btn" data-act="close">Close</button></div></div></div>`;
  }
  if (sheet && sheet.kick) {
    return `<div class="scrim" data-act="close"><div class="sheet" role="dialog" aria-label="Remove"><h3>Remove ${esc(nameOf(sheet.kick))}</h3>
      <p>Every file you can change is re-encrypted. Old versions in Git stay readable, so rotate the secrets themselves afterwards.</p>
      <div class="rowbtn"><button class="btn" data-act="close">Cancel</button><button class="btn pri" data-act="kickSave">Remove</button></div></div></div>`;
  }
  if (sheet && sheet.merge) {
    const { preview, at } = sheet.merge;
    const clash = preview.clashes[at];
    const taken = preview.automatic ? `${plural(preview.automatic, 'other change is', 'other changes are')} already merged. ` : '';
    return `<div class="scrim" data-act="close"><div class="sheet" role="dialog" aria-label="Merge"><h3>Which value applies to ${esc(clash.label)}?</h3>
      <p>${taken}You both changed this.${preview.clashes.length > 1 ? ` Decision ${at + 1} of ${preview.clashes.length}.` : ''}</p>
      <button class="choice" data-act="choose" data-side="ours"><small>Yours</small>${clash.ours === null ? 'deleted' : esc(clash.ours)}</button>
      <button class="choice" data-act="choose" data-side="theirs"><small>Theirs</small>${clash.theirs === null ? 'deleted' : esc(clash.theirs)}</button>
      <div class="rowbtn"><button class="btn" data-act="close">Later</button></div></div></div>`;
  }
  return '';
}

function draw() {
  document.body.classList.toggle('bare', state.view === 'start');
  document.body.classList.toggle('wide', ['access', 'file', 'compare', 'changes'].includes(state.view));
  $('side').innerHTML = sidebar();
  $('top').innerHTML = topBar();
  $('main').innerHTML = { start: startView, home: homeView, file: fileView, access: accessView, compare: compareView, changes: changesView }[state.view]();
  $('dock').innerHTML = dock();
  $('layer').innerHTML = layer();
  const cell = $('cellEdit');
  if (cell && state.just === 'cell') { cell.focus(); cell.setSelectionRange(cell.value.length, cell.value.length); }
  if (state.just === 'add' && $('newKey')) $('newKey').focus();
  if (state.palette) $('query').focus();
  if (state.just === 'invite' && $('inviteName')) $('inviteName').focus();
  if (state.just === 'person' && $('personName')) $('personName').focus();
  if (state.just === 'nextName') { const next = document.querySelector('.namel [data-name-for]'); if (next) next.focus(); }
  state.just = null;
  state.enter = false;
  state.sharedPending = false;
}

// ---------- Actions ----------

function setKeys(status) {
  state.keyStatus = status;
  state.keys = status.publicKeys;
}

const openEntry = () => state.file.entries[state.open];
const target = () => ({ file: state.file.path, path: openEntry().path });

function toggleTheme() {
  const root = document.documentElement;
  const current = root.dataset.theme || (matchMedia('(prefers-color-scheme:dark)').matches ? 'dark' : 'light');
  root.dataset.theme = current === 'dark' ? 'light' : 'dark';
  try { localStorage.setItem('gedoens.theme', root.dataset.theme); } catch (e) { /* does not matter */ }
}

async function merge(file) {
  const preview = await call('merge_preview', { file });
  if (preview.clashes.length) {
    state.sheet = { merge: { file, preview, at: 0, choices: [] } };
    return;
  }
  await call('merge_apply', { file, choices: [] });
  await refresh();
  toast(`${base(file)} merged, nothing was lost`);
}

const actions = {
  async genkey() { setKeys(await call('generate_key')); },
  async pickKey() {
    state.palette = false;
    const status = await call('pick_key_folder');
    if (!status) return;
    setKeys(status);
    toast(`Using the key from ${status.foundIn[0] || 'that folder'}`);
    if (state.view !== 'start') await refresh();
  },
  person(el) { state.sheet = { person: el.dataset.recipient }; state.just = 'person'; },
  async pick() {
    state.palette = false;
    const folder = await call('pick_folder');
    if (folder) await openFolder(folder);
  },
  async go(el) {
    state.palette = false; state.sheet = null;
    if (dirty()) { toast('Save or discard your change first'); state.just = 'cell'; return; }
    state.overview = await call('overview');
    if (el.dataset.view === 'access' || (el.dataset.view === 'home' && state.overview.unnamed)) state.access = await call('access');
    if (el.dataset.view === 'compare') state.compare = await call('compare');
    if (el.dataset.view === 'changes') { state.changes = await call('changes'); state.published = null; }
    go(el.dataset.view);
  },
  async open(el) {
    state.palette = false;
    if (dirty()) { toast('Save or discard your change first'); state.just = 'cell'; return; }
    state.file = await call('file', { file: el.dataset.file });
    state.filter = ''; state.collapsed = new Set();
    go('file');
  },
  async reveal(el) {
    const i = Number(el.dataset.i);
    if (state.open === i) return;
    if (dirty()) { toast('Save or discard your change first'); state.just = 'cell'; return; }
    state.adding = false;
    state.value = await call('reveal', { file: state.file.path, path: state.file.entries[i].path });
    state.draft = state.value;
    state.open = i; state.just = 'cell';
  },
  gitBranchMode(el) { state.git.newBranch = el.dataset.new === '1'; },
  gitAsk() {
    const g = state.git;
    if (g.newBranch && !g.branch.trim()) { toast('Name the branch'); return; }
    if (!g.message.trim()) { toast('Write a commit message'); return; }
    state.sheet = { publish: true };
  },
  async gitPublish() {
    const c = state.changes, g = state.git;
    state.sheet = null;
    const mr = g.mr && !!c.target && !(!g.newBranch && c.branch === c.target);
    state.published = await call('publish', { input: {
      files: [...g.selected], branch: g.newBranch ? g.branch.trim() : c.branch, newBranch: g.newBranch, message: g.message.trim(), mergeRequest: mr } });
    state.git = null;
    state.changes = await call('changes');
    state.overview = await call('overview');
    toast(state.published.mergeRequestUrl ? 'Pushed, merge request is open' : `Pushed to ${state.published.branch}`);
  },
  async restartUpdate() {
    if (dirty()) { toast('Save or discard your change first'); state.just = 'cell'; return; }
    await call('restart_to_update');
  },
  async checkUpdate() {
    state.palette = false;
    state.update = await call('check_for_update');
    const u = state.update;
    toast(u.state === 'ready' ? `gedoens ${u.version} is ready` : u.state === 'downloading' ? `Downloading ${u.version}` : u.state === 'failed' ? `No update check: ${u.message}` : `gedoens ${u.current} is the latest version`);
    pollUpdate();
  },
  async openMr() { if (state.published && state.published.mergeRequestUrl) await call('open_url', { url: state.published.mergeRequestUrl }); },
  cmode(el) { state.cmode = el.dataset.mode; },
  cfolder(el) { state.cfolder = el.dataset.folder; },
  async jump(el) {
    state.file = await call('file', { file: el.dataset.file });
    state.filter = el.dataset.var; state.collapsed = new Set();
    go('file');
  },
  hide() {
    if (dirty()) { toast('Save or discard your change first'); state.just = 'cell'; return; }
    state.open = null;
  },
  discard() { state.draft = state.value; state.just = 'cell'; },
  fold(el) {
    const name = el.dataset.name;
    if (state.collapsed.has(name)) state.collapsed.delete(name); else state.collapsed.add(name);
  },
  foldAll() {
    if (state.collapsed.size) { state.collapsed.clear(); return; }
    for (const e of state.file.entries) if (!e.group && e.path.length > 1) state.collapsed.add(String(e.path[0]));
  },
  async copy() {
    await call('copy_value', target());
    state.copied = true;
    setTimeout(() => { state.copied = false; if (document.activeElement.tagName !== 'INPUT') draw(); }, 1400);
    toast('Copied. The clipboard clears itself in 30 seconds.');
  },
  cancel() { state.adding = false; },
  async save() {
    // Only an explicit Save (button or Enter) writes. Leaving the field never does.
    if (!dirty()) return;
    const value = state.draft;
    const where = target(), label = dotted(openEntry().path);
    const previous = await call('set_value', { ...where, value });
    state.value = value; state.draft = value; state.just = 'cell';
    toast(`${label} saved`, async () => { await call('set_value', { ...where, value: previous ?? '' }); state.open = null; await refresh(); });
  },
  async remove() {
    const where = target(), label = dotted(openEntry().path);
    const previous = await call('remove_value', where);
    state.open = null;
    await refresh();
    toast(`${label} deleted`, async () => { await call('set_value', { ...where, value: previous ?? '' }); await refresh(); });
  },
  add() {
    if (dirty()) { toast('Save or discard your change first'); state.just = 'cell'; return; }
    state.adding = true; state.open = null; state.just = 'add';
  },
  async addSave() {
    const key = $('newKey').value.trim();
    if (!key) { $('newKey').focus(); return; }
    const where = { file: state.file.path, path: key.split('.').filter(Boolean) };
    await call('set_value', { ...where, value: $('newValue').value });
    state.adding = false;
    await refresh();
    toast(`${key} added`, async () => { await call('remove_value', where); await refresh(); });
  },
  async align(el) {
    await call('align', { file: el.dataset.file });
    await refresh();
    toast(`${base(el.dataset.file)} aligned`);
  },
  async merge(el) { await merge(el.dataset.file); },
  async choose(el) {
    const m = state.sheet.merge;
    m.choices.push({ path: m.preview.clashes[m.at].path, takeTheirs: el.dataset.side === 'theirs' });
    m.at += 1;
    if (m.at < m.preview.clashes.length) return;
    state.sheet = null;
    await call('merge_apply', { file: m.file, choices: m.choices });
    await refresh();
    toast(`${base(m.file)} merged`);
  },
  async ask() { await call('copy_public_key'); toast('Your public key is copied. Send it to someone with access.'); },
  async myKey() { await call('copy_public_key'); toast('Your public key is copied'); },
  async name(el) {
    const input = document.querySelector(`[data-name-for="${CSS.escape(el.dataset.recipient)}"]`);
    const name = input.value.trim();
    if (!name) { input.focus(); return; }
    await call('name_key', { recipient: el.dataset.recipient, name });
    state.sheet = null;
    await refresh();
    toast(`The key is now called ${name}`);
    const next = document.querySelector('.namel [data-name-for]');
    if (next) state.just = 'nextName';
  },
  async toggleAccess(el) {
    const person = state.access.people.find((p) => p.recipient === el.dataset.recipient);
    const area = state.access.areas[Number(el.dataset.area)];
    const has = person.areas[Number(el.dataset.area)];
    if (!area.editable) { toast(`You cannot change ${areaName(area)}. You have no access to it.`); return; }
    if (person.me) { toast('You cannot remove yourself'); return; }
    if (has) { state.sheet = { revoke: { person, area } }; return; }
    await call('grant', { recipient: person.recipient, area: area.index, name: person.name });
    await refresh();
    toast(`${nameOf(person)} can now read ${areaName(area)}`);
  },
  async revokeSave() {
    const { person, area } = state.sheet.revoke;
    state.sheet = null;
    const rotated = await call('revoke', { recipient: person.recipient, area: area.index });
    await refresh();
    toast(`${nameOf(person)} can no longer read ${areaName(area)}. ${plural(rotated, 'file', 'files')} rotated.`);
  },
  kick(el) { state.sheet = { kick: state.access.people.find((p) => p.recipient === el.dataset.recipient) }; },
  async kickSave() {
    const person = state.sheet.kick;
    state.sheet = null;
    for (const [i, area] of state.access.areas.entries()) {
      if (area.editable && person.areas[i]) await call('revoke', { recipient: person.recipient, area: area.index });
    }
    await refresh();
    toast(`${nameOf(person)} removed`);
  },
  async invite() {
    state.palette = false;
    if (state.view !== 'access') { state.access = await call('access'); go('access'); }
    if (!state.access.hasPolicy) return;
    state.sheet = 'invite'; state.just = 'invite';
  },
  async inviteSave() {
    const name = $('inviteName').value.trim();
    const recipient = $('inviteKey').value.replace(/\s+/g, '');
    const areas = [...document.querySelectorAll('.sheet input[type=checkbox]:checked')].map((box) => Number(box.value));
    const fail = (message, field) => { $('inviteError').textContent = message; if (field) $(field).focus(); };
    if (!name) return fail('Enter a name.', 'inviteName');
    if (!/^age1[a-z0-9]{16,}$/.test(recipient)) return fail('The key has to start with age1.', 'inviteKey');
    if (!areas.length) return fail('Pick at least one area.');
    state.sheet = null;
    for (const area of areas) await call('grant', { recipient, area, name });
    await refresh();
    toast(`${name} is in`);
  },
  async undo() {
    const undo = state.undo;
    state.undo = null;
    $('toast').innerHTML = '';
    if (undo) await undo();
  },
  palette() { state.palette = true; },
  theme() { state.palette = false; toggleTheme(); },
  close() { state.palette = false; state.sheet = null; },
};

async function run(el) {
  if (state.working) return;
  state.working = true;
  try {
    await actions[el.dataset.act](el);
  } catch (error) {
    // The message is already shown below. The view stays as it was.
  } finally {
    state.working = false;
    draw();
  }
}

document.addEventListener('click', (event) => {
  const el = event.target.closest('[data-act]');
  if (!el) {
    if (state.open !== null && !dirty() && !event.target.closest('.trow.open, #toast')) { state.open = null; draw(); }
    return;
  }
  if (el.classList.contains('scrim') && event.target !== el) return;
  run(el);
});

document.addEventListener('change', (event) => {
  const file = event.target.dataset && event.target.dataset.gitFile;
  if (file) {
    if (event.target.checked) state.git.selected.add(file); else state.git.selected.delete(file);
    draw();
  }
  if (event.target.id === 'gMr') state.git.mr = event.target.checked;
});

document.addEventListener('focusout', () => {
  setTimeout(() => {
    if (state.sharedPending && !(document.activeElement && document.activeElement.tagName === 'INPUT')) draw();
  }, 0);
});

document.addEventListener('input', (event) => {
  if (event.target.id === 'query') $('hits').innerHTML = paletteItems(event.target.value);
  if (event.target.id === 'cellEdit') {
    state.draft = event.target.value;
    const row = event.target.closest('.trow');
    row.classList.toggle('dirty', dirty());
    row.querySelectorAll('.confirm .btn').forEach((b) => { b.disabled = !dirty(); });
  }
  if (event.target.id === 'gBranch') state.git.branch = event.target.value;
  if (event.target.id === 'gMsg') state.git.message = event.target.value;
  if (event.target.id === 'cfilter') {
    const at = event.target.selectionStart;
    state.cfilter = event.target.value;
    draw();
    $('cfilter').focus(); $('cfilter').setSelectionRange(at, at);
  }
  if (event.target.id === 'filter') {
    const at = event.target.selectionStart;
    state.filter = event.target.value; state.open = null;
    draw();
    $('filter').focus(); $('filter').setSelectionRange(at, at);
  }
});

document.addEventListener('keydown', (event) => {
  const mod = event.metaKey || event.ctrlKey;
  const field = event.target.tagName === 'INPUT' ? event.target : null;
  const press = (selector) => { const el = document.querySelector(selector); if (el) el.click(); };
  if (mod && event.key.toLowerCase() === 'k' && state.view !== 'start') {
    event.preventDefault();
    state.palette = !state.palette; state.sheet = null; draw();
  } else if (mod && event.key.toLowerCase() === 'z' && !field && state.undo) {
    event.preventDefault();
    press('[data-act="undo"]');
  } else if (event.key === 'Escape') {
    if (state.palette || state.sheet) { state.palette = false; state.sheet = null; }
    else if (state.adding) state.adding = false;
    else if (field && field.id === 'cellEdit' && dirty()) { state.draft = state.value; state.just = 'cell'; }
    else if (state.filter && event.target.id === 'filter') state.filter = '';
    else if (!dirty()) state.open = null;
    draw();
  } else if (event.key === 'Enter' && field) {
    if (field.id === 'cellEdit') press('.trow.open [data-act="save"]');
    else if (field.id === 'newKey') $('newValue').focus();
    else if (field.id === 'newValue') press('[data-act="addSave"]');
    else if (field.id === 'query') press('#hits .item');
    else if (field.dataset.nameFor) press(`[data-act="name"][data-recipient="${CSS.escape(field.dataset.nameFor)}"]`);
    else if (field.id === 'inviteName') $('inviteKey').focus();
    else if (field.id === 'inviteKey') press('[data-act="inviteSave"]');
  }
});

// ---------- Start ----------

(async () => {
  try {
    const theme = localStorage.getItem('gedoens.theme');
    if (theme) document.documentElement.dataset.theme = theme;
  } catch (e) { /* does not matter */ }
  pollUpdate();
  try {
    setKeys(await call('key_status'));
    const opened = await call('current_folder');
    const last = localStorage.getItem('gedoens.folder');
    if (opened) {
      state.folder = opened;
      state.overview = await call('overview');
      if (state.overview.unnamed) state.access = await call('access');
      loadShared(); loadChanges();
      go('home');
    } else if (last && state.keys.length) {
      await openFolder(last);
    }
  } catch (e) {
    try { localStorage.removeItem('gedoens.folder'); } catch (e2) { /* does not matter */ }
  }
  draw();
})();
})();
