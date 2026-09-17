// Panneau coutcouticket, lecture seule. Données : démon (/ui/api/…) ou maquette (window.MOCK).
'use strict';
const MOCK = window.MOCK || null;

class SessionExpired extends Error {}

async function get(url) {
  const r = await fetch(url, { credentials: 'same-origin', cache: 'no-store' });
  if (r.status === 401) throw new SessionExpired();
  const body = await r.json();
  if (!r.ok) throw new Error(body.error || ('erreur HTTP ' + r.status));
  return body;
}
const api = {
  overview: () => MOCK ? Promise.resolve(MOCK.overview) : get('api/overview'),
  detail: (t) => MOCK ? Promise.resolve(MOCK.details[t.project + '/' + t.id])
    : get('api/ticket?project=' + encodeURIComponent(t.project_path) + '&id=' + encodeURIComponent(t.id)),
};
const COLS = [['in-progress', 'En cours'], ['review', 'En revue'], ['blocked', 'Bloqué'], ['todo', 'À faire']];
const state = { data: null, project: null, status: null, prio: null, current: null, tab: 'ticket', events: null };
const $ = (id) => document.getElementById(id);
const esc = (s) => String(s).replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));

function setLive(ok, text) {
  $('dot').className = ok ? 'dot' : 'dot off';
  $('live').textContent = text;
}

// Session perdue (démon redémarré, cookie effacé) : le panneau ne peut plus rien lire.
function expired() {
  if (state.events) { state.events.close(); state.events = null; }
  setLive(false, 'session expirée : relancer « coutcouticket ui »');
}

function render() {
  const d = state.data;
  const shown = d.tickets.filter(t => (!state.project || t.project === state.project) && (!state.prio || t.priority === state.prio));
  const projects = [...new Set(d.tickets.map(t => t.project))];
  $('summary').textContent = `· ${shown.length} ticket(s) ouvert(s) · ${d.projects} projet(s)`;
  const f = $('filters');
  f.innerHTML = '';
  const btn = (label, on, click) => { const b = document.createElement('button'); b.textContent = label;
    b.setAttribute('aria-pressed', on); b.onclick = click; f.append(b); };
  const sep = () => f.append(Object.assign(document.createElement('span'), { className: 'sep' }));
  btn('Tous', !state.project, () => { state.project = null; render(); });
  projects.forEach(p => btn(p, state.project === p, () => { state.project = p; render(); }));
  sep();
  COLS.forEach(([s, label]) => btn(label, state.status === s, () => { state.status = state.status === s ? null : s; render(); }));
  sep();
  ['p0', 'p1', 'p2', 'p3'].forEach(p => btn(p, state.prio === p, () => { state.prio = state.prio === p ? null : p; render(); }));
  const w = $('warnings');
  w.hidden = !d.warnings.length;
  w.innerHTML = d.warnings.map(x => `<div>⚠️ <code>${esc(x.project_path)}</code> : ${esc(x.message)}</div>`).join('');
  const board = $('board');
  board.innerHTML = '';
  for (const [status, label] of COLS.filter(([s]) => !state.status || s === state.status)) {
    const list = shown.filter(t => t.status === status);
    const col = document.createElement('section');
    col.className = 'col';
    col.innerHTML = `<h2><span>${label}</span><span>${list.length}</span></h2><div class="cards"></div>`;
    const cards = col.querySelector('.cards');
    if (!list.length) cards.innerHTML = '<div class="empty">Aucun ticket</div>';
    for (const t of list) {
      const c = document.createElement('article');
      c.className = 'card ' + esc(t.priority);
      c.tabIndex = 0;
      c.innerHTML = `<div class="meta"><span class="id">${esc(t.id)}</span><span class="chip">${esc(t.project)}</span>
        <span class="chip">${esc(t.type)}</span><span>${esc(t.priority)}</span></div>
        <div class="title">${esc(t.title)}</div>
        ${t.open_blockers.length ? `<div class="blocked">⛔ bloqué par ${esc(t.open_blockers.join(', '))}</div>` : ''}`;
      c.onclick = () => open(t);
      c.onkeydown = (e) => { if (e.key === 'Enter') open(t); };
      cards.append(c);
    }
    board.append(col);
  }
}

