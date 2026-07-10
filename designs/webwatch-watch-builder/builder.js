// builder.js — the Watch Builder. Operates on state.draft via actions from app.js.

import { html, For, Show } from './vendor.js';
import {
  SUBJECTS, opsFor, valueField, locatorMode, describeCondition,
} from './catalog.js';
import { PAGES } from './data.js';
import { conditionMonthlyCost, watchEconomics, usd } from './engine.js';
import {
  Icon, StatusDot, StrategyChip, EngineChip, ConfidenceText, WSwitch, WTabs, ConfirmDialog, WTooltip, NSelect,
} from './ui.js';

const INTERVALS = [
  { secs: 300, label: '5m' },
  { secs: 900, label: '15m' },
  { secs: 3600, label: '1h' },
  { secs: 21600, label: '6h' },
  { secs: 86400, label: 'daily' },
];
const EXAMPLES = [
  'tell me when the mug is back in stock and under $25',
  'alert when “Sold out” is lifted',
  'when the price drops below $400',
  'when the listing description changes',
];

const intervalLabel = (secs) => (INTERVALS.find((i) => i.secs === secs) || {}).label || secs + 's';

export function Builder({ state, actions }) {
  const draft = () => state.draft;
  const isEdit = () => !!state.editingId;

  const tested = () => draft().conditions.filter((c) => c.result && c.result.state !== 'running');
  const passing = () => tested().filter((c) => c.result.matched).length;

  return html`<div class="builder">
    <div>
      <div class="builder__title">${() => (isEdit() ? draft().name || 'Edit watch' : 'New watch')}</div>
      <div class="builder__sub">Point at a page, say what you want to know, and confirm what we'd alert on.</div>
    </div>

    ${targetBlock(state, actions)}
    ${whatToWatch(state, actions, tested, passing)}
    ${summaryBlock(state)}
    ${scheduleBlock(state, actions)}
    ${saveBar(state, actions, isEdit)}
    ${ConfirmDialog({
      open: () => state.confirmOpen,
      onOpenChange: (v) => actions.setConfirm(v),
      title: 'Delete this watch?',
      body: () => html`“${state.draft.name}” will stop being monitored. This can't be undone.`,
      confirmLabel: 'Delete watch',
      danger: true,
      onConfirm: () => actions.deleteWatch(state.editingId),
    })}
  </div>`;
}

/* ───────────────────────── TARGET ───────────────────────── */
function targetBlock(state, actions) {
  const draft = () => state.draft;
  return html`<div class="block">
    <div class="block__head"><span class="label">Target</span><span class="rule"></span></div>
    <div class="block__body" style="display:grid;gap:14px">
      <div class="urlrow">
        <div class="field">
          <span class="label">Page URL</span>
          <input class="input mono" placeholder="store.example.com/products/…" spellcheck="false"
            value=${() => draft().url} onInput=${(e) => actions.setUrl(e.currentTarget.value)} />
        </div>
        ${() =>
          draft().page
            ? (draft().page.jsRendered
                ? EngineChip('browser')
                : EngineChip('http'))
            : html`<span class="engine-badge http"><span class="dot"></span>no page</span>`}
      </div>
      <div class="field">
        <span class="label">Name</span>
        <input class="input" placeholder="Auto-suggested from the page"
          value=${() => draft().name} onInput=${(e) => actions.setName(e.currentTarget.value)} />
      </div>
      ${() => urlStatus(state, actions)}
    </div>
  </div>`;
}

function seededChips(actions) {
  return html`<div style="display:flex;gap:6px;flex-wrap:wrap;margin-top:7px">
    ${PAGES.map((p) => html`<button class="nl__chip mono" onClick=${() => actions.setUrl(p.url)}>${p.url}</button>`)}
  </div>`;
}

function urlStatus(state, actions) {
  const d = state.draft;
  if (d.page)
    return html`<div class="nl__hint"><span class="dot watching" data-glow=""></span>
      <span>Reading <b style="color:var(--text)">${d.page.title}</b> — ${d.page.jsRendered ? "JavaScript-rendered, we'll use the browser engine." : 'served as static HTML.'}</span></div>`;
  if (d.url.trim())
    return html`<div class="urlmiss">
      <span style="color:var(--warn)">${Icon('alert')}</span>
      <div><div>Couldn't reach <span class="mono">${d.url}</span>. In this prototype, pick a seeded page:</div>${seededChips(actions)}</div>
    </div>`;
  return html`<div class="nl__hint"><span>Point at a seeded page:</span>${seededChips(actions)}</div>`;
}

/* ───────────────────── WHAT TO WATCH ───────────────────── */
function whatToWatch(state, actions, tested, passing) {
  const draft = () => state.draft;
  return html`<div class="block">
    <div class="block__head">
      <span class="label">What to watch</span>
      <span class="rule"></span>
      ${() => {
        state.draft.mode; // re-create the uncontrolled Tabs when mode is changed programmatically
        return WTabs({
          value: state.draft.mode,
          onChange: (v) => actions.setMode(v),
          tabs: [
            { value: 'describe', label: 'Describe', icon: 'ai' },
            { value: 'rules', label: 'Build rules', icon: 'edit' },
          ],
        });
      }}
    </div>
    <div class="block__body">
      <${Show} when=${() => draft().mode === 'describe'}>
        ${() => describePane(state, actions)}
      <//>
      <${Show} when=${() => draft().mode === 'rules'}>
        ${() => rulesPane(state, actions)}
      <//>

      ${testBar(state, actions, tested, passing)}
    </div>
  </div>`;
}

function describePane(state, actions) {
  const draft = () => state.draft;
  return html`<div class="nl">
    <div class="nl__row">
      <textarea class="nl__input" placeholder="Tell me when… (e.g. the mug is back in stock and under $25)"
        spellcheck="false"
        value=${() => draft().nl}
        onInput=${(e) => actions.setNl(e.currentTarget.value)}
        onKeyDown=${(e) => { if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) { e.preventDefault(); actions.runNl(); } }}></textarea>
      <button class="btn ai" onClick=${() => actions.runNl()} title="⌘↵">
        ${Icon('ai')} Understand
      </button>
    </div>
    <div class="nl__hint">
      <span style="color:var(--faint)">examples</span>
      ${EXAMPLES.map((ex) => html`<button class="nl__chip" onClick=${() => { actions.setNl(ex); actions.runNl(); }}>${ex}</button>`)}
    </div>
    <${Show} when=${() => draft().nlResult}>
      ${() => html`<div class="understood">
        <span class="understood__icon">${Icon('ai')}</span>
        <span class="understood__text">${draft().nlResult.explanation}
          ${draft().nlResult.usedAi ? html` <span class="mono" style="color:var(--ai)">· AI resolved this at setup; checks stay deterministic.</span>` : null}</span>
      </div>`}
    <//>

    <${Show} when=${() => draft().conditions.length}>
      <div style="margin-top:14px;display:grid;gap:8px">
        <${For} each=${() => state.draft.conditions}>
          ${(c, i) => compiledRow(state, actions, c, i)}
        <//>
      </div>
    <//>
  </div>`;
}

// Read-only compiled condition (Describe mode) with its dry-run result.
function compiledRow(state, actions, c, i) {
  return html`<div class="cond" style="background:var(--surface)">
    <div class="cond__top" style="padding:11px 12px">
      <span class="cond__idx" style="padding-top:2px">${() => String(i() + 1).padStart(2, '0')}</span>
      <div class="cond__grid" style="align-items:center">
        <${Show} when=${() => c.result && c.result.state !== 'running'}
          fallback=${html`<span class="dot" style="--c:var(--faint)"></span>`}>
          ${() => verdictIcon(c.result)}
        <//>
        <span style="font-size:13px">${() => describeCondition(c)}</span>
      </div>
      <button class="btn ghost sm" onClick=${() => actions.editAsRule(c.cid)}>${Icon('edit')} edit as rule</button>
    </div>
    <${Show} when=${() => c.result}>${() => resultRow(state, actions, c)}<//>
  </div>`;
}

/* ───────────────────── Build rules pane ───────────────────── */
function rulesPane(state, actions) {
  return html`<div>
    <${For} each=${() => state.draft.conditions}>
      ${(c, i) => condCard(state, actions, c, i)}
    <//>
    <button class="add-cond" onClick=${() => actions.addCondition()}>${Icon('plus')} Add a condition — alerts fire when ALL match</button>
  </div>`;
}

function condCard(state, actions, c, i) {
  const subjectOpts = SUBJECTS.map((s) => ({ value: s.id, label: s.label }));
  const opOpts = () => opsFor(c.subject).map((o) => ({ value: o.id, label: o.label }));
  const vf = () => valueField(c.subject, c.op);
  const locMode = () => locatorMode(c.subject);

  return html`<div class="cond">
    <div class="cond__top">
      <span class="cond__idx">${() => String(i() + 1).padStart(2, '0')}</span>
      <div class="cond__grid">
        ${NSelect({ value: () => c.subject, onChange: (v) => actions.changeSubject(c.cid, v), options: subjectOpts, ariaLabel: 'Subject' })}
        ${() => NSelect({ value: () => c.op, onChange: (v) => actions.updateCondition(c.cid, { op: v, result: null }), options: opOpts(), ariaLabel: 'Operator' })}
        ${() => (vf() ? valueControl(c, vf(), actions) : null)}
        <div class="seg truth" role="group" aria-label="Alert when">
          <button data-on=${() => (!c.negate).toString()} data-truth="true" onClick=${() => actions.updateCondition(c.cid, { negate: false, result: null })}>TRUE</button>
          <button data-on=${() => (c.negate).toString()} data-truth="false" onClick=${() => actions.updateCondition(c.cid, { negate: true, result: null })}>FALSE</button>
        </div>
      </div>
      <button class="cond__remove" aria-label="Remove condition" onClick=${() => actions.removeCondition(c.cid)}>${Icon('trash')}</button>
    </div>

    <${Show} when=${() => locMode() !== 'page'}>
      ${() => locatorRow(c, locMode(), actions)}
    <//>

    <${Show} when=${() => c.result}>${() => resultRow(state, actions, c)}<//>
  </div>`;
}

function valueControl(c, vf, actions) {
  if (vf.kind === 'money') {
    return html`<span style="display:inline-flex;align-items:center;gap:0;background:var(--bg);border:1px solid var(--line-2);border-radius:var(--r)">
      <span class="mono" style="padding-left:10px;color:var(--faint)">$</span>
      <input class="input mono" style="width:92px;border:0;background:transparent" inputmode="decimal" placeholder=${vf.placeholder}
        value=${() => c.value} onInput=${(e) => actions.updateCondition(c.cid, { value: e.currentTarget.value, result: null })} />
    </span>`;
  }
  return html`<input class="input" style="width:200px;flex:1;min-width:120px" placeholder=${vf.placeholder}
    value=${() => c.value} onInput=${(e) => actions.updateCondition(c.cid, { value: e.currentTarget.value, result: null })} />`;
}

function locatorRow(c, mode, actions) {
  const types = mode === 'required' ? ['css', 'xpath'] : ['page', 'css', 'xpath'];
  const tlabel = { page: 'whole page', css: 'CSS', xpath: 'XPath' };
  return html`<div class="cond__advanced">
    <div class="adv-grid">
      <span class="label">Where to look${mode === 'optional' ? ' · optional' : ''}</span>
      <div style="display:flex;gap:8px;align-items:center;flex-wrap:wrap;min-width:0">
        <div class="seg" role="group" aria-label="Locator type">
          ${types.map((t) => html`<button data-on=${() => (c.locator.type === t).toString()}
            onClick=${() => actions.setLocatorType(c.cid, t)}>${tlabel[t]}</button>`)}
        </div>
        <${Show} when=${() => c.locator.type !== 'page'}>
          <input class="input mono" style="flex:1;min-width:160px" spellcheck="false"
            placeholder=${() => (c.locator.type === 'xpath' ? "//span[@class='price']" : '.price')}
            value=${() => c.locator.query} onInput=${(e) => actions.updateCondition(c.cid, { locator: { ...c.locator, query: e.currentTarget.value }, result: null })} />
        <//>
      </div>
      <span class="label">How</span>
      <div style="display:flex;gap:8px;align-items:center;flex-wrap:wrap">
        <div class="seg" role="group" aria-label="Extraction strategy">
          ${['auto', 'exact', 'keyword', 'ai'].map((s) => html`<button data-on=${() => (c.strategy === s).toString()}
            onClick=${() => actions.updateCondition(c.cid, { strategy: s, result: null })}>${s}</button>`)}
        </div>
        ${WTooltip(
          html`<span><b>auto</b> tries exact → keyword → AI. A precise locator reads the element verbatim (free, 100%). A broad one falls back to keyword, then AI. Lock fuzzy matches to an element so every check stays free.</span>`,
          html`<span class="ttip-trigger" style="color:var(--faint)">${Icon('alert', '')}</span>`
        )}
      </div>
    </div>
  </div>`;
}

/* ───────────────────── dry-run result row ───────────────────── */
function verdictIcon(r) {
  if (!r) return null;
  if (r.state === 'pass') return html`<span class="res__icon" style="color:var(--ok)">${Icon('check')}</span>`;
  if (r.state === 'fail') return html`<span class="res__icon" style="color:var(--bad)">${Icon('x')}</span>`;
  return html`<span class="res__icon" style="color:var(--warn)">${Icon('alert')}</span>`;
}

function resultRow(state, actions, c) {
  return html`<div class=${() => 'cond__result ' + (c.result.state === 'running' ? 'running' : '')}>
    <${Show} when=${() => c.result.state !== 'running'}
      fallback=${html`<span class="res__icon" style="color:var(--accent)">${Icon('search')}</span>`}>
      ${() => verdictIcon(c.result)}
    <//>
    <div class="res__main">
      <div class="res__line">
        <span class="res__verdict" style=${() => `color:${c.result.matched ? 'var(--ok)' : c.result.state === 'running' ? 'var(--accent)' : 'var(--bad)'}`}>
          ${() => (c.result.state === 'running' ? 'checking…' : c.result.matched ? 'would alert' : "wouldn't alert")}
        </span>
        <${Show} when=${() => c.result.state !== 'running'}>
          ${() => StrategyChip(c.result.strategy)}
          ${() => EngineChip(c.result.engine)}
          ${() => ConfidenceText(c.result.confidence)}
        <//>
      </div>
      <${Show} when=${() => c.result.state !== 'running'}>
        <div class="res__evidence">${() => c.result.evidence}</div>
        <${Show} when=${() => c.result.engine === 'browser'}>
          <div class="res__note info">${Icon('globe')} HTTP couldn't prove this — escalated to the browser engine.</div>
        <//>
        ${() => costNote(state, actions, c)}
      <//>
    </div>
  </div>`;
}

// Per-condition economics: exact/keyword run free forever; AI costs per check.
function costNote(state, actions, c) {
  const r = c.result;
  const perMo = usd(conditionMonthlyCost(state.draft.intervalSecs));
  if (r.strategy === 'ai' && r.lockTo) {
    return html`<div class="locknote">
      ${Icon('lock')}
      <span>Found via AI — ≈<b style="color:var(--text)">${perMo}/mo</b> at this interval. Lock to
        <span class="mono" style="color:var(--text)">${r.lockTo}</span> to make every check
        <b style="color:var(--ok)">free</b>.</span>
      <button class="btn sm" style="margin-left:auto" onClick=${() => actions.lockToElement(c.cid)}>Lock to element</button>
    </div>`;
  }
  if (r.strategy === 'ai') {
    return html`<div class="res__note warn">${Icon('ai')}
      <span>Runs AI on <b>every check</b> — ≈${perMo}/mo. Semantic check with nothing to pin to; narrow the locator to make it cheaper.</span></div>`;
  }
  if (r.strategy === 'keyword' && r.lockTo) {
    return html`<div class="locknote">
      ${Icon('lock')}
      <span>Matched by keyword (free). Lock to <span class="mono" style="color:var(--text)">${r.lockTo}</span> for an exact, more precise read.</span>
      <button class="btn sm" style="margin-left:auto" onClick=${() => actions.lockToElement(c.cid)}>Lock to element</button>
    </div>`;
  }
  return html`<div class="res__note free">${Icon('check')} Free on every check — deterministic ${() => r.strategy}.</div>`;
}

/* ───────────────────── test bar ───────────────────── */
function testBar(state, actions, tested, passing) {
  return html`<div style="margin-top:16px;padding-top:14px;border-top:1px solid var(--line);display:grid;gap:11px">
    <div style="display:flex;gap:12px;align-items:center">
      <button class="btn primary" onClick=${() => actions.testAll()} disabled=${() => state.draft.testing || !state.draft.page}>
        ${() => (state.draft.testing ? html`${Icon('search')} checking…` : html`${Icon('bolt')} Test against live page`)}
      </button>
      <${Show} when=${() => tested().length}>
        ${() => html`<span class="mono" style="font-size:12.5px;color:var(--muted)">
          <b style=${`color:${passing() === tested().length ? 'var(--ok)' : 'var(--warn)'}`}>${passing()}</b> / ${tested().length} would alert
        </span>`}
      <//>
      <span style="flex:1"></span>
      <span class="nl__hint" style="margin:0"><span class="kbd">⌘↵</span> test</span>
    </div>
    <${Show} when=${() => tested().length}>${() => economicsBar(state)}<//>
  </div>`;
}

function economicsBar(state) {
  const e = watchEconomics(state.draft.conditions, state.draft.intervalSecs);
  if (e.free)
    return html`<div class="costbar free">${Icon('check')}
      <span>Runs <b>free</b> — every check is deterministic, at any interval.</span></div>`;
  return html`<div class="costbar warn">${Icon('ai')}
    <span>Uses <b>${e.aiCount}</b> AI call${e.aiCount > 1 ? 's' : ''} every check · ≈<b>${usd(e.monthly)}/mo</b>${' at '}${intervalLabel(state.draft.intervalSecs)}${e.lockable ? html` · <span style="color:var(--ok)">${e.lockable} can be made free by locking to an element</span>` : ''}.</span></div>`;
}

/* ───────────────────── summary ───────────────────── */
function summaryBlock(state) {
  const text = () => {
    const cs = state.draft.conditions;
    if (!cs.length) return null;
    return cs.map(describeCondition).join('  ·  AND  ·  ');
  };
  return html`<${Show} when=${() => state.draft.conditions.length}>
    <div class="block"><div class="summary">
      <span style="color:var(--accent);margin-top:1px">${Icon('target')}</span>
      <span class="summary__txt">Alert me when <em>all</em> of: ${() => text()}</span>
    </div></div>
  <//>`;
}

