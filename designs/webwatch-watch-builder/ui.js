// ui.js — presentational bits + thin Base UI wrappers, styled by styles.css.

import { html, createEffect, Switch, Tabs, Tooltip, AlertDialog } from './vendor.js';

/* ---- icons (inline SVG, stroke = currentColor) ---- */
const PATHS = {
  check: '<path d="M3.5 8.5l3 3 6-7"/>',
  x: '<path d="M4 4l8 8M12 4l-8 8"/>',
  alert: '<path d="M8 2.5l6 11H2l6-11zM8 7v3M8 11.6v.01"/>',
  ai: '<path d="M8 2.5l1.4 3.6L13 7.5l-3.6 1.4L8 12.5 6.6 8.9 3 7.5l3.6-1.4L8 2.5z"/>',
  lock: '<rect x="3.5" y="7.5" width="9" height="6" rx="1"/><path d="M5.5 7.5V6a2.5 2.5 0 015 0v1.5"/>',
  trash: '<path d="M3.5 4.5h9M6.5 4.5V3.5a1 1 0 011-1h1a1 1 0 011 1v1M5 4.5l.5 8a1 1 0 001 .9h3a1 1 0 001-.9l.5-8"/>',
  plus: '<path d="M8 3.5v9M3.5 8h9"/>',
  chevron: '<path d="M4 6l4 4 4-4"/>',
  bolt: '<path d="M8.5 2L4 9h3l-.5 5L11 7H8l.5-5z"/>',
  clock: '<circle cx="8" cy="8" r="5.5"/><path d="M8 5v3l2 1.3"/>',
  target: '<circle cx="8" cy="8" r="5.5"/><circle cx="8" cy="8" r="2"/>',
  search: '<circle cx="7" cy="7" r="3.8"/><path d="M10 10l3 3"/>',
  pause: '<path d="M6 4v8M10 4v8"/>',
  globe: '<circle cx="8" cy="8" r="5.5"/><path d="M2.5 8h11M8 2.5c1.8 2 1.8 9 0 11M8 2.5c-1.8 2-1.8 9 0 11"/>',
  edit: '<path d="M10.5 3l2.5 2.5L6 12.5 3 13l.5-3L10.5 3z"/>',
};
export function Icon(name, cls = '') {
  return html`<svg class=${'ic ' + cls} width="16" height="16" viewBox="0 0 16 16" fill="none"
    stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"
    innerHTML=${PATHS[name] || ''}></svg>`;
}

/* ---- status light ---- */
export function StatusDot(stateClass, opts = {}) {
  const cls = `dot ${stateClass}` + (opts.pulse ? ' pulse' : '');
  return html`<span class=${cls} data-glow=${stateClass !== 'paused' ? '' : null}></span>`;
}

/* ---- chips ---- */
const STRAT = {
  exact: { cls: 'exact', label: 'exact', tip: 'The locator resolves to one element — we read it verbatim. 100% deterministic and free on every check.' },
  keyword: { cls: 'keyword', label: 'keyword', tip: 'Matched by keyword/regex within the scope. Deterministic and free, but a tighter locator would be more precise.' },
  ai: { cls: 'ai', label: 'AI', tip: 'Resolved by understanding the page. Great for setup — but if this ran on every poll it would cost an AI call each time. Lock it to an element to make checks free.' },
};
export function StrategyChip(strategy) {
  const s = STRAT[strategy] || STRAT.keyword;
  return WTooltip(
    html`<span class="mono">${s.tip}</span>`,
    html`<span class=${'chip ' + s.cls}><span class="dot" data-glow=""></span>${s.label}</span>`
  );
}
export function EngineChip(engine) {
  const browser = engine === 'browser';
  return WTooltip(
    browser
      ? html`This page renders with JavaScript, so we fetch it through the headless <span class="mono">browser</span> engine.`
      : html`Fetched with a plain <span class="mono">http</span> request — cheap and fast.`,
    html`<span class=${'chip ' + (browser ? 'browser' : 'http')}><span class="dot" data-glow=""></span>${engine}</span>`
  );
}
export function ConfidenceText(c) {
  return html`<span class="mono" style="color:var(--faint);font-size:10.5px">${Math.round(c * 100)}%</span>`;
}

/* ---- Base UI: Switch (uncontrolled; parent remounts to reset) ---- */
export function WSwitch({ checked, onChange, label }) {
  return html`<label class="switch-row">
    <${Switch.Root} class="ww-switch" defaultChecked=${checked} onCheckedChange=${onChange}>
      <${Switch.Thumb} class="ww-switch__thumb" />
    <//>
    ${label ? html`<span>${label}</span>` : null}
  </label>`;
}

/* ---- Base UI: Tabs (segmented control; content rendered by caller) ---- */
export function WTabs({ value, onChange, tabs }) {
  // NOTE: an interpolated ARRAY of Base UI parts loses the Root context in
  // solid-js/html — wrap it in a function so it renders inside Tabs.List.
  return html`<${Tabs.Root} defaultValue=${value} onValueChange=${onChange}>
    <${Tabs.List} class="ww-tabs-list">
      ${() => tabs.map(
        (t) => html`<${Tabs.Tab} class="ww-tab" value=${t.value}>
          ${t.icon ? Icon(t.icon) : null}${t.label}
        <//>`
      )}
    <//>
  <//>`;
}

/* ---- Base UI: Tooltip ---- */
export function WTooltip(content, children, side = 'top') {
  return html`<${Tooltip.Root} delay=${160} closeDelay=${60}>
    <${Tooltip.Trigger} class="ttip-trigger">${children}<//>
    <${Tooltip.Portal}>
      <${Tooltip.Positioner} side=${side} sideOffset=${8}>
        <${Tooltip.Popup} class="ww-tooltip">${content}<//>
      <//>
    <//>
  <//>`;
}

/* ---- Base UI: AlertDialog (controlled from the store) ----
   `open` is an accessor; Portal children are lazy so they mount on open. */
export function ConfirmDialog({ open, onOpenChange, title, body, confirmLabel, onConfirm, danger }) {
  return html`<${AlertDialog.Root} open=${open} onOpenChange=${onOpenChange}>
    <${AlertDialog.Portal}>
      ${() => html`<${AlertDialog.Backdrop} class="ww-backdrop" />`}
      ${() => html`<${AlertDialog.Popup} class="ww-dialog">${() => html`
        <${AlertDialog.Title} class="dlg-title">${title}<//>
        <${AlertDialog.Description} class="dlg-desc">${body}<//>
        <div class="ww-dialog__actions">
          <button class="btn ghost" onClick=${() => onOpenChange(false)}>Cancel</button>
          <button class=${'btn ' + (danger ? 'danger' : 'primary')}
            onClick=${() => { onConfirm(); onOpenChange(false); }}>${confirmLabel}</button>
        </div>
      `}<//>`}
    <//>
  <//>`;
}

/* ---- native styled <select> (controlled) ----
   Solid's `value=` on <select> doesn't bind reliably with dynamically-rendered
   options, so set the property via a ref + effect after the options mount. */
export function NSelect({ value, onChange, options, ariaLabel }) {
  const read = () => (typeof value === 'function' ? value() : value);
  return html`<select class="nselect" aria-label=${ariaLabel || ''}
    ref=${(el) => createEffect(() => { const v = read(); if (el.value !== v) el.value = v; })}
    onChange=${(e) => onChange(e.currentTarget.value)}>
    ${options.map((o) => html`<option value=${o.value}>${o.label}</option>`)}
  </select>`;
}
