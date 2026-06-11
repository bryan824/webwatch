// catalog.js — the shared condition schema.
//
// THE POINT: in production this catalog is emitted by the backend so the web UI,
// the JSON API, and targets.toml all render from ONE definition and can't drift.
// Here it's the single source the builder renders every control from — change a
// subject's ops or fields and the whole form follows. No hand-kept parallel lists.
//
// A condition is modelled as (Locator -> Target/Subject -> Assertion) + negate,
// which collapses the legacy 9 wire "kinds" into 4 subjects x ops x a negate flag.

export const SUBJECTS = [
  {
    id: 'text',
    label: 'Page text',
    glyph: 'T',
    help: 'Look for a phrase anywhere on the page.',
    ops: [{ id: 'contains', label: 'contains' }],
    value: { label: 'Phrase', placeholder: 'Add to cart', kind: 'text' },
    locator: 'page', // always whole-page; no selector needed
  },
  {
    id: 'element',
    label: 'Element',
    glyph: '#',
    help: 'Check whether a specific element is present.',
    ops: [{ id: 'exists', label: 'is present' }],
    value: null,
    locator: 'required',
  },
  {
    id: 'value',
    label: 'Element value',
    glyph: '"',
    help: 'Read text from a region and test it. A broad locator is fine — we narrow with keyword or AI.',
    ops: [
      { id: 'contains', label: 'contains' },
      { id: 'equals', label: 'equals' },
      { id: 'changed', label: 'changes' },
    ],
    value: { label: 'Text', placeholder: 'On sale', kind: 'text', omitFor: ['changed'] },
    locator: 'optional',
  },
  {
    id: 'price',
    label: 'Price',
    glyph: '$',
    help: 'Read a price and compare it. Point at the price element for an exact read, or let us find it.',
    ops: [
      { id: 'below', label: 'drops below' },
      { id: 'above', label: 'rises above' },
      { id: 'changed', label: 'changes' },
    ],
    value: { label: 'Threshold', placeholder: '25.00', kind: 'money', prefix: '$', omitFor: ['changed'] },
    locator: 'optional',
  },
];

export const SUBJECT_BY_ID = Object.fromEntries(SUBJECTS.map((s) => [s.id, s]));

export function subject(id) {
  return SUBJECT_BY_ID[id];
}
export function opsFor(id) {
  return SUBJECT_BY_ID[id]?.ops ?? [];
}
export function opLabel(subjectId, opId) {
  return opsFor(subjectId).find((o) => o.id === opId)?.label ?? opId;
}
export function locatorMode(id) {
  return SUBJECT_BY_ID[id]?.locator ?? 'optional';
}
// The value field for a given (subject, op), or null when the op takes no value.
export function valueField(subjectId, op) {
  const v = SUBJECT_BY_ID[subjectId]?.value;
  if (!v) return null;
  if (v.omitFor?.includes(op)) return null;
  return v;
}

let seq = 0;
export function blankCondition(seed = {}) {
  seq += 1;
  return {
    cid: 'c' + seq + Math.random().toString(36).slice(2, 5),
    subject: 'text',
    op: 'contains',
    value: '',
    negate: false,
    locator: { type: 'page', query: '' },
    strategy: 'auto', // auto | exact | keyword | ai
    result: null,
    ...seed,
  };
}

// When the subject changes, keep the condition coherent (valid op + locator).
export function coerceForSubject(cond, subjectId) {
  const s = subject(subjectId);
  const op = s.ops.some((o) => o.id === cond.op) ? cond.op : s.ops[0].id;
  let locator = cond.locator;
  if (s.locator === 'page') locator = { type: 'page', query: '' };
  else if (locator.type === 'page') locator = { type: 'css', query: '' };
  return { ...cond, subject: subjectId, op, locator, result: null };
}

// Plain-English fragment for one condition (used in the summary line + chips).
export function describeCondition(c) {
  const where = locatorText(c.locator);
  let core;
  if (c.subject === 'text') core = `the page contains “${c.value || '…'}”`;
  else if (c.subject === 'element') core = `${where} is present`;
  else if (c.subject === 'value')
    core = c.op === 'changed' ? `${where} changes` : `${where} ${opLabel('value', c.op)} “${c.value || '…'}”`;
  else core = c.op === 'changed' ? `the price at ${where} changes` : `the price ${opLabel('price', c.op)} $${c.value || '…'}`;
  return c.negate ? flip(core) : core;
}

function flip(core) {
  // Turn the assertion into its "alert when false" reading for the summary.
  return core
    .replace('contains', 'no longer contains')
    .replace('is present', 'disappears')
    .replace('drops below', 'is not below')
    .replace('rises above', 'is not above')
    .replace('changes', "doesn't change")
    .replace('equals', "doesn't equal");
}

export function locatorText(locator) {
  if (!locator || locator.type === 'page') return 'the page';
  if (!locator.query) return locator.type === 'xpath' ? 'an xpath' : 'a selector';
  return locator.query;
}
