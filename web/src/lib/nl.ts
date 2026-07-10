import type { Condition } from './conditions';
import { blankCondition } from './conditions';

interface InterpretResult {
  conditions: Condition[];
  explanation: string;
  usedAi: boolean;
  confidence: number;
}

interface Intent {
  test: (t: string) => boolean;
  build: (t: string) => { cond: Condition; ai: boolean; note: string };
}

function priceFrom(t: string): string {
  const m = /\$?\s*(\d+(?:\.\d{1,2})?)/.exec(t);
  return m ? m[1] : '0';
}

const INTENTS: Intent[] = [
  {
    test: (t) => /(back\s*in\s*stock|in\s*stock|available again|restock|add to cart|buy now)/.test(t),
    build: () => ({
      cond: blankCondition({ subject: 'text', op: 'contains', value: 'Add to cart' }),
      ai: true,
      note: 'in-stock → looked for the buy control',
    }),
  },
  {
    test: (t) => /(sold\s*out).*(lift|gone|no longer|back|again)|(no longer|not).*(sold\s*out)/.test(t),
    build: () => ({
      cond: blankCondition({ subject: 'text', op: 'contains', value: 'Sold out', negate: true }),
      ai: true,
      note: '"sold out" lifted → alert when it disappears',
    }),
  },
  {
    test: (t) => /(on\s*sale|goes on sale|available for sale)/.test(t),
    build: () => ({
      cond: blankCondition({
        subject: 'value', op: 'contains', value: 'On sale',
        locator: { type: 'css', query: '.availability' },
      }),
      ai: true,
      note: 'read the availability element',
    }),
  },
  {
    test: (t) => /(under|below|less than|cheaper than|drops? to|<)\s*\$?\s*\d/.test(t),
    build: (t) => ({
      cond: blankCondition({
        subject: 'price', op: 'below', value: priceFrom(t),
        locator: { type: 'css', query: '.price' },
      }),
      ai: true,
      note: `inferred a price threshold of $${priceFrom(t)}`,
    }),
  },
  {
    test: (t) => /(over|above|more than|rises? to|>)\s*\$?\s*\d/.test(t),
    build: (t) => ({
      cond: blankCondition({
        subject: 'price', op: 'above', value: priceFrom(t),
        locator: { type: 'css', query: '.price' },
      }),
      ai: true,
      note: `inferred a price threshold of $${priceFrom(t)}`,
    }),
  },
  {
    test: (t) => /(price (change|drop|move)|drops?\b|cheaper)/.test(t),
    build: () => ({
      cond: blankCondition({
        subject: 'price', op: 'changed',
        locator: { type: 'css', query: '.price' },
      }),
      ai: true,
      note: 'watch the price for any change',
    }),
  },
  {
    test: (t) => /(description|listing|content|wording|copy|details?|anything)\s*(chang|updat|edit|different|move)/.test(t)
      || /(chang|updat)\w*\s*(description|listing|content|details?)/.test(t),
    build: () => ({
      cond: blankCondition({
        subject: 'value', op: 'changed',
        locator: { type: 'page', query: '' },
      }),
      ai: true,
      note: 'a meaningful content change — semantic, so it needs AI each check',
    }),
  },
];

function clauses(text: string): string[] {
  return text
    .split(/\b(?:and|&|;|,|\+| then )\b/i)
    .map((s) => s.trim())
    .filter(Boolean);
}

export function interpret(text: string): InterpretResult {
  const raw = (text || '').trim();
  if (!raw) return { conditions: [], explanation: '', usedAi: false, confidence: 0 };

  const conditions: Condition[] = [];
  let usedAi = false;
  const notes: string[] = [];

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

  if (conditions.length === 0) {
    const quoted = /["""]([^"""]{2,})["""]/.exec(raw);
    const phrase = quoted ? quoted[1] : raw.split(/\s+/).slice(0, 4).join(' ');
    conditions.push(blankCondition({ subject: 'text', op: 'contains', value: phrase }));
    notes.push(`couldn't recognise an intent — watching for the phrase "${phrase}"`);
    usedAi = true;
  }

  const n = conditions.length;
  const explanation = `Understood as ${n} rule${n > 1 ? 's' : ''}${notes.length ? ' — ' + notes.join('; ') : ''}.`;
  return { conditions, explanation, usedAi, confidence: usedAi ? 0.78 : 0.9 };
}