// Markdown minimal : titres, listes, cases à cocher, gras, code. Tout le texte est échappé.
function md(src) {
  const inline = (s) => esc(s).replace(/`([^`]+)`/g, '<code>$1</code>').replace(/\*\*([^*]+)\*\*/g, '<b>$1</b>');
  let out = '', inList = false;
  for (const line of src.split('\n')) {
    const h = line.match(/^(#{1,3}) (.*)/), li = line.match(/^\s*[-*] (.*)/);
    if (!li && inList) { out += '</ul>'; inList = false; }
    if (h) out += `<h${h[1].length}>${inline(h[2])}</h${h[1].length}>`;
    else if (li) {
      if (!inList) { out += '<ul>'; inList = true; }
      const task = li[1].match(/^\[( |x)\] (.*)/);
      out += task ? `<li class="task">${task[1] === 'x' ? '☑' : '☐'} ${inline(task[2])}</li>` : `<li>${inline(li[1])}</li>`;
    } else if (line.trim()) out += `<p>${inline(line)}</p>`;
  }
  return out + (inList ? '</ul>' : '');
}

function showFacts(t, det) {
  const facts = $('d-facts');
  facts.innerHTML = [['Projet', t.project], ['Statut', t.status], ['Priorité', t.priority],
    ['Type', t.type], ['Branche', t.branch], ['Dépend de', t.blocked_by.join(', ') || '—']]
    .map(([k, v]) => `<span>${k}</span><b>${esc(v)}</b>`).join('');
  // Les navigateurs refusent les liens file:// depuis une page http : chemin à copier.
  if (det && det.ticket_file) {
    facts.insertAdjacentHTML('beforeend', `<span>Fichier</span><b class="path"><code>${esc(det.ticket_file)}</code>
      <button class="copy" type="button">Copier</button></b>`);
    const b = facts.querySelector('.copy');
    b.onclick = () => navigator.clipboard.writeText(det.ticket_file)
      .then(() => { b.textContent = 'Copié'; }, () => { b.textContent = 'Copie impossible'; });
  }
}

async function open(t, keepTab) {
  state.current = t;
  if (!keepTab) state.tab = 'ticket';
  $('d-title').textContent = `${t.id} · ${t.title}`;
  let det;
  try {
    det = await api.detail(t);
  } catch (e) {
    if (e instanceof SessionExpired) return expired();
    det = { ticket: `**Lecture impossible :** ${e.message}`, decisions: '', journal: '' };
  }
  if (state.current !== t) return;
  showFacts(t, det);
  const tabs = [['ticket', 'Ticket'], ['decisions', 'Décisions'], ['journal', 'Journal']];
  const show = (key) => {
    state.tab = key;
    $('d-doc').innerHTML = md(det[key] || '');
    [...$('d-tabs').children].forEach(b => b.setAttribute('aria-selected', b.dataset.k === key));
  };
  $('d-tabs').innerHTML = tabs.map(([k, l]) => `<button role="tab" data-k="${k}">${l}</button>`).join('');
  [...$('d-tabs').children].forEach(b => { b.onclick = () => show(b.dataset.k); });
  show(state.tab);
  $('drawer').classList.add('open'); $('scrim').classList.add('open');
  $('drawer').setAttribute('aria-hidden', 'false');
}
function close() {
  state.current = null;
  $('drawer').classList.remove('open'); $('scrim').classList.remove('open');
  $('drawer').setAttribute('aria-hidden', 'true');
}
$('d-close').onclick = close; $('scrim').onclick = close;
document.addEventListener('keydown', (e) => { if (e.key === 'Escape') close(); });

async function refresh() {
  try {
    state.data = await api.overview();
  } catch (e) {
    if (e instanceof SessionExpired) return expired();
    return setLive(false, 'lecture impossible : ' + e.message);
  }
  render();
  // Détail ouvert : relu avec les nouvelles données (le ticket a pu changer ou se fermer).
  const cur = state.current;
  if (cur) {
    const fresh = state.data.tickets.find(t => t.project_path === cur.project_path && t.id === cur.id);
    open(fresh || cur, true);
  }
}

refresh();
// Mise à jour poussée par le démon (SSE) : aucun sondage.
if (!MOCK) {
  const es = new EventSource('api/events');
  state.events = es;
  let lost = false;
  es.onmessage = refresh;
  es.onopen = () => { setLive(true, 'à jour'); if (lost) { lost = false; refresh(); } };
  es.onerror = () => {
    lost = true;
    setLive(false, 'démon injoignable, reconnexion…');
    // Le flux ne dit pas pourquoi il a échoué : une requête distingue la session expirée.
    if (es.readyState === EventSource.CLOSED) refresh();
  };
} else { setLive(true, 'maquette (données figées)'); }
