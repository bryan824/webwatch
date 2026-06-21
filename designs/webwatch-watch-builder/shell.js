// shell.js — app frame: top bar + the watch-list rail.

import { html, For, Show } from './vendor.js';
import { Icon, StatusDot } from './ui.js';

export function Topbar(state, actions) {
  const live = () => state.watches.filter((w) => w.enabled).length;
  const matched = () => state.watches.filter((w) => w.state === 'matched').length;
  return html`<header class="topbar">
    <div class="brand">
      <span class="brand__mark"><u></u><i></i></span>
      <span class="brand__name">web<b>watch</b></span>
    </div>
    <span class="topbar__spacer"></span>
    <div class="topbar__stat">
      <span class="dot matched" data-glow=""></span>
      <span class="mono">${() => matched()}</span> matched
      <span style="opacity:.4;margin:0 6px">·</span>
      <span class="dot watching" data-glow=""></span>
      <span class="mono">${() => live()}</span> live
    </div>
    <button class="btn primary sm" onClick=${() => actions.newWatch()} style="margin-left:14px">
      ${Icon('plus')} New watch
    </button>
  </header>`;
}

export function Rail(state, actions) {
  return html`<aside class="rail">
    <div class="rail__head">
      <span class="label">Watches</span>
      <span class="label mono">${() => state.watches.length}</span>
    </div>
    <div class="rail__list">
      <${For} each=${() => state.watches}>
        ${(w) => html`<button class="watch" aria-current=${() => (state.editingId === w.id).toString()}
          onClick=${() => actions.openWatch(w.id)}>
          ${() => StatusDot(w.state, { pulse: w.state === 'watching' })}
          <span class="watch__body">
            <span class="watch__name truncate">${() => w.name}</span>
            <span class="watch__url truncate">${() => w.url}</span>
          </span>
          <span class="watch__meta">${() => w.lastCheck}</span>
        </button>`}
      <//>
      <${Show} when=${() => state.watches.length === 0}>
        <div style="padding:24px 12px;text-align:center;color:var(--faint);font-size:12.5px">No watches yet.</div>
      <//>
    </div>
  </aside>`;
}
