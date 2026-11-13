// Die Oberfläche von gedoens.
// Sie zeigt an und reicht Klicks weiter. Entschieden wird im Backend.
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
  team: '<circle cx="7.5" cy="7" r="2.8"/><path d="M2.5 16c.4-2.8 2.4-4.3 5-4.3s4.6 1.5 5 4.3"/><path d="M13 4.5a2.6 2.6 0 0 1 0 5M15 11.900c1.500.6 2.300 2 2.500 4.100"/>',
  x: '<path d="M5 5l10 10M15 5L5 15"/>',
};
const icon = (name) => `<svg viewBox="0 0 20 20" aria-hidden="true">${ICONS[name]}</svg>`;
const esc = (text) => String(text ?? '').replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
const $ = (id) => document.getElementById(id);
const base = (path) => path.split('/').pop();
const plural = (n, one, many) => `${n} ${n === 1 ? one : many}`;
const nameOf = (person) => (person.me ? 'Du' : person.name || person.short);

const state = {
  view: 'start', keys: [], folder: '', overview: null, file: null, access: null,
  open: null, value: '', editing: false, adding: false, copied: false,
  sheet: null, palette: false, enter: true, just: null, undo: null, working: false,
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

async function refresh() {
  state.overview = await call('overview');
  if (state.view === 'file') state.file = await call('file', { file: state.file.path });
  if (state.view === 'access') state.access = await call('access');
}

async function openFolder(folder) {
  state.folder = await call('open_folder', { folder });
  try { localStorage.setItem('gedoens.folder', folder); } catch (e) { /* ohne Speicher geht es auch */ }
  state.overview = await call('overview');
  go('home');
}

function go(view) {
  Object.assign(state, { view, open: null, editing: false, adding: false, enter: true });
  $('pane').scrollTo(0, 0);
}

// ---------- Meldungen ----------

function toast(message, undo) {
  clearTimeout(toast.timer);
  state.undo = undo || null;
  $('toast').innerHTML = `<div class="toast"><span>${esc(message)}</span>${undo ? '<button data-act="undo">Rückgängig</button>' : ''}</div>`;
  toast.timer = setTimeout(() => { $('toast').innerHTML = ''; state.undo = null; }, undo ? 6000 : 3200);
}

// ---------- Ansichten ----------

function topBar() {
  if (state.view === 'start') return '';
  const left = state.view === 'file'
    ? `<button class="icon" data-act="go" data-view="home" aria-label="Zurück zu den Dateien" style="margin-left:-10px">${icon('back')}</button><span class="crumb">Dateien</span>`
    : `<span class="wm">gedoens</span><span class="crumb fine" style="margin-left:10px">${esc(state.folder)}</span>`;
  return `<header class="top"><div class="wrap">${left}<span class="sp"></span>
    <button class="icon" data-act="palette" aria-label="Suchen" title="Suchen">${icon('search')}</button>
    <button class="icon" data-act="theme" aria-label="Hell oder Dunkel" title="Hell oder Dunkel">${icon('theme')}</button>
    <button class="icon" data-act="pick" aria-label="Anderen Ordner öffnen" title="Anderen Ordner öffnen" style="margin-right:-10px">${icon('folder')}</button>
  </div></header>`;
}

function sidebar() {
  if (state.view === 'start' || !state.overview) return '';
  const o = state.overview;
  const current = state.view === 'file' && state.file ? state.file.path : null;
  const fileItem = (f) => `<button class="sitem ${current === f.path ? 'on' : ''}" data-act="open" data-file="${esc(f.path)}" title="${esc(f.path)}">
    ${icon(f.readable ? 'file' : 'lock')}<span class="n">${esc(f.name)}</span>${f.conflicted || (f.readable && f.drifted) ? '<span class="dot" title="Braucht dich"></span>' : ''}</button>`;
  return `<div class="brand"><span class="wm">gedoens</span><span class="fine">${esc(state.folder)}</span></div>
    <button class="sitem ${state.view === 'home' ? 'on' : ''}" data-act="go" data-view="home">${icon('home')}<span class="n">Übersicht</span>${o.issues.length ? `<span class="count">${o.issues.length}</span>` : ''}</button>
    <button class="sitem ${state.view === 'access' ? 'on' : ''}" data-act="go" data-view="access">${icon('team')}<span class="n">Zugriff</span></button>
    ${o.groups.map((group) => `<h2>${esc(group.folder || 'Im Ordner selbst')}</h2>${group.files.map(fileItem).join('')}`).join('')}
    <span class="sp"></span>
    <div class="sfoot">
      <button class="icon" data-act="palette" aria-label="Suchen" title="Suchen">${icon('search')}</button>
      <button class="icon" data-act="theme" aria-label="Hell oder Dunkel" title="Hell oder Dunkel">${icon('theme')}</button>
      <button class="icon" data-act="pick" aria-label="Anderen Ordner öffnen" title="Anderen Ordner öffnen">${icon('folder')}</button>
    </div>`;
}

function dock() {
  if (state.view === 'start') return '';
  const unnamed = state.overview ? state.overview.unnamed : 0;
  return `<div class="dock"><nav aria-label="Bereiche">
    <button class="${state.view !== 'access' ? 'on' : ''}" data-act="go" data-view="home">${icon('file')}Dateien</button>
    <button class="${state.view === 'access' ? 'on' : ''}" data-act="go" data-view="access">${icon('team')}Zugriff${unnamed ? `<span class="count">${unnamed}</span>` : ''}</button>
  </nav></div>`;
}

function startView() {
  const body = state.keys.length
    ? `<div class="seal">${icon('check')}</div><h1>Dein Key ist schon da.</h1><p>Es gibt nichts einzurichten.</p>
       <button class="btn pri big" data-act="pick">Ordner öffnen</button>`
    : `<div class="seal">${icon('lock')}</div><h1>Erst brauchst du einen Key.</h1><p>Er entsteht auf deinem Rechner, an dem Ort, an dem auch SOPS ihn sucht.</p>
       <button class="btn pri big" data-act="genkey">Key erzeugen</button>`;
  return `<div class="start view ${state.enter ? 'enter' : ''}">${body}<div class="fine">Alles bleibt auf deinem Rechner.</div></div>`;
}

function driftText(drift) {
  return drift.missing.length
    ? `${drift.missing.map(nameOf).join(', ')} fehlt noch`
    : `${drift.extra.map(nameOf).join(', ')} steht zu viel drin`;
}

function taskOf(issue) {
  if (issue.kind === 'conflict') {
    return { title: `${base(issue.file)} hat einen Konflikt`, detail: 'Zwei Stände passen nach dem Pull nicht zusammen', label: 'Zusammenführen', act: 'merge', file: issue.file };
  }
  if (issue.kind === 'drift') {
    return { title: `${base(issue.file)} weicht ab`, detail: driftText(issue.drift), label: 'Angleichen', act: 'align', file: issue.file };
  }
  return { title: 'Ein Key hat keinen Namen', detail: issue.person.short, label: 'Benennen', act: 'go', view: 'access' };
}

function taskCard(task, primary) {
  return `<div class="task"><div class="t"><b>${esc(task.title)}</b><span>${esc(task.detail)}</span></div>
    <button class="btn ${primary ? 'pri' : ''}" data-act="${task.act}" ${task.file ? `data-file="${esc(task.file)}"` : ''} ${task.view ? `data-view="${task.view}"` : ''}>${task.label}</button></div>`;
}

function homeView() {
  const o = state.overview;
  const fileCount = o.groups.reduce((n, g) => n + g.files.length, 0);
  if (!fileCount) {
    return `<div class="view ${state.enter ? 'enter' : ''}"><div class="empty"><h1>Hier liegt noch nichts.</h1>
      <p class="lead" style="margin:12px auto 0">In diesem Ordner gibt es keine Dateien, die mit SOPS verschlüsselt sind.</p>
      <button class="btn pri" data-act="pick">Anderen Ordner öffnen</button></div></div>`;
  }
  const tasks = o.issues.map(taskOf);
  const circumference = 2 * Math.PI * 26;
  const share = o.checksTotal ? o.checksPassed / o.checksTotal : 1;
  const headline = tasks.length === 0 ? 'Alles im Einklang.' : tasks.length === 1 ? 'Eine Sache ist offen.' : `${tasks.length} Sachen sind offen.`;
  const status = (f) => (f.conflicted ? 'Konflikt' : !f.readable ? 'Für dich nicht lesbar' : f.drifted ? 'Weicht ab' : 'Im Einklang');
  return `<div class="view ${state.enter ? 'enter' : ''}">
    <div class="hero">
      <svg class="ring" viewBox="0 0 64 64" aria-hidden="true"><circle class="bgc" cx="32" cy="32" r="26"/><circle class="fg" cx="32" cy="32" r="26" stroke-dasharray="${circumference}" stroke-dashoffset="${circumference * (1 - share)}"/></svg>
      <div><h1>${headline}</h1><p>${o.checksPassed} von ${o.checksTotal} Prüfungen bestanden</p></div>
    </div>
    ${tasks.map((task, i) => taskCard(task, i === 0)).join('')}
    <div class="narrow">${o.groups.map((group) => `<h2>${esc(group.folder || 'Im Ordner selbst')}</h2><div class="list">${group.files.map((f) => `
      <button class="item" data-act="open" data-file="${esc(f.path)}">${icon(f.readable ? 'file' : 'lock')}
        <span class="t"><span class="n">${esc(f.name)}</span><span class="s ${f.conflicted || (f.readable && f.drifted) ? 'a' : ''}">${status(f)}</span></span>${icon('chev')}</button>`).join('')}</div>`).join('')}</div>
    ${tasks.length ? '' : '<p class="lead">Alle Dateien passen zu ihren Regeln, und jeder Key hat einen Namen.</p>'}
  </div>`;
}

function chips(people) {
  return people.map((p) => `<span class="chip ${p.name || p.me ? '' : 'un'}">${esc(nameOf(p))}</span>`).join('');
}

function rowOf(entry, i) {
  if (entry.group) return `<div class="grp" style="--d:${entry.depth}">${esc(entry.label)}</div>`;
  const open = state.open === i;
  let panel = '';
  if (open && state.editing) {
    panel = `<div class="panel"><input class="field mono" id="edit" value="${esc(state.value)}" autocomplete="off" spellcheck="false" aria-label="Neuer Wert für ${esc(entry.label)}">
      <div class="acts"><button class="btn pri" data-act="save">Speichern</button><button class="btn" data-act="cancel">Abbrechen</button></div></div>`;
  } else if (open) {
    panel = `<div class="panel ${state.just === i ? 'just' : ''}"><div class="val">${esc(state.value)}</div>
      <div class="acts"><button class="btn" data-act="copy">${icon(state.copied ? 'check' : 'copy')}${state.copied ? 'Kopiert' : 'Kopieren'}</button>
      <button class="btn" data-act="edit">${icon('pen')}Ändern</button><button class="btn" data-act="remove">${icon('trash')}Löschen</button></div></div>`;
  }
  return `<div class="row ${open ? 'open' : ''}"><button class="rowb" data-act="toggle" data-i="${i}" aria-expanded="${open}">
    <span class="k">${esc(entry.label)}</span><span class="v">${open ? '' : '••••••••'}</span>${icon('chev')}</button>${panel}</div>`;
}

function fileView() {
  const f = state.file;
  let note = '';
  if (f.conflicted) {
    note = taskCard({ title: 'Konflikt nach dem Pull', detail: 'Zwei Stände passen nicht zusammen', label: 'Zusammenführen', act: 'merge', file: f.path }, true);
  } else if (!f.readable) {
    note = taskCard({ title: 'Für dich ist das Kauderwelsch.', detail: 'Dein Key steht nicht in dieser Datei', label: 'Zugriff anfragen', act: 'ask' }, true);
  } else if (f.drift) {
    note = taskCard({ title: 'Weicht ab', detail: driftText(f.drift), label: 'Angleichen', act: 'align', file: f.path }, true);
  }
  const editable = f.readable && !f.conflicted;
  const adding = state.adding
    ? `<div class="addbox"><input class="field mono" id="newKey" placeholder="Name, zum Beispiel api_token" autocomplete="off" spellcheck="false"><input class="field mono" id="newValue" placeholder="Wert" autocomplete="off" spellcheck="false">
       <div class="acts"><button class="btn pri" data-act="addSave">Hinzufügen</button><button class="btn" data-act="cancel">Abbrechen</button></div></div>`
    : `<button class="add" data-act="add">${icon('plus')}Eintrag hinzufügen</button>`;
  return `<div class="view ${state.enter ? 'enter' : ''}">
    <h1>${esc(f.name)}</h1>
    <div class="who"><span>Lesen können</span>${chips(f.readers)}</div>
    ${note}
    ${f.entries.map(rowOf).join('')}
    ${editable ? adding : ''}
  </div>`;
}

function personCard(person, areas) {
  const canRemove = !person.me && areas.some((area, i) => area.editable && person.areas[i]);
  const head = person.name || person.me
    ? `<div class="pers"><span class="ava">${esc(nameOf(person)[0])}</span><div class="t"><div class="n">${esc(nameOf(person))}</div><span class="key">${esc(person.short)}</span></div>
        ${person.me ? `<button class="icon" data-act="myKey" aria-label="Deinen Public Key kopieren" title="Public Key kopieren">${icon('copy')}</button>`
          : canRemove ? `<button class="icon" data-act="kick" data-recipient="${esc(person.recipient)}" aria-label="${esc(nameOf(person))} entfernen" title="Entfernen">${icon('x')}</button>` : ''}</div>`
    : `<div class="pers"><span class="ava">?</span><div class="t"><div class="n">Unbekannter Key</div><span class="key">${esc(person.short)}</span></div></div>
       <div class="namer"><input class="field" data-name-for="${esc(person.recipient)}" placeholder="Wem gehört er?" autocomplete="off"><button class="btn pri" data-act="name" data-recipient="${esc(person.recipient)}">Benennen</button></div>`;
  const pills = areas.map((area, i) => `<button class="pill ${person.areas[i] ? 'on' : ''} ${area.editable ? '' : 'lock'}" data-act="toggleAccess" data-recipient="${esc(person.recipient)}" data-area="${i}" aria-pressed="${person.areas[i]}">${person.areas[i] ? icon('check') : ''}${esc(area.name)}</button>`).join('');
  return `<div class="card">${head}<div class="pills">${pills}</div></div>`;
}

function accessView() {
  const a = state.access;
  if (!a.hasPolicy) {
    return `<div class="view ${state.enter ? 'enter' : ''}"><div class="empty"><h1>Noch keine Regeln.</h1>
      <p class="lead" style="margin:12px auto 0">In diesem Ordner gibt es keine .sops.yaml. Sobald eine da ist, verwaltest du hier, wer was lesen darf.</p></div></div>`;
  }
  const unnamed = a.people.some((p) => !p.name && !p.me);
  return `<div class="view ${state.enter ? 'enter' : ''}">
    <h1>Wer darf was lesen</h1>
    <p class="lead">Tippe einen Bereich an, um Zugriff zu geben oder zu nehmen. Die Dateien zieht gedoens selbst nach.</p>
    ${a.people.map((p) => personCard(p, a.areas)).join('')}
    <button class="btn ${unnamed ? '' : 'pri'} wide" data-act="invite">${icon('plus')}Person einladen</button>
    <h2>Bereiche</h2>
    <div class="list">${a.areas.map((area) => `<div class="item"><span class="t"><span class="n">${esc(area.name)}</span><span class="key">${esc(area.pattern || 'alle Dateien')}</span></span><span class="s">${plural(area.files, 'Datei', 'Dateien')}</span></div>`).join('')}</div>
  </div>`;
}

function paletteItems(query) {
  const files = state.overview ? state.overview.groups.flatMap((g) => g.files.map((f) => ({ title: f.name, hint: g.folder, act: 'open', file: f.path, icon: 'file' }))) : [];
  const commands = [
    { title: 'Zugriff', act: 'go', view: 'access', icon: 'team' },
    { title: 'Person einladen', act: 'invite', icon: 'plus' },
    { title: 'Anderen Ordner öffnen', act: 'pick', icon: 'folder' },
    { title: 'Hell oder Dunkel', act: 'theme', icon: 'theme' },
  ];
  const hits = [...files, ...commands].filter((x) => `${x.title} ${x.hint || ''}`.toLowerCase().includes(query.toLowerCase()));
  if (!hits.length) return '<div class="fine" style="padding:14px">Nichts gefunden. Gesucht wird in Namen, nie in Werten.</div>';
  return hits.map((x) => `<button class="item" data-act="${x.act}" ${x.file ? `data-file="${esc(x.file)}"` : ''} ${x.view ? `data-view="${x.view}"` : ''}>${icon(x.icon)}<span class="t"><span class="n">${esc(x.title)}</span></span>${x.hint ? `<span class="h">${esc(x.hint)}</span>` : ''}</button>`).join('');
}

function layer() {
  const sheet = state.sheet;
  if (state.palette) {
    return `<div class="scrim topa" data-act="close"><div class="pal" role="dialog" aria-label="Suchen"><input id="query" placeholder="Datei oder Befehl" autocomplete="off"><div class="lst" id="hits">${paletteItems('')}</div></div></div>`;
  }
  if (sheet === 'invite') {
    const areas = state.access.areas.filter((area) => area.editable);
    return `<div class="scrim" data-act="close"><div class="sheet" role="dialog" aria-label="Person einladen"><h3>Person einladen</h3><p>Lass dir den Public Key schicken. Den Rest macht gedoens.</p>
      <label class="l" for="inviteName">Name</label><input class="field" id="inviteName" autocomplete="off">
      <label class="l" for="inviteKey">Public Key</label><input class="field mono" id="inviteKey" placeholder="age1…" autocomplete="off" spellcheck="false">
      <label class="l">Darf lesen</label>${areas.map((area, i) => `<label class="check"><input type="checkbox" value="${area.index}" ${i === 0 ? 'checked' : ''}>${esc(area.name)}</label>`).join('')}
      <div class="err" id="inviteError"></div>
      <div class="rowbtn"><button class="btn" data-act="close">Abbrechen</button><button class="btn pri" data-act="inviteSave">Einladen</button></div></div></div>`;
  }
  if (sheet && sheet.revoke) {
    const { person, area } = sheet.revoke;
    return `<div class="scrim" data-act="close"><div class="sheet" role="dialog" aria-label="Zugriff nehmen"><h3>${esc(nameOf(person))} aus ${esc(area.name)} nehmen</h3>
      <p>${plural(area.files, 'Datei wird', 'Dateien werden')} neu verschlüsselt. Alte Stände in Git bleiben lesbar. Tausch die Secrets selbst danach aus.</p>
      <div class="rowbtn"><button class="btn" data-act="close">Abbrechen</button><button class="btn pri" data-act="revokeSave">Zugriff nehmen</button></div></div></div>`;
  }
  if (sheet && sheet.kick) {
    return `<div class="scrim" data-act="close"><div class="sheet" role="dialog" aria-label="Entfernen"><h3>${esc(nameOf(sheet.kick))} entfernen</h3>
      <p>Alle Dateien, die du ändern kannst, werden neu verschlüsselt. Alte Stände in Git bleiben lesbar. Tausch die Secrets selbst danach aus.</p>
      <div class="rowbtn"><button class="btn" data-act="close">Abbrechen</button><button class="btn pri" data-act="kickSave">Entfernen</button></div></div></div>`;
  }
  if (sheet && sheet.merge) {
    const { preview, at } = sheet.merge;
    const clash = preview.clashes[at];
    const taken = preview.automatic ? `${plural(preview.automatic, 'andere Änderung ist', 'andere Änderungen sind')} schon übernommen. ` : '';
    return `<div class="scrim" data-act="close"><div class="sheet" role="dialog" aria-label="Zusammenführen"><h3>Welcher Wert gilt für ${esc(clash.label)}?</h3>
      <p>${taken}Hier habt ihr beide etwas geändert.${preview.clashes.length > 1 ? ` Entscheidung ${at + 1} von ${preview.clashes.length}.` : ''}</p>
      <button class="choice" data-act="choose" data-side="ours"><small>Deiner</small>${clash.ours === null ? 'gelöscht' : esc(clash.ours)}</button>
      <button class="choice" data-act="choose" data-side="theirs"><small>Der andere</small>${clash.theirs === null ? 'gelöscht' : esc(clash.theirs)}</button>
      <div class="rowbtn"><button class="btn" data-act="close">Später</button></div></div></div>`;
  }
  return '';
}

function draw() {
  document.body.classList.toggle('bare', state.view === 'start');
  $('side').innerHTML = sidebar();
  $('top').innerHTML = topBar();
  $('main').innerHTML = { start: startView, home: homeView, file: fileView, access: accessView }[state.view]();
  $('dock').innerHTML = dock();
  $('layer').innerHTML = layer();
  const edit = $('edit');
  if (edit) { edit.focus(); edit.select(); }
  if (state.just === 'add' && $('newKey')) $('newKey').focus();
  if (state.palette) $('query').focus();
  if (state.just === 'invite' && $('inviteName')) $('inviteName').focus();
  state.just = null;
  state.enter = false;
}

// ---------- Aktionen ----------

const openEntry = () => state.file.entries[state.open];
const target = () => ({ file: state.file.path, path: openEntry().path });

function toggleTheme() {
  const root = document.documentElement;
  const current = root.dataset.theme || (matchMedia('(prefers-color-scheme:dark)').matches ? 'dark' : 'light');
  root.dataset.theme = current === 'dark' ? 'light' : 'dark';
  try { localStorage.setItem('gedoens.theme', root.dataset.theme); } catch (e) { /* egal */ }
}

async function merge(file) {
  const preview = await call('merge_preview', { file });
  if (preview.clashes.length) {
    state.sheet = { merge: { file, preview, at: 0, choices: [] } };
    return;
  }
  await call('merge_apply', { file, choices: [] });
  await refresh();
  toast(`${base(file)} zusammengeführt, nichts ging verloren`);
}

const actions = {
  async genkey() { state.keys = (await call('generate_key')).publicKeys; },
  async pick() {
    state.palette = false;
    const folder = await call('pick_folder');
    if (folder) await openFolder(folder);
  },
  async go(el) {
    state.palette = false; state.sheet = null;
    if (el.dataset.view === 'access') state.access = await call('access');
    state.overview = await call('overview');
    go(el.dataset.view);
  },
  async open(el) {
    state.palette = false;
    state.file = await call('file', { file: el.dataset.file });
    go('file');
  },
  async toggle(el) {
    const i = Number(el.dataset.i);
    state.editing = false; state.adding = false;
    if (state.open === i) { state.open = null; return; }
    state.value = await call('reveal', { file: state.file.path, path: state.file.entries[i].path });
    state.open = i; state.just = i;
  },
  async copy() {
    await call('copy_value', target());
    state.copied = true;
    setTimeout(() => { state.copied = false; if (document.activeElement.tagName !== 'INPUT') draw(); }, 1400);
    toast('Kopiert. Die Zwischenablage leert sich in 30 Sekunden.');
  },
  edit() { state.editing = true; },
  cancel() { state.editing = false; state.adding = false; },
  async save() {
    const value = $('edit').value;
    state.editing = false;
    if (value === state.value) return;
    const where = target(), label = openEntry().label;
    const previous = await call('set_value', { ...where, value });
    state.value = value;
    toast(`${label} gespeichert`, async () => { await call('set_value', { ...where, value: previous ?? '' }); state.open = null; await refresh(); });
  },
  async remove() {
    const where = target(), label = openEntry().label;
    const previous = await call('remove_value', where);
    state.open = null;
    await refresh();
    toast(`${label} gelöscht`, async () => { await call('set_value', { ...where, value: previous ?? '' }); await refresh(); });
  },
  add() { state.adding = true; state.open = null; state.just = 'add'; },
  async addSave() {
    const key = $('newKey').value.trim();
    if (!key) { $('newKey').focus(); return; }
    const where = { file: state.file.path, path: [key] };
    await call('set_value', { ...where, value: $('newValue').value });
    state.adding = false;
    await refresh();
    toast(`${key} hinzugefügt`, async () => { await call('remove_value', where); await refresh(); });
  },
  async align(el) {
    await call('align', { file: el.dataset.file });
    await refresh();
    toast(`${base(el.dataset.file)} angeglichen`);
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
    toast(`${base(m.file)} zusammengeführt`);
  },
  async ask() { await call('copy_public_key'); toast('Dein Public Key ist kopiert. Schick ihn jemandem mit Zugriff.'); },
  async myKey() { await call('copy_public_key'); toast('Dein Public Key ist kopiert'); },
  async name(el) {
    const input = document.querySelector(`[data-name-for="${CSS.escape(el.dataset.recipient)}"]`);
    const name = input.value.trim();
    if (!name) { input.focus(); return; }
    await call('name_key', { recipient: el.dataset.recipient, name });
    await refresh();
    toast(`Der Key heißt jetzt ${name}`);
  },
  async toggleAccess(el) {
    const person = state.access.people.find((p) => p.recipient === el.dataset.recipient);
    const area = state.access.areas[Number(el.dataset.area)];
    const has = person.areas[Number(el.dataset.area)];
    if (!area.editable) { toast(`${area.name} kannst du nicht ändern. Dir fehlt dort der Zugriff.`); return; }
    if (person.me) { toast('Dich selbst kannst du nicht austragen'); return; }
    if (has) { state.sheet = { revoke: { person, area } }; return; }
    await call('grant', { recipient: person.recipient, area: area.index, name: person.name });
    await refresh();
    toast(`${nameOf(person)} kann jetzt ${area.name} lesen`);
  },
  async revokeSave() {
    const { person, area } = state.sheet.revoke;
    state.sheet = null;
    const rotated = await call('revoke', { recipient: person.recipient, area: area.index });
    await refresh();
    toast(`${nameOf(person)} kann ${area.name} nicht mehr lesen. ${plural(rotated, 'Datei', 'Dateien')} rotiert.`);
  },
  kick(el) { state.sheet = { kick: state.access.people.find((p) => p.recipient === el.dataset.recipient) }; },
  async kickSave() {
    const person = state.sheet.kick;
    state.sheet = null;
    for (const [i, area] of state.access.areas.entries()) {
      if (area.editable && person.areas[i]) await call('revoke', { recipient: person.recipient, area: area.index });
    }
    await refresh();
    toast(`${nameOf(person)} entfernt`);
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
    if (!name) return fail('Gib einen Namen ein.', 'inviteName');
    if (!/^age1[a-z0-9]{16,}$/.test(recipient)) return fail('Der Key muss mit age1 beginnen.', 'inviteKey');
    if (!areas.length) return fail('Wähle mindestens einen Bereich.');
    state.sheet = null;
    for (const area of areas) await call('grant', { recipient, area, name });
    await refresh();
    toast(`${name} ist dabei`);
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
    // Die Meldung steht schon unten. Die Ansicht bleibt, wie sie war.
  } finally {
    state.working = false;
    draw();
  }
}

document.addEventListener('click', (event) => {
  const el = event.target.closest('[data-act]');
  if (!el) return;
  if (el.classList.contains('scrim') && event.target !== el) return;
  run(el);
});

document.addEventListener('input', (event) => {
  if (event.target.id === 'query') $('hits').innerHTML = paletteItems(event.target.value);
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
    else if (state.editing || state.adding) { state.editing = false; state.adding = false; }
    else state.open = null;
    draw();
  } else if (event.key === 'Enter' && field) {
    if (field.id === 'edit') press('[data-act="save"]');
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
  } catch (e) { /* egal */ }
  try {
    state.keys = (await call('key_status')).publicKeys;
    const opened = await call('current_folder');
    const last = localStorage.getItem('gedoens.folder');
    if (opened) {
      state.folder = opened;
      state.overview = await call('overview');
      go('home');
    } else if (last && state.keys.length) {
      await openFolder(last);
    }
  } catch (e) {
    try { localStorage.removeItem('gedoens.folder'); } catch (e2) { /* egal */ }
  }
  draw();
})();
})();
