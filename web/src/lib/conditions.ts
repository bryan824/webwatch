import type { ConditionInput, ConditionWireKind } from './types';

export type Subject = 'text' | 'element' | 'value' | 'price';
export type LocatorType = 'page' | 'css' | 'xpath';
export type Strategy = 'auto' | 'exact' | 'keyword' | 'ai';

export interface OpSpec {
  id: string;
  label: string;
}

export interface ValueSpec {
  label: string;
  placeholder: string;
  kind: 'text' | 'money';
  omitFor?: string[];
}

export interface SubjectDef {
  id: Subject;
  label: string;
  glyph: string;
  help: string;
  ops: OpSpec[];
  value: ValueSpec | null;
  locator: 'page' | 'required' | 'optional';
}

export const SUBJECTS: SubjectDef[] = [
  {
    id: 'text',
    label: 'Page text',
    glyph: 'T',
    help: 'Look for a phrase anywhere on the page.',
    ops: [{ id: 'contains', label: 'contains' }],
    value: { label: 'Phrase', placeholder: 'Add to cart', kind: 'text' },
    locator: 'page',
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
    help: 'Read text from a region and test it.',
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
    help: 'Read a price and compare it.',
    ops: [
      { id: 'below', label: 'drops below' },
      { id: 'above', label: 'rises above' },
      { id: 'changed', label: 'changes' },
    ],
    value: { label: 'Threshold', placeholder: '25.00', kind: 'money', omitFor: ['changed'] },
    locator: 'optional',
  },
];

const SUBJECT_BY_ID = Object.fromEntries(SUBJECTS.map((s) => [s.id, s]));

export function subjectDef(id: Subject): SubjectDef {
  return SUBJECT_BY_ID[id]!;
}

export function opsFor(id: Subject): OpSpec[] {
  return SUBJECT_BY_ID[id]?.ops ?? [];
}

export function opLabel(subjectId: Subject, opId: string): string {
  return opsFor(subjectId).find((o) => o.id === opId)?.label ?? opId;
}

export function locatorMode(id: Subject): 'page' | 'required' | 'optional' {
  return SUBJECT_BY_ID[id]?.locator ?? 'optional';
}

export function valueField(subjectId: Subject, op: string): ValueSpec | null {
  const v = SUBJECT_BY_ID[subjectId]?.value ?? null;
  if (!v) return null;
  if (v.omitFor?.includes(op)) return null;
  return v;
}

export interface Locator {
  type: LocatorType;
  query: string;
}

export interface Condition {
  cid: string;
  subject: Subject;
  op: string;
  value: string;
  negate: boolean;
  locator: Locator;
  strategy: Strategy;
}

let cidSeq = 0;
export function blankCondition(seed: Partial<Condition> = {}): Condition {
  cidSeq += 1;
  return {
    cid: 'c' + cidSeq + Math.random().toString(36).slice(2, 5),
    subject: 'text',
    op: 'contains',
    value: '',
    negate: false,
    locator: { type: 'page', query: '' },
    strategy: 'auto',
    ...seed,
  };
}

export function coerceForSubject(cond: Condition, subjectId: Subject): Condition {
  const s = subjectDef(subjectId);
  const op = s.ops.some((o) => o.id === cond.op) ? cond.op : s.ops[0].id;
  let locator = { ...cond.locator };
  if (s.locator === 'page') locator = { type: 'page', query: '' };
  else if (locator.type === 'page') locator = { type: 'css', query: '' };
  return { ...cond, subject: subjectId, op, locator, strategy: 'auto' };
}

export function describeCondition(c: Condition): string {
  const where = locatorText(c.locator);
  let core: string;
  if (c.subject === 'text') core = `the page contains "${c.value || '…'}"`;
  else if (c.subject === 'element') core = `${where} is present`;
  else if (c.subject === 'value')
    core = c.op === 'changed' ? `${where} changes` : `${where} ${opLabel('value', c.op)} "${c.value || '…'}"`;
  else
    core = c.op === 'changed' ? `the price at ${where} changes` : `the price ${opLabel('price', c.op)} $${c.value || '…'}`;
  return c.negate ? flipDescription(core) : core;
}

