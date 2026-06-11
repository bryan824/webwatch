// engine.js — the dry-run check engine (mocked, deterministic).
//
// This is where the graceful-degradation extraction ladder lives:
//   exact   — the locator resolves to one element; take its content verbatim (100%)
//   keyword — a broad locator (or whole page); regex/substring within that scope
//   ai      — nothing resolved, or a semantic ask; "understand" the scope
//
// The key product idea: prefer to resolve to an EXACT locator at config time so
// every future poll is cheap + deterministic. So a keyword/ai match always reports
// a `lockTo` selector the builder can promote to (turning a fuzzy match free).

const PRICE_RE = /\$\s*([0-9]{1,3}(?:,[0-9]{3})*|[0-9]+)(?:\.([0-9]{1,2}))?/;

export function firstPriceCents(text) {
  const m = PRICE_RE.exec(text || '');
  if (!m) return null;
  const dollars = parseInt(m[1].replace(/,/g, ''), 10);
  const cents = m[2] ? parseInt((m[2] + '00').slice(0, 2), 10) : 0;
  return dollars * 100 + cents;
}
export const money = (cents) => '$' + (cents / 100).toFixed(2);

// --- cost model -----------------------------------------------------------
// AI runs PER CHECK. A monitor polls forever, so resolving to exact/keyword at
// config time makes every future check free; AI-every-check is the expensive path.
export const AI_COST_PER_CALL = 0.003; // mock price of one small-model call
export const checksPerDay = (intervalSecs) => 86400 / Math.max(intervalSecs || 1, 1);
export const conditionMonthlyCost = (intervalSecs) => checksPerDay(intervalSecs) * 30 * AI_COST_PER_CALL;
export const usd = (n) => '$' + n.toFixed(2);
export function watchEconomics(conditions, intervalSecs) {
  const ai = conditions.filter((c) => c.result && c.result.strategy === 'ai');
  return {
    aiCount: ai.length,
    lockable: ai.filter((c) => c.result.lockTo).length,
    monthly: ai.length * conditionMonthlyCost(intervalSecs),
    free: ai.length === 0,
  };
}

// Resolve a locator against a page. Returns { kind, scope, el, els }.
function resolveLocator(page, locator) {
  if (!locator || locator.type === 'page') {
    return { kind: 'page', scope: page.text, els: page.elements };
  }
  const q = (locator.query || '').trim();
  if (!q) return { kind: 'unset', scope: '', els: [] };
  const match = (e) => (locator.type === 'xpath' ? e.xpath === q : e.selector === q);
  const exact = page.elements.find(match);
  if (exact) return { kind: 'exact', scope: exact.text, el: exact, els: [exact] };
  // Broad: the query names a container that elements live within.
  const broad = page.elements.filter((e) => (e.within || []).includes(q));
  if (broad.length) return { kind: 'broad', scope: broad.map((e) => e.text).join(' · '), els: broad };
  return { kind: 'none', scope: '', els: [] };
}

// Decide which extraction strategy actually fired, honoring an explicit override.
function pickStrategy(requested, locKind, semantic) {
  if (requested && requested !== 'auto') return requested;
  if (locKind === 'exact') return 'exact';
  if (semantic) return 'ai';
  if (locKind === 'none' || locKind === 'unset') return 'ai';
  return 'keyword';
}

const CONF = { exact: 1.0, keyword: 0.86, ai: 0.71 };