/* ───────────────────── schedule ───────────────────── */
function scheduleBlock(state, actions) {
  return html`<div class="block">
    <div class="block__head"><span class="label">Schedule</span><span class="rule"></span></div>
    <div class="block__body sched">
      <span class="label" style="margin-right:2px">Check every</span>
      <div class="presets">
        ${INTERVALS.map((iv) => html`<button class="preset" data-on=${() => (state.draft.intervalSecs === iv.secs).toString()}
          onClick=${() => actions.setInterval(iv.secs)}>${iv.label}</button>`)}
      </div>
      <span style="flex:1"></span>
      ${() => html`<div data-enabled-key=${state.draft.id}>${WSwitch({
        checked: state.draft.enabled,
        onChange: (v) => actions.setEnabled(v),
        label: state.draft.enabled ? 'enabled' : 'paused',
      })}</div>`}
    </div>
  </div>`;
}

/* ───────────────────── save bar ───────────────────── */
function saveBar(state, actions, isEdit) {
  return html`<div class="savebar">
    <${Show} when=${() => isEdit()}>
      <button class="as-trigger" onClick=${() => actions.askDelete()}>${Icon('trash')} Delete</button>
    <//>
    <span class="savebar__spacer"></span>
    <button class="btn ghost" onClick=${() => actions.cancel()}>Cancel</button>
    <button class="btn primary" onClick=${() => actions.save()} disabled=${() => !state.draft.page || !state.draft.conditions.length}>
      ${() => (state.justSaved ? html`${Icon('check')} Saved` : isEdit() ? 'Save changes' : 'Create watch')}
    </button>
  </div>`;
}
