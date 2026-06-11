// nl.js — the natural-language front door (mocked interpreter).
//
// THE NORTH STAR: the user describes what they want to watch; we read the page and
// compile it into the SAME structured conditions a power user would author by hand.
// NL is a front door onto the rule model, not a parallel system — so the output is
// ordinary conditions the user can open in "Build rule" and tighten.
//
// In production this is one LLM call AT CONFIG TIME (cheap, once) that resolves the
// vague ask into concrete, deterministic rules. Here it's keyword heuristics over a
// few intents, tuned so the seeded pages tell a believable story. `usedAi` marks
// where a real model would have inferred a selector or meaning.

import { blankCondition } from './catalog.js';

// Each intent: a matcher over the lowercased text -> a condition (+ whether AI was "used").
const INTENTS = [
  {
    test: (t) => /(back\s*in\s*stock|in\s*stock|available again|restock|add to cart|buy now)/.test(t),
    build: () => ({ cond: blankCondition({ subject: 'text', op: 'contains', value: 'Add to cart' }), ai: true, note: 'in-stock → looked for the buy control' }),
  },
  {
    test: (t) => /(sold\s*out).*(lift|gone|no longer|back|again)|(no longer|not).*(sold\s*out)/.test(t),
    build: () => ({ cond: blankCondition({ subject: 'text', op: 'contains', value: 'Sold out', negate: true }), ai: true, note: '“sold out” lifted → alert when it disappears' }),
  },
  {
    test: (t) => /(on\s*sale|goes on sale|available for sale)/.test(t),
    build: () => ({ cond: blankCondition({ subject: 'value', op: 'contains', value: 'On sale', locator: { type: 'css', query: '.availability' } }), ai: true, note: 'read the availability element' }),
  },
  {
    // price under / below / less than $N
    test: (t) => /(under|below|less than|cheaper than|drops? to|<)\s*\$?\s*\d/.test(t),
    build: (t) => ({ cond: blankCondition({ subject: 'price', op: 'below', value: priceFrom(t), locator: { type: 'css', query: priceSelectorFor() } }), ai: true, note: `inferred a price threshold of $${priceFrom(t)}` }),
  },
  {
    test: (t) => /(over|above|more than|rises? to|>)\s*\$?\s*\d/.test(t),
    build: (t) => ({ cond: blankCondition({ subject: 'price', op: 'above', value: priceFrom(t), locator: { type: 'css', query: priceSelectorFor() } }), ai: true, note: `inferred a price threshold of $${priceFrom(t)}` }),
  },
  {
    test: (t) => /(price (change|drop|move)|drops?\b|cheaper)/.test(t),
    build: () => ({ cond: blankCondition({ subject: 'price', op: 'changed', locator: { type: 'css', query: priceSelectorFor() } }), ai: true, note: 'watch the price for any change' }),
  },
  {
    test: (t) => /(posted|new (listing|role|job|post)|opening|hiring|listing appears)/.test(t),
    build: () => ({ cond: blankCondition({ subject: 'element', op: 'exists', locator: { type: 'css', query: "a[href*='rust-engineer']" } }), ai: true, note: 'look for a matching listing link' }),
  },
  {
    // semantic "changes" — no element backs the meaning, so it needs AI on every check
    test: (t) => /(description|listing|content|wording|copy|details?|anything)\s*(chang|updat|edit|different|move)/.test(t) || /(chang|updat)\w*\s*(description|listing|content|details?)/.test(t),
    build: () => ({ cond: blankCondition({ subject: 'value', op: 'changed', locator: { type: 'page', query: '' } }), ai: true, note: 'a meaningful content change — semantic, so it needs AI each check' }),
  },
];

function priceFrom(t) {
  const m = /\$?\s*(\d+(?:\.\d{1,2})?)/.exec(t);
  return m ? m[1] : '0';
}
function priceSelectorFor() {
  // A real model would read the page; the seeded pages use these price hooks.
  return '.price';
}

// Split a request like "back in stock and under $25" into intent clauses.
function clauses(text) {
  return text
    .split(/\b(?:and|&|;|,|\+| then )\b/i)
    .map((s) => s.trim())
    .filter(Boolean);
}

export function interpret(text) {
  const raw = (text || '').trim();
  if (!raw) return { conditions: [], explanation: '', usedAi: false, confidence: 0 };

  const conditions = [];
  let usedAi = false;
  const notes = [];
  for (const clause of clauses(raw)) {
    const lc = clause.toLowerCase();
    const intent = INTENTS.find((i) => i.test(lc));
    if (intent) {
      const { cond, ai, note } = intent.build(lc);
      conditions.push(cond);
      usedAi = usedAi || ai;
      if (note) notes.push(note);
    }
  }

  // Fallback: nothing matched a known intent — take a quoted phrase or salient words.
  if (conditions.length === 0) {
    const quoted = /["“]([^"”]{2,})["”]/.exec(raw);
    const phrase = quoted ? quoted[1] : raw.split(/\s+/).slice(0, 4).join(' ');
    conditions.push(blankCondition({ subject: 'text', op: 'contains', value: phrase }));
    notes.push(`couldn't recognise an intent — watching for the phrase “${phrase}”`);
    usedAi = true;
  }

  const n = conditions.length;
  const explanation = `Understood as ${n} rule${n > 1 ? 's' : ''}${notes.length ? ' — ' + notes.join('; ') : ''}.`;
  return { conditions, explanation, usedAi, confidence: usedAi ? 0.78 : 0.9 };
}