// Run one condition against a page. Pure + synchronous; the UI adds latency.
export function runCondition(page, c) {
  if (!page) {
    return result(false, 'error', 'no page loaded', null, 'http', 'ai', null);
  }
  const loc = resolveLocator(page, c.locator);
  const engine = page.jsRendered ? 'browser' : 'http';

  // --- element is present -------------------------------------------------
  if (c.subject === 'element') {
    const present = loc.kind === 'exact' || loc.kind === 'broad';
    const strat = loc.kind === 'exact' ? 'exact' : present ? 'keyword' : 'ai';
    const base = present;
    const ev = present
      ? `${c.locator.query} matched ${loc.els.length} element(s)`
      : `${c.locator.query || 'selector'} matched nothing`;
    return finalize(c, base, strat, ev, present ? loc.els[0]?.selector : null, engine, present ? loc.els[0]?.text : null);
  }

  // --- price below / above / changes -------------------------------------
  if (c.subject === 'price') {
    const priceEl = loc.el?.price ? loc.el : loc.els?.find((e) => e.price);
    const scope = priceEl ? priceEl.text : loc.scope || page.text;
    const cents = firstPriceCents(scope);
    const semantic = false;
    const strat = pickStrategy(c.strategy, priceEl && loc.kind === 'exact' ? 'exact' : loc.kind, semantic);
    if (cents == null) return finalize(c, false, strat, 'no price found in scope', null, engine, null);
    let base, ev;
    if (c.op === 'changed') {
      base = true; // mock: treat any observed price as "could change" baseline set
      ev = `observed ${money(cents)} (baseline recorded)`;
    } else {
      const threshold = Math.round(parseFloat(c.value || '0') * 100);
      base = c.op === 'below' ? cents < threshold : cents > threshold;
      ev = `observed ${money(cents)} ${cents < threshold ? '<' : '≥'} ${money(threshold)}`;
    }
    const lockTo = priceEl && loc.kind !== 'exact' ? priceEl.selector : null;
    return finalize(c, base, strat, ev, lockTo, engine, money(cents));
  }

  // --- text / value contains | equals | changes --------------------------
  const scope = c.subject === 'text' ? page.text : loc.scope;
  const semantic = c.op === 'changed' || (c.subject === 'value' && loc.kind === 'none');
  const strat = pickStrategy(c.strategy, c.subject === 'text' ? 'keyword' : loc.kind, semantic);

  if (c.subject === 'value' && loc.kind === 'unset')
    return finalize(c, false, 'ai', 'no locator set', null, engine, null);

  let base, ev, observed = null;
  const needle = (c.value || '').toLowerCase();
  if (c.op === 'equals') {
    base = scope.trim().toLowerCase() === needle;
    observed = scope.trim();
    ev = `value is “${truncate(scope)}”`;
  } else if (c.op === 'changed') {
    base = true;
    observed = truncate(scope);
    ev = `current value “${truncate(scope)}” (baseline recorded)`;
  } else {
    // contains
    base = scope.toLowerCase().includes(needle);
    const where = c.subject === 'text' ? 'page text' : c.locator.query || 'scope';
    if (base) {
      observed = snippet(scope, c.value);
      ev = `${where} contains “${c.value}” — “${observed}”`;
    } else {
      ev = `${where} does not contain “${c.value}”`;
    }
  }
  // Find a concrete element to lock onto when this was fuzzy.
  let lockTo = null;
  if (strat !== 'exact') {
    const backing = (loc.els || page.elements).find((e) => e.text.toLowerCase().includes(needle) && needle);
    lockTo = backing?.selector ?? null;
  }
  return finalize(c, base, strat, ev, lockTo, engine, observed);
}

function finalize(c, base, strategy, evidence, lockTo, engine, observed) {
  const matched = c.negate ? !base : base;
  return result(matched, matched ? 'pass' : 'fail', evidence, observed, engine, strategy, lockTo, base);
}
function result(matched, state, evidence, observed, engine, strategy, lockTo, base = matched) {
  return {
    state, // pass | fail | error | running | idle
    matched,
    base,
    evidence,
    observed,
    engine, // http | browser
    strategy, // exact | keyword | ai
    confidence: CONF[strategy] ?? 0.7,
    lockTo, // selector to promote to, or null
    ranAt: Date.now(),
  };
}

function truncate(s, n = 48) {
  s = (s || '').trim();
  return s.length > n ? s.slice(0, n - 1) + '…' : s;
}
function snippet(haystack, needle) {
  if (!needle) return truncate(haystack);
  const i = haystack.toLowerCase().indexOf(needle.toLowerCase());
  if (i < 0) return truncate(haystack);
  const start = Math.max(0, i - 12);
  return (start > 0 ? '…' : '') + haystack.slice(start, i + needle.length + 12).trim() + '…';
}
