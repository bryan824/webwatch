// app.js — state (Solid store) + actions + composition + mount.

import { render, html, createStore, produce, onMount, onCleanup, Tooltip } from './vendor.js';
import { SEED_WATCHES, resolvePage } from './data.js';
import { blankCondition, coerceForSubject } from './catalog.js';
import { runCondition } from './engine.js';
import { interpret } from './nl.js';
import { Topbar, Rail } from './shell.js';
import { Builder } from './builder.js';

// JSON clone: robust across Solid store proxies (conditions are plain JSON data).
const clone = (x) => JSON.parse(JSON.stringify(x));
const slugify = (s) =>
  (s || '').toLowerCase().split(/[^a-z0-9]+/).filter(Boolean).join('-');

function newDraft() {
  return {
    id: 'new', name: '', url: '', page: null, pageKey: null,
    enabled: true, intervalSecs: 900,
    mode: 'describe', nl: '', nlResult: null,
    conditions: [], testing: false,
  };
}
function draftFromWatch(w) {
  return {
    id: w.id, name: w.name, url: w.url, page: resolvePage(w.url), pageKey: w.pageKey,
    enabled: w.enabled, intervalSecs: w.intervalSecs,
    mode: 'rules', nl: '', nlResult: null,
    conditions: clone(w.conditions).map((c) => ({ ...c, result: null })),
    testing: false,
  };
}
function watchState(d) {
  if (!d.enabled) return 'paused';
  const cs = d.conditions;
  if (cs.length && cs.every((c) => c.result && c.result.matched)) return 'matched';
  if (d.page && d.page.jsRendered) return 'browser';
  return 'watching';
}

const [state, setState] = createStore({
  watches: clone(SEED_WATCHES),
  editingId: null,
  draft: newDraft(),
  justSaved: false,
  confirmOpen: false,
});

// helper: mutate the condition with cid via produce
const mutateCond = (cid, fn) =>
  setState('draft', produce((d) => {
    const c = d.conditions.find((x) => x.cid === cid);
    if (c) fn(c, d);
  }));
const setResult = (cid, result) => mutateCond(cid, (c) => { c.result = result; });

const actions = {
  /* target */
  setUrl(url) {
    const page = resolvePage(url);
    setState('draft', produce((d) => {
      d.url = url; d.page = page; d.pageKey = page ? page.key : null;
      if (page && !d.name.trim()) d.name = page.title;
    }));
  },
  setName(name) { setState('draft', 'name', name); },

  /* describe (NL) */
  setNl(text) { setState('draft', 'nl', text); },
  runNl() {
    const { conditions, explanation, usedAi, confidence } = interpret(state.draft.nl);
    if (!conditions.length) return;
    setState('draft', produce((d) => {
      d.conditions = conditions;
      d.nlResult = { explanation, usedAi, confidence };
    }));
  },
  setMode(mode) { setState('draft', 'mode', mode); },
  editAsRule() { setState('draft', 'mode', 'rules'); },

  /* conditions */
  addCondition() { setState('draft', produce((d) => { d.conditions.push(blankCondition()); d.mode = 'rules'; })); },
  changeSubject(cid, subjectId) {
    setState('draft', produce((d) => {
      const i = d.conditions.findIndex((x) => x.cid === cid);
      if (i >= 0) d.conditions[i] = coerceForSubject(clone(d.conditions[i]), subjectId);
    }));
  },
  updateCondition(cid, patch) { mutateCond(cid, (c) => Object.assign(c, patch)); },
  setLocatorType(cid, type) {
    mutateCond(cid, (c) => {
      c.locator = { type, query: type === 'page' ? '' : c.locator.query };
      c.result = null;
    });
  },
  removeCondition(cid) { setState('draft', 'conditions', (cs) => cs.filter((c) => c.cid !== cid)); },

  /* dry-run */
  testAll() {
    const page = state.draft.page;
    if (!page || state.draft.testing || !state.draft.conditions.length) return;
    setState('draft', 'testing', true);
    const conds = state.draft.conditions.map((c) => c.cid);
    conds.forEach((cid) => setResult(cid, { state: 'running' }));
    conds.forEach((cid, i) => {
      setTimeout(() => {
        const c = state.draft.conditions.find((x) => x.cid === cid);
        if (c) setResult(cid, runCondition(page, c)); // runCondition only reads
        if (i === conds.length - 1) setState('draft', 'testing', false);
      }, 340 + i * 430);
    });
  },
  lockToElement(cid) {
    const c = state.draft.conditions.find((x) => x.cid === cid);
    const sel = c && c.result && c.result.lockTo;
    if (!sel) return;
    mutateCond(cid, (cc) => {
      cc.locator = { type: 'css', query: sel };
      cc.strategy = 'exact';
      cc.result = { state: 'running' };
    });
    setTimeout(() => {
      const cc = state.draft.conditions.find((x) => x.cid === cid);
      if (cc) setResult(cid, runCondition(state.draft.page, cc));
    }, 460);
  },

  /* schedule */
  setInterval(secs) { setState('draft', 'intervalSecs', secs); },
  setEnabled(on) { setState('draft', 'enabled', on); },

  /* lifecycle */
  newWatch() { setState({ editingId: null, draft: newDraft() }); },
  openWatch(id) {
    const w = state.watches.find((x) => x.id === id);
    if (w) setState({ editingId: id, draft: draftFromWatch(w) });
  },
  cancel() { actions.newWatch(); },
  askDelete() { setState('confirmOpen', true); },
  setConfirm(v) { setState('confirmOpen', v); },
  deleteWatch(id) {
    setState('confirmOpen', false);
    setState('watches', (ws) => ws.filter((w) => w.id !== id));
    actions.newWatch();
  },
  save() {
    const d = state.draft;
    if (!d.page || !d.conditions.length) return;
    const id = state.editingId || slugify(d.name) || 'watch-' + Date.now();
    const w = {
      id,
      name: d.name.trim() || d.page.title,
      url: d.url, pageKey: d.pageKey,
      enabled: d.enabled, intervalSecs: d.intervalSecs,
      conditions: clone(d.conditions).map((c) => ({ ...c, result: null })),
      state: watchState(d),
      lastCheck: 'just now',
      lastEvidence: d.conditions.map((c) => (c.result ? c.result.evidence : '')).filter(Boolean)[0] || 'not yet checked',
    };
    setState('watches', (ws) => {
      const i = ws.findIndex((x) => x.id === id);
      if (i >= 0) { const copy = ws.slice(); copy[i] = w; return copy; }
      return [w, ...ws];
    });
    setState('editingId', id);
    setState('draft', 'id', id);
    setState('justSaved', true);
    setTimeout(() => setState('justSaved', false), 1600);
  },
};

function App() {
  onMount(() => {
    const onKey = (e) => {
      if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
        const tag = (document.activeElement?.tagName || '').toLowerCase();
        if (tag === 'textarea') return; // textarea's ⌘↵ = Understand
        e.preventDefault();
        actions.testAll();
      }
    };
    window.addEventListener('keydown', onKey);
    onCleanup(() => window.removeEventListener('keydown', onKey));
  });

  return html`<div class="app">
    ${Topbar(state, actions)}
    <div class="main">
      ${Rail(state, actions)}
      <div class="pane">${Builder({ state, actions })}</div>
    </div>
  </div>`;
}

render(() => html`<${Tooltip.Provider}>${App()}<//>`, document.getElementById('root'));