function flipDescription(core: string): string {
  return core
    .replace('contains', 'no longer contains')
    .replace('is present', 'disappears')
    .replace('drops below', 'is not below')
    .replace('rises above', 'is not above')
    .replace('changes', "doesn't change")
    .replace('equals', "doesn't equal");
}

function locatorText(locator: Locator): string {
  if (locator.type === 'page') return 'the page';
  if (!locator.query) return locator.type === 'xpath' ? 'an xpath' : 'a selector';
  return locator.query;
}

const WIRE_MAP: Record<string, ConditionWireKind> = {
  'text|contains|false': 'text_appears',
  'text|contains|true': 'text_disappears',
  'element|exists|false': 'selector_exists',
  'element|exists|true': 'selector_missing',
  'value|contains|false': 'selector_text_contains',
  'value|contains|true': 'selector_text_not_contains',
  'price|below|false': 'price_below',
  'price|above|false': 'price_above',
  'price|changed|false': 'price_changed',
};

export function toWire(c: Condition): ConditionInput {
  const key = `${c.subject}|${c.op}|${c.negate}`;
  const kind = WIRE_MAP[key];
  if (!kind) throw new Error(`Unsupported condition: ${c.subject} ${c.op} (negate=${c.negate})`);

  const input: ConditionInput = { kind };
  const selector = c.locator.type !== 'page' ? c.locator.query : undefined;

  if (c.subject === 'text') {
    input.value = c.value;
  } else if (c.subject === 'element') {
    input.selector = selector;
  } else if (c.subject === 'value') {
    input.selector = selector;
    input.value = c.value;
  } else if (c.subject === 'price') {
    input.price_selector = selector || undefined;
    if (c.op !== 'changed') {
      const dollars = parseFloat(c.value);
      if (Number.isFinite(dollars)) input.threshold_cents = Math.round(dollars * 100);
    }
  }

  return input;
}

const REVERSE_MAP: Record<ConditionWireKind, { subject: Subject; op: string; negate: boolean }> = {
  text_appears: { subject: 'text', op: 'contains', negate: false },
  text_disappears: { subject: 'text', op: 'contains', negate: true },
  selector_exists: { subject: 'element', op: 'exists', negate: false },
  selector_missing: { subject: 'element', op: 'exists', negate: true },
  selector_text_contains: { subject: 'value', op: 'contains', negate: false },
  selector_text_not_contains: { subject: 'value', op: 'contains', negate: true },
  price_below: { subject: 'price', op: 'below', negate: false },
  price_above: { subject: 'price', op: 'above', negate: false },
  price_changed: { subject: 'price', op: 'changed', negate: false },
};

export function fromWire(input: ConditionInput): Condition {
  const mapping = REVERSE_MAP[input.kind];
  const selector = input.selector ?? input.price_selector ?? '';
  return blankCondition({
    subject: mapping.subject,
    op: mapping.op,
    negate: mapping.negate,
    value:
      mapping.subject === 'price' && input.threshold_cents != null
        ? (input.threshold_cents / 100).toFixed(2)
        : input.value ?? '',
    locator: selector ? { type: 'css', query: selector } : { type: 'page', query: '' },
  });
}

const UNSUPPORTED_OPS = new Set(['value|equals', 'value|changed']);

export type BuildResult =
  | { ok: true; conditions: ConditionInput[] }
  | { ok: false; error: string };

export function validateAndBuild(conditions: Condition[]): BuildResult {
  const result: ConditionInput[] = [];

  for (const c of conditions) {
    const s = subjectDef(c.subject);

    if (UNSUPPORTED_OPS.has(`${c.subject}|${c.op}`)) {
      return { ok: false, error: `"${s.label} ${c.op}" is not yet supported by the backend.` };
    }

    const vf = valueField(c.subject, c.op);
    if (vf && !c.value.trim()) {
      return { ok: false, error: `${s.label} condition requires a value.` };
    }
    if (s.locator === 'required' && (!c.locator.query.trim() || c.locator.type === 'page')) {
      return { ok: false, error: `${s.label} condition requires a CSS selector.` };
    }
    if (c.subject === 'price' && c.op !== 'changed') {
      if (!Number.isFinite(parseFloat(c.value))) {
        return { ok: false, error: 'Price threshold must be a number.' };
      }
    }

    result.push(toWire(c));
  }

  return { ok: true, conditions: result };
}
