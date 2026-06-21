# Watch Builder Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port the prototype Watch Builder from `designs/webwatch-watch-builder/` into the production SolidJS frontend — converting the builder from a modal dialog to an inline route-based pane with enriched condition model, locator rows, schedule presets, NL describe mode, and summary block.

**Architecture:** The builder becomes a full pane rendered at `/watches/new` (create) and `/watches/$id/edit` (edit). The condition model gains `cid`, `locator {type, query}`, and `strategy` fields. A mock NL interpreter powers the "Describe" tab. The existing wire format mapping (`toWire`/`fromWire`) adapts to the enriched model. All new CSS goes into the existing `instrument.css`.

**Tech Stack:** SolidJS + JSX + Vite, `@msviderok/base-ui-solid` (Tooltip, Dialog), `solid-js/store` (createStore + produce), TanStack Solid Router + Query

---

## File Structure

| Action | Path | Responsibility |
|--------|------|----------------|
| Rewrite | `web/src/lib/conditions.ts` | Enriched condition model, SUBJECTS catalog, wire mapping |
| Create | `web/src/lib/nl.ts` | Mock NL keyword interpreter |
| Create | `web/src/components/Icon.tsx` | Inline SVG icon component |
| Rewrite | `web/src/components/ConditionCard.tsx` | Rich condition card with locator row + strategy |
| Create | `web/src/components/BuilderPane.tsx` | Full builder pane (target, rules/NL tabs, schedule, summary, save) |
| Create | `web/src/pages/NewWatch.tsx` | Route component for `/watches/new` |
| Create | `web/src/pages/EditWatch.tsx` | Route component for `/watches/$id/edit` |
| Modify | `web/src/styles/instrument.css` | Add builder-pane CSS from prototype |
| Modify | `web/src/App.tsx` | Add new routes |
| Modify | `web/src/components/Topbar.tsx` | Navigate to `/watches/new` instead of opening dialog |
| Modify | `web/src/components/WatchDetail.tsx` | Add "edit" link |
| Delete | `web/src/components/WatchBuilder.tsx` | Replaced by BuilderPane |

---

### Task 1: Enrich the condition model

**Files:**
- Rewrite: `web/src/lib/conditions.ts`

This replaces the flat `SubjectSpec` + flat `Condition` with the prototype's richer model: ops as `{id, label}` objects, locator modes per subject, value specs with `omitFor`, `cid` on conditions, `locator {type, query}` replacing flat `selector`, and `strategy`. The wire format mapping (`toWire`/`fromWire`) adapts to the new field names. Ops the backend doesn't support yet (`value|equals`, `value|changed`) are included in the catalog but rejected by `validateAndBuild`.

- [ ] **Step 1: Rewrite conditions.ts**

Replace the entire contents of `web/src/lib/conditions.ts` with:

```typescript
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

// --- Wire format mapping ---
// Maps frontend (subject, op, negate) → backend ConditionWireKind.
// Only combinations the backend supports are included.

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
```

- [ ] **Step 2: Verify the build still compiles**

Run: `cd /Users/bryan/Projects/webwatch/web && npx vite build 2>&1 | head -30`

Expected: Build errors in ConditionCard.tsx (uses old `SubjectSpec` shape — will be fixed in Task 4). The conditions.ts itself should have no type errors.

- [ ] **Step 3: Commit**

```bash
git add web/src/lib/conditions.ts
git commit -m "feat: enrich condition model with locator, strategy, cid"
```

---

### Task 2: Add Icon component

**Files:**
- Create: `web/src/components/Icon.tsx`

Port the SVG icon set from `designs/webwatch-watch-builder/ui.js`. These are inline SVG paths rendered at 16×16 with `stroke="currentColor"`.

- [ ] **Step 1: Create Icon.tsx**

```tsx
const PATHS: Record<string, string> = {
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

interface Props {
  name: string;
  class?: string;
}

export function Icon(props: Props) {
  return (
    <svg
      class={'ic' + (props.class ? ' ' + props.class : '')}
      width="16"
      height="16"
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      stroke-width="1.5"
      stroke-linecap="round"
      stroke-linejoin="round"
      innerHTML={PATHS[props.name] ?? ''}
    />
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add web/src/components/Icon.tsx
git commit -m "feat: add Icon component with SVG icon set"
```

---

### Task 3: Add builder CSS to instrument.css

**Files:**
- Modify: `web/src/styles/instrument.css`

Add all the CSS from the prototype's `styles.css` that's missing from the production `instrument.css`. The production file already has: tokens, app frame, topbar, rail, status dot, detail pane, builder (basic), buttons, dialog, tooltip, tabs, toast, skeleton, motion. What's missing: builder pane layout, URL row, NL compose, condition locator/advanced row, chips, dry-run result row, summary, schedule presets, save bar, switch, engine badge, AI button, and the `@keyframes scan` animation.

- [ ] **Step 1: Add missing CSS**

Append the following CSS blocks to `web/src/styles/instrument.css`, before the `/* ───────────── motion ───────────── */` section:

```css
/* ───────────── builder pane ───────────── */
.builder-pane { max-width: 760px; margin: 0 auto; padding: 24px 28px 120px; }
.builder-pane__title { font-family: var(--cond); font-size: 23px; font-weight: 600; letter-spacing: .01em; margin: 2px 0 2px; }
.builder-pane__sub { color: var(--muted); font-size: 13px; margin-bottom: 22px; }

.block { border: 1px solid var(--line); border-radius: var(--r-lg); background: var(--surface); margin-bottom: 16px; box-shadow: var(--shadow); }
.block__head { display: flex; align-items: center; gap: 10px; padding: 12px 16px; border-bottom: 1px solid var(--line); }
.block__head .label { flex: none; }
.block__head .rule { flex: 1; height: 1px; background: repeating-linear-gradient(90deg, var(--line) 0 6px, transparent 6px 10px); }
.block__body { padding: 16px; }

/* URL row + engine badge */
.urlrow { display: grid; grid-template-columns: 1fr auto; gap: 12px; align-items: center; }
.engine-badge { display: inline-flex; align-items: center; gap: 7px; padding: 6px 10px; border-radius: var(--r); border: 1px solid var(--line); background: var(--surface-2); font-size: 12px; color: var(--muted); white-space: nowrap; }
.engine-badge .dot { width: 7px; height: 7px; }

/* NL compose */
.nl__row { display: grid; grid-template-columns: 1fr auto; gap: 10px; align-items: stretch; }
.nl__input {
  width: 100%; resize: none; min-height: 46px; background: var(--bg); color: var(--text);
  border: 1px solid var(--line-2); border-radius: var(--r); padding: 12px 13px; font-size: 14.5px;
  font-family: var(--sans); line-height: 1.4;
}
.nl__input:focus { border-color: var(--ai); outline: none; box-shadow: 0 0 0 3px oklch(0.75 0.16 300 / .14); }
.nl__hint { margin-top: 8px; color: var(--faint); font-size: 12px; display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
.nl__chip { padding: 3px 8px; border: 1px dashed var(--line-2); border-radius: 20px; font-size: 11.5px; color: var(--muted); background: transparent; transition: border-color .14s, color .14s; }
.nl__chip:hover { border-color: var(--ai); color: var(--text); }

.understood { margin-top: 12px; border: 1px solid color-mix(in oklch, var(--ai) 30%, var(--line)); background: oklch(0.21 0.03 300 / .5); border-radius: var(--r); padding: 11px 13px; display: flex; gap: 10px; align-items: flex-start; animation: rise .3s var(--ease); }
.understood__icon { width: 16px; height: 16px; flex: none; color: var(--ai); margin-top: 1px; }
.understood__text { font-size: 12.5px; color: color-mix(in oklch, var(--ai) 25%, var(--text)); }

/* condition advanced row (locator + strategy) */
.cond__advanced { border-top: 1px dashed var(--line); padding: 12px; display: grid; gap: 12px; background: oklch(0.19 0.012 248); }
.adv-grid { display: grid; grid-template-columns: auto 1fr; gap: 10px 12px; align-items: center; }

/* chips */
.chip { display: inline-flex; align-items: center; gap: 5px; padding: 2px 7px; border-radius: 20px; font-family: var(--mono); font-size: 10.5px; letter-spacing: .02em; border: 1px solid color-mix(in oklch, var(--c, var(--line)) 40%, var(--line)); color: color-mix(in oklch, var(--c, var(--muted)) 65%, var(--text)); background: color-mix(in oklch, var(--c, var(--surface)) 12%, transparent); white-space: nowrap; }
.chip .dot { width: 6px; height: 6px; --c: inherit; }
.chip.exact   { --c: var(--ok); }
.chip.keyword { --c: var(--warn); }
.chip.ai      { --c: var(--ai); }
.chip.http    { --c: var(--faint); }
.chip.browser { --c: var(--info); }

/* summary + schedule + save */
.summary { display: flex; gap: 10px; align-items: flex-start; padding: 13px 16px; }
.summary__txt { font-size: 13px; color: var(--muted); }
.summary__txt em { color: var(--accent); font-style: normal; }

.sched { display: flex; gap: 10px; align-items: center; flex-wrap: wrap; }
.presets { display: inline-flex; gap: 4px; }
.preset { padding: 6px 11px; border-radius: var(--r); border: 1px solid var(--line-2); background: var(--bg); color: var(--muted); font-family: var(--mono); font-size: 12px; cursor: pointer; }
.preset[data-on="true"] { border-color: var(--accent); color: var(--accent); background: oklch(0.22 0.04 210 / .5); }

.savebar { position: sticky; bottom: 0; display: flex; gap: 12px; align-items: center; padding: 14px 28px; margin: 0 -28px -120px; border-top: 1px solid var(--line); background: linear-gradient(transparent, var(--bg) 40%); backdrop-filter: blur(8px); }
.savebar__spacer { flex: 1; }

/* switch */
.ww-switch { position: relative; width: 38px; height: 22px; border-radius: 20px; background: var(--surface-3); border: 1px solid var(--line-2); padding: 0; transition: background .16s, border-color .16s; flex: none; cursor: pointer; }
.ww-switch[data-checked] { background: color-mix(in oklch, var(--accent) 60%, var(--surface-3)); border-color: transparent; }
.ww-switch__thumb { display: block; width: 16px; height: 16px; border-radius: 50%; background: var(--text); transform: translateX(3px); transition: transform .16s var(--ease); box-shadow: 0 1px 3px oklch(0 0 0 /.5); pointer-events: none; }
.ww-switch[data-checked] .ww-switch__thumb { transform: translateX(19px); background: var(--accent-ink); }
.switch-row { display: inline-flex; align-items: center; gap: 9px; cursor: pointer; }
.switch-row span { font-size: 13px; color: var(--muted); }

/* AI button */
.btn.ai { background: var(--ai); color: oklch(0.16 0.03 300); border-color: transparent; font-weight: 600; }
.btn.ai:hover { background: oklch(0.79 0.16 300); }

/* delete trigger */
.as-trigger { display: inline-flex; align-items: center; gap: 8px; background: transparent; border: 1px solid color-mix(in oklch, var(--bad) 40%, var(--line)); color: var(--bad); border-radius: var(--r); padding: 8px 12px; font-size: 13px; cursor: pointer; }
.as-trigger:hover { background: oklch(0.3 0.08 25 / .3); }

.kbd { font-family: var(--mono); font-size: 10.5px; padding: 1px 5px; border: 1px solid var(--line-2); border-bottom-width: 2px; border-radius: 4px; color: var(--muted); background: var(--bg); }
```

Also add `@keyframes scan` to the motion section:

```css
@keyframes scan { 100% { transform: translateX(100%); } }
```

- [ ] **Step 2: Commit**

```bash
git add web/src/styles/instrument.css
git commit -m "feat: add builder pane CSS from prototype"
```

---

### Task 4: Rewrite ConditionCard with locator row

**Files:**
- Rewrite: `web/src/components/ConditionCard.tsx`

Port the condition card from the prototype's `builder.js:condCard` + `locatorRow` + `valueControl`. The card has: index badge, subject select, op select, value input (text or money), truth toggle (TRUE/FALSE), remove button, locator row (where to look: page/css/xpath + query input), and strategy selector (auto/exact/keyword/ai).

- [ ] **Step 1: Rewrite ConditionCard.tsx**

```tsx
import { Show, For } from 'solid-js';
import { Icon } from './Icon';
import type { Condition, Subject } from '../lib/conditions';
import { SUBJECTS, opsFor, valueField, locatorMode, subjectDef } from '../lib/conditions';

interface Props {
  index: number;
  condition: Condition;
  onChange: (c: Condition) => void;
  onChangeSubject: (cid: string, subject: Subject) => void;
  onRemove: () => void;
}

export function ConditionCard(props: Props) {
  const c = () => props.condition;
  const opOpts = () => opsFor(c().subject);
  const vf = () => valueField(c().subject, c().op);
  const locMode = () => locatorMode(c().subject);

  function patch(updates: Partial<Condition>) {
    props.onChange({ ...c(), ...updates });
  }

  return (
    <div class="cond">
      <div class="cond__top">
        <span class="cond__idx">{String(props.index + 1).padStart(2, '0')}</span>
        <div class="cond__grid">
          <select
            class="nselect"
            value={c().subject}
            onChange={(e) => props.onChangeSubject(c().cid, e.currentTarget.value as Subject)}
          >
            <For each={SUBJECTS}>
              {(s) => <option value={s.id}>{s.label}</option>}
            </For>
          </select>

          <select
            class="nselect"
            value={c().op}
            onChange={(e) => patch({ op: e.currentTarget.value })}
          >
            <For each={opOpts()}>
              {(o) => <option value={o.id}>{o.label}</option>}
            </For>
          </select>

          <Show when={vf()}>
            {(field) => (
              field().kind === 'money' ? (
                <span style="display:inline-flex;align-items:center;gap:0;background:var(--bg);border:1px solid var(--line-2);border-radius:var(--r)">
                  <span class="mono" style="padding-left:10px;color:var(--faint)">$</span>
                  <input
                    class="input mono"
                    style="width:92px;border:0;background:transparent"
                    inputmode="decimal"
                    placeholder={field().placeholder}
                    value={c().value}
                    onInput={(e) => patch({ value: e.currentTarget.value })}
                  />
                </span>
              ) : (
                <input
                  class="input"
                  style="width:200px;flex:1;min-width:120px"
                  placeholder={field().placeholder}
                  value={c().value}
                  onInput={(e) => patch({ value: e.currentTarget.value })}
                />
              )
            )}
          </Show>

          <div class="seg truth" role="group" aria-label="Alert when">
            <button
              data-on={(!c().negate).toString()}
              data-truth="true"
              onClick={() => patch({ negate: false })}
            >
              TRUE
            </button>
            <button
              data-on={c().negate.toString()}
              data-truth="false"
              onClick={() => patch({ negate: true })}
            >
              FALSE
            </button>
          </div>
        </div>
        <button class="cond__remove" aria-label="Remove condition" onClick={props.onRemove}>
          <Icon name="trash" />
        </button>
      </div>

      <Show when={locMode() !== 'page'}>
        <LocatorRow condition={c()} mode={locMode()} onPatch={patch} />
      </Show>
    </div>
  );
}

function LocatorRow(props: {
  condition: Condition;
  mode: 'required' | 'optional';
  onPatch: (updates: Partial<Condition>) => void;
}) {
  const c = () => props.condition;
  const types = () => props.mode === 'required' ? ['css', 'xpath'] as const : ['page', 'css', 'xpath'] as const;
  const tlabel: Record<string, string> = { page: 'whole page', css: 'CSS', xpath: 'XPath' };

  return (
    <div class="cond__advanced">
      <div class="adv-grid">
        <span class="label">Where to look{props.mode === 'optional' ? ' · optional' : ''}</span>
        <div style="display:flex;gap:8px;align-items:center;flex-wrap:wrap;min-width:0">
          <div class="seg" role="group" aria-label="Locator type">
            <For each={types()}>
              {(t) => (
                <button
                  data-on={(c().locator.type === t).toString()}
                  onClick={() => props.onPatch({
                    locator: { type: t, query: t === 'page' ? '' : c().locator.query },
                  })}
                >
                  {tlabel[t]}
                </button>
              )}
            </For>
          </div>
          <Show when={c().locator.type !== 'page'}>
            <input
              class="input mono"
              style="flex:1;min-width:160px"
              spellcheck={false}
              placeholder={c().locator.type === 'xpath' ? "//span[@class='price']" : '.price'}
              value={c().locator.query}
              onInput={(e) => props.onPatch({
                locator: { ...c().locator, query: e.currentTarget.value },
              })}
            />
          </Show>
        </div>
        <span class="label">How</span>
        <div style="display:flex;gap:8px;align-items:center;flex-wrap:wrap">
          <div class="seg" role="group" aria-label="Extraction strategy">
            <For each={['auto', 'exact', 'keyword', 'ai'] as const}>
              {(s) => (
                <button
                  data-on={(c().strategy === s).toString()}
                  onClick={() => props.onPatch({ strategy: s })}
                >
                  {s}
                </button>
              )}
            </For>
          </div>
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add web/src/components/ConditionCard.tsx
git commit -m "feat: rewrite ConditionCard with locator row and strategy"
```

---

### Task 5: Create NL interpreter (mock)

**Files:**
- Create: `web/src/lib/nl.ts`

Port the mock keyword-based interpreter from `designs/webwatch-watch-builder/nl.js`. This interprets natural language descriptions into structured conditions. In production, this would be replaced by a backend LLM call; the mock demonstrates the UI flow.

- [ ] **Step 1: Create nl.ts**

```typescript
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
    const quoted = /[""“]([^""”]{2,})[""”]/.exec(raw);
    const phrase = quoted ? quoted[1] : raw.split(/\s+/).slice(0, 4).join(' ');
    conditions.push(blankCondition({ subject: 'text', op: 'contains', value: phrase }));
    notes.push(`couldn't recognise an intent — watching for the phrase "${phrase}"`);
    usedAi = true;
  }

  const n = conditions.length;
  const explanation = `Understood as ${n} rule${n > 1 ? 's' : ''}${notes.length ? ' — ' + notes.join('; ') : ''}.`;
  return { conditions, explanation, usedAi, confidence: usedAi ? 0.78 : 0.9 };
}
```

- [ ] **Step 2: Commit**

```bash
git add web/src/lib/nl.ts
git commit -m "feat: add mock NL keyword interpreter"
```

---

### Task 6: Create BuilderPane component

**Files:**
- Create: `web/src/components/BuilderPane.tsx`

This is the main builder pane ported from `designs/webwatch-watch-builder/builder.js` + `app.js`. It manages draft state via Solid's `createStore`, and composes: target block, what-to-watch tabs (Describe/Rules), NL pane, condition cards, summary, schedule presets, enabled switch, and save bar.

The component accepts an optional `target: TargetStatus` prop for edit mode (pre-populate from existing watch) and an `onSaved`/`onCancel` callback for navigation after save.

- [ ] **Step 1: Create BuilderPane.tsx**

```tsx
import { Show, For, createEffect, createSignal } from 'solid-js';
import { createStore, produce } from 'solid-js/store';
import { Switch as BSwitch } from '@msviderok/base-ui-solid';
import { Icon } from './Icon';
import { ConditionCard } from './ConditionCard';
import { ConfirmDialog } from './ConfirmDialog';
import {
  blankCondition, coerceForSubject, describeCondition,
  validateAndBuild, fromWire,
} from '../lib/conditions';
import type { Condition, Subject } from '../lib/conditions';
import { interpret } from '../lib/nl';
import { createAddTargetMutation, createDeleteTargetMutation } from '../lib/mutations';
import type { TargetInput, TargetStatus } from '../lib/types';
import { addToast } from './Toast';

const INTERVALS = [
  { secs: 300, label: '5m' },
  { secs: 900, label: '15m' },
  { secs: 3600, label: '1h' },
  { secs: 21600, label: '6h' },
  { secs: 86400, label: 'daily' },
];

const NL_EXAMPLES = [
  'tell me when the mug is back in stock and under $25',
  'alert when "Sold out" is lifted',
  'when the price drops below $400',
  'when the listing description changes',
];

interface Draft {
  name: string;
  url: string;
  enabled: boolean;
  intervalSecs: number;
  mode: 'describe' | 'rules';
  nl: string;
  nlResult: { explanation: string; usedAi: boolean } | null;
  conditions: Condition[];
}

function newDraft(): Draft {
  return {
    name: '', url: '', enabled: true, intervalSecs: 900,
    mode: 'describe', nl: '', nlResult: null, conditions: [],
  };
}

function draftFromTarget(t: TargetStatus): Draft {
  const conditions = t.condition_results.length > 0
    ? t.condition_results.map((cr) => {
        const kind = cr.kind;
        const wireKindMap: Record<string, string> = {
          text: 'text_appears',
          selector: 'selector_exists',
          selector_text: 'selector_text_contains',
          price: 'price_below',
          price_observed: 'price_changed',
        };
        const wireKind = wireKindMap[kind] ?? 'text_appears';
        return fromWire({
          kind: wireKind as any,
          value: cr.evidence[0] || undefined,
        });
      })
    : [blankCondition()];

  return {
    name: t.name,
    url: t.url,
    enabled: t.enabled,
    intervalSecs: 900,
    mode: 'rules',
    nl: '',
    nlResult: null,
    conditions,
  };
}

interface Props {
  target?: TargetStatus;
  onSaved?: () => void;
  onCancel?: () => void;
  onDeleted?: () => void;
}

export function BuilderPane(props: Props) {
  const isEdit = () => !!props.target;
  const add = createAddTargetMutation();
  const del = createDeleteTargetMutation();
  const [error, setError] = createSignal('');
  const [confirmOpen, setConfirmOpen] = createSignal(false);
  const [justSaved, setJustSaved] = createSignal(false);

  const initial = props.target ? draftFromTarget(props.target) : newDraft();
  const [draft, setDraft] = createStore<Draft>(initial);

  function setUrl(url: string) { setDraft('url', url); }
  function setName(name: string) { setDraft('name', name); }

  function setNl(text: string) { setDraft('nl', text); }
  function runNl() {
    const result = interpret(draft.nl);
    if (!result.conditions.length) return;
    setDraft(produce((d) => {
      d.conditions = result.conditions;
      d.nlResult = { explanation: result.explanation, usedAi: result.usedAi };
    }));
  }

  function setMode(mode: 'describe' | 'rules') { setDraft('mode', mode); }

  function addCondition() {
    setDraft(produce((d) => {
      d.conditions.push(blankCondition());
      d.mode = 'rules';
    }));
  }

  function changeSubject(cid: string, subjectId: Subject) {
    setDraft('conditions', (cs) =>
      cs.map((c) => c.cid === cid ? coerceForSubject({ ...c }, subjectId) : c)
    );
  }

  function updateCondition(cid: string, patch: Partial<Condition>) {
    setDraft('conditions', (cs) =>
      cs.map((c) => c.cid === cid ? { ...c, ...patch } : c)
    );
  }

  function removeCondition(cid: string) {
    setDraft('conditions', (cs) => cs.filter((c) => c.cid !== cid));
  }

  function setIntervalSecs(secs: number) { setDraft('intervalSecs', secs); }
  function setEnabled(on: boolean) { setDraft('enabled', on); }

  function save() {
    setError('');
    const n = draft.name.trim();
    if (!n) { setError('Name is required.'); return; }

    const u = draft.url.trim();
    try { new URL(u); } catch { setError('Enter a valid absolute URL (https://...).'); return; }

    if (draft.conditions.length === 0) { setError('Add at least one condition.'); return; }

    const result = validateAndBuild(draft.conditions);
    if (!result.ok) { setError(result.error); return; }

    const input: TargetInput = {
      name: n,
      url: u,
      enabled: draft.enabled,
      conditions: result.conditions,
      interval_secs: draft.intervalSecs,
    };

    add.mutate(input, {
      onSuccess: () => {
        setJustSaved(true);
        setTimeout(() => setJustSaved(false), 1600);
        addToast(`${isEdit() ? 'Updated' : 'Created'} ${n}`, 'success');
        props.onSaved?.();
      },
    });
  }

  function handleDelete() {
    if (!props.target) return;
    del.mutate(props.target.target_id, {
      onSuccess: () => {
        addToast('Deleted', 'success');
        props.onDeleted?.();
      },
    });
  }

  const summaryText = () => {
    if (!draft.conditions.length) return null;
    return draft.conditions.map(describeCondition).join('  ·  AND  ·  ');
  };

  return (
    <div class="builder-pane">
      <div>
        <div class="builder-pane__title">
          {isEdit() ? draft.name || 'Edit watch' : 'New watch'}
        </div>
        <div class="builder-pane__sub">
          Point at a page, say what you want to know, and confirm what we'd alert on.
        </div>
      </div>

      {/* TARGET BLOCK */}
      <div class="block">
        <div class="block__head">
          <span class="label">Target</span><span class="rule" />
        </div>
        <div class="block__body" style="display:grid;gap:14px">
          <div class="field">
            <span class="label">Page URL</span>
            <input
              class="input mono"
              placeholder="https://store.example.com/products/…"
              spellcheck={false}
              value={draft.url}
              onInput={(e) => setUrl(e.currentTarget.value)}
            />
          </div>
          <div class="field">
            <span class="label">Name</span>
            <input
              class="input"
              placeholder="My watch"
              value={draft.name}
              onInput={(e) => setName(e.currentTarget.value)}
            />
          </div>
        </div>
      </div>

      {/* WHAT TO WATCH */}
      <div class="block">
        <div class="block__head">
          <span class="label">What to watch</span>
          <span class="rule" />
          <div class="seg" role="group">
            <button data-on={(draft.mode === 'describe').toString()} onClick={() => setMode('describe')}>
              <Icon name="ai" /> Describe
            </button>
            <button data-on={(draft.mode === 'rules').toString()} onClick={() => setMode('rules')}>
              <Icon name="edit" /> Build rules
            </button>
          </div>
        </div>
        <div class="block__body">
          <Show when={draft.mode === 'describe'}>
            <div>
              <div class="nl__row">
                <textarea
                  class="nl__input"
                  placeholder="Tell me when… (e.g. the mug is back in stock and under $25)"
                  spellcheck={false}
                  value={draft.nl}
                  onInput={(e) => setNl(e.currentTarget.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
                      e.preventDefault();
                      runNl();
                    }
                  }}
                />
                <button class="btn ai" onClick={runNl} title="⌘↵">
                  <Icon name="ai" /> Understand
                </button>
              </div>
              <div class="nl__hint">
                <span style="color:var(--faint)">examples</span>
                <For each={NL_EXAMPLES}>
                  {(ex) => (
                    <button
                      class="nl__chip"
                      onClick={() => { setNl(ex); runNl(); }}
                    >
                      {ex}
                    </button>
                  )}
                </For>
              </div>
              <Show when={draft.nlResult}>
                {(nr) => (
                  <div class="understood">
                    <span class="understood__icon"><Icon name="ai" /></span>
                    <span class="understood__text">
                      {nr().explanation}
                      <Show when={nr().usedAi}>
                        {' '}<span class="mono" style="color:var(--ai)">
                          · AI resolved this at setup; checks stay deterministic.
                        </span>
                      </Show>
                    </span>
                  </div>
                )}
              </Show>
              <Show when={draft.conditions.length > 0}>
                <div style="margin-top:14px;display:grid;gap:8px">
                  <For each={draft.conditions}>
                    {(c, i) => (
                      <div class="cond" style="background:var(--surface)">
                        <div class="cond__top" style="padding:11px 12px">
                          <span class="cond__idx" style="padding-top:2px">
                            {String(i() + 1).padStart(2, '0')}
                          </span>
                          <div class="cond__grid" style="align-items:center">
                            <span style="font-size:13px">{describeCondition(c)}</span>
                          </div>
                          <button
                            class="btn ghost sm"
                            onClick={() => setMode('rules')}
                          >
                            <Icon name="edit" /> edit as rule
                          </button>
                        </div>
                      </div>
                    )}
                  </For>
                </div>
              </Show>
            </div>
          </Show>

          <Show when={draft.mode === 'rules'}>
            <div>
              <For each={draft.conditions}>
                {(c, i) => (
                  <ConditionCard
                    index={i()}
                    condition={c}
                    onChange={(updated) => updateCondition(c.cid, updated)}
                    onChangeSubject={changeSubject}
                    onRemove={() => removeCondition(c.cid)}
                  />
                )}
              </For>
              <button class="add-cond" onClick={addCondition}>
                <Icon name="plus" /> Add a condition — alerts fire when ALL match
              </button>
            </div>
          </Show>
        </div>
      </div>

      {/* SUMMARY */}
      <Show when={draft.conditions.length > 0}>
        <div class="block">
          <div class="summary">
            <span style="color:var(--accent);margin-top:1px"><Icon name="target" /></span>
            <span class="summary__txt">
              Alert me when <em>all</em> of: {summaryText()}
            </span>
          </div>
        </div>
      </Show>

      {/* SCHEDULE */}
      <div class="block">
        <div class="block__head">
          <span class="label">Schedule</span><span class="rule" />
        </div>
        <div class="block__body sched">
          <span class="label" style="margin-right:2px">Check every</span>
          <div class="presets">
            <For each={INTERVALS}>
              {(iv) => (
                <button
                  class="preset"
                  data-on={(draft.intervalSecs === iv.secs).toString()}
                  onClick={() => setIntervalSecs(iv.secs)}
                >
                  {iv.label}
                </button>
              )}
            </For>
          </div>
          <span style="flex:1" />
          <label class="switch-row">
            <BSwitch.Root
              class="ww-switch"
              checked={draft.enabled}
              onCheckedChange={setEnabled}
            >
              <BSwitch.Thumb class="ww-switch__thumb" />
            </BSwitch.Root>
            <span>{draft.enabled ? 'enabled' : 'paused'}</span>
          </label>
        </div>
      </div>

      {/* ERROR */}
      <Show when={error()}>
        <p style="color: var(--bad); font-size: 13px; margin: 0 0 12px">{error()}</p>
      </Show>

      {/* SAVE BAR */}
      <div class="savebar">
        <Show when={isEdit()}>
          <button class="as-trigger" onClick={() => setConfirmOpen(true)}>
            <Icon name="trash" /> Delete
          </button>
        </Show>
        <span class="savebar__spacer" />
        <button class="btn ghost" onClick={() => props.onCancel?.()}>Cancel</button>
        <button
          class="btn primary"
          onClick={save}
          disabled={add.isPending || draft.conditions.length === 0}
        >
          {justSaved()
            ? <><Icon name="check" /> Saved</>
            : add.isPending
              ? 'saving...'
              : isEdit() ? 'Save changes' : 'Create watch'}
        </button>
      </div>

      <ConfirmDialog
        open={confirmOpen()}
        onOpenChange={setConfirmOpen}
        title="Delete this watch?"
        description={`"${draft.name}" will stop being monitored. This can't be undone.`}
        confirmLabel="Delete watch"
        variant="danger"
        onConfirm={handleDelete}
      />
    </div>
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add web/src/components/BuilderPane.tsx
git commit -m "feat: create BuilderPane with NL, rules, schedule, summary"
```

---

### Task 7: Wire routing and navigation

**Files:**
- Create: `web/src/pages/NewWatch.tsx`
- Create: `web/src/pages/EditWatch.tsx`
- Modify: `web/src/App.tsx`
- Modify: `web/src/components/Topbar.tsx`
- Modify: `web/src/components/WatchDetail.tsx`
- Modify: `web/src/components/Shell.tsx`
- Delete: `web/src/components/WatchBuilder.tsx`

- [ ] **Step 1: Create NewWatch.tsx**

```tsx
import { useNavigate } from '@tanstack/solid-router';
import { BuilderPane } from '../components/BuilderPane';

export function NewWatch() {
  const navigate = useNavigate();

  return (
    <BuilderPane
      onSaved={() => navigate({ to: '/' })}
      onCancel={() => navigate({ to: '/' })}
    />
  );
}
```

- [ ] **Step 2: Create EditWatch.tsx**

```tsx
import { Show } from 'solid-js';
import { useParams, useNavigate } from '@tanstack/solid-router';
import { BuilderPane } from '../components/BuilderPane';
import { createTargetsQuery } from '../lib/queries';

export function EditWatch() {
  const params = useParams({ from: '/watches/$id/edit' });
  const navigate = useNavigate();
  const targets = createTargetsQuery();

  const target = () => (targets.data ?? []).find((t) => t.target_id === params()?.id);

  return (
    <Show
      when={target()}
      fallback={
        <Show
          when={targets.isPending}
          fallback={
            <div style="padding: 16px; color: var(--faint); font-size: 13px">
              Target not found.
            </div>
          }
        >
          <div style="padding: 16px; color: var(--faint); font-size: 13px">Loading...</div>
        </Show>
      }
    >
      {(t) => (
        <BuilderPane
          target={t()}
          onSaved={() => navigate({ to: '/watches/$id', params: { id: t().target_id } })}
          onCancel={() => navigate({ to: '/watches/$id', params: { id: t().target_id } })}
          onDeleted={() => navigate({ to: '/' })}
        />
      )}
    </Show>
  );
}
```

- [ ] **Step 3: Add routes to App.tsx**

Add the two new routes. The modified `App.tsx`:

```tsx
import { QueryClient, QueryClientProvider } from '@tanstack/solid-query';
import { RouterProvider, createRouter, createRoute, createRootRoute } from '@tanstack/solid-router';
import { Tooltip } from '@msviderok/base-ui-solid';
import { Shell } from './components/Shell';
import { Home } from './pages/Home';
import { Detail } from './pages/Detail';
import { NewWatch } from './pages/NewWatch';
import { EditWatch } from './pages/EditWatch';
import { ToastContainer } from './components/Toast';

const queryClient = new QueryClient();

const rootRoute = createRootRoute({
  component: Shell,
});

const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/',
  component: Home,
});

const detailRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/watches/$id',
  component: Detail,
});

const newWatchRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/watches/new',
  component: NewWatch,
});

const editWatchRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/watches/$id/edit',
  component: EditWatch,
});

const routeTree = rootRoute.addChildren([indexRoute, newWatchRoute, editWatchRoute, detailRoute]);
const router = createRouter({ routeTree });

declare module '@tanstack/solid-router' {
  interface Register {
    router: typeof router;
  }
}

export function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <Tooltip.Provider>
        <RouterProvider router={router} />
        <ToastContainer />
      </Tooltip.Provider>
    </QueryClientProvider>
  );
}
```

**Important:** `newWatchRoute` must come before `detailRoute` in `addChildren` so `/watches/new` matches before `/watches/$id`.

- [ ] **Step 4: Update Topbar.tsx — navigate instead of dialog**

Replace the dialog-based "+ watch" with a `Link` to `/watches/new`:

```tsx
import { Link } from '@tanstack/solid-router';
import { createReloadMutation, createNotifyMutation } from '../lib/mutations';
import { ConfirmDialog } from './ConfirmDialog';
import { createSignal } from 'solid-js';

interface Props {
  updatedLabel: string;
}

export function Topbar(props: Props) {
  const reload = createReloadMutation();
  const notify = createNotifyMutation();
  const [confirmOpen, setConfirmOpen] = createSignal(false);

  return (
    <header class="topbar">
      <div class="brand">
        <div class="brand__mark"><i /><u /></div>
        <span class="brand__name">web<b>watch</b></span>
      </div>
      <span class="topbar__stat mono">updated {props.updatedLabel}</span>
      <div class="topbar__spacer" />
      <Link to="/watches/new" class="btn sm primary">+ watch</Link>
      <button
        class="btn sm"
        disabled={reload.isPending}
        onClick={() => reload.mutate(undefined)}
      >
        {reload.isPending ? 'reloading...' : 'reload'}
      </button>
      <button class="btn sm" onClick={() => setConfirmOpen(true)}>send report</button>

      <ConfirmDialog
        open={confirmOpen()}
        onOpenChange={setConfirmOpen}
        title="Send Discord status report?"
        description="This re-checks every enabled target and posts one report to Discord. It can take a while."
        confirmLabel={notify.isPending ? 'sending...' : 'send report'}
        confirmDisabled={notify.isPending}
        onConfirm={() => notify.mutate(undefined)}
      />
    </header>
  );
}
```

- [ ] **Step 5: Update Shell.tsx — remove WatchBuilder dialog references**

The Shell.tsx no longer needs `WatchBuilder` or `addOpen` signal. The current Shell.tsx already doesn't use WatchBuilder (Topbar handles it), so just verify there are no references. Shell.tsx should remain as-is.

- [ ] **Step 6: Update WatchDetail.tsx — add edit link**

Add a `Link` to the edit route in the detail actions:

In `web/src/components/WatchDetail.tsx`, add the import and button:

```tsx
// Add to imports:
import { Link } from '@tanstack/solid-router';

// Add inside the detail__actions div, before the existing buttons:
<Link to="/watches/$id/edit" params={{ id: props.target.target_id }} class="btn sm">
  edit
</Link>
```

- [ ] **Step 7: Delete WatchBuilder.tsx**

```bash
rm web/src/components/WatchBuilder.tsx
```

- [ ] **Step 8: Commit**

```bash
git add web/src/pages/NewWatch.tsx web/src/pages/EditWatch.tsx web/src/App.tsx web/src/components/Topbar.tsx web/src/components/WatchDetail.tsx
git rm web/src/components/WatchBuilder.tsx
git commit -m "feat: route-based builder pane, replace dialog with /watches/new and /watches/:id/edit"
```

---

### Task 8: Build, fix, and verify

**Files:**
- Possibly any file touched in Tasks 1–7

- [ ] **Step 1: Build the frontend**

```bash
cd /Users/bryan/Projects/webwatch/web && npx vite build 2>&1
```

Expected: May have type errors or import issues. Fix them.

Common issues to watch for:
- `Switch` might not be exported from `@msviderok/base-ui-solid` → replace with a plain `<button>` styled as `.ww-switch`
- `useParams` in `EditWatch.tsx` needs the correct route path string
- Any remaining references to old `SubjectSpec` type or `selector` field on `Condition`

- [ ] **Step 2: Fix any build errors**

Address each error. If `BSwitch` is not available, replace with:

```tsx
<button
  class="ww-switch"
  data-checked={draft.enabled ? '' : undefined}
  onClick={() => setEnabled(!draft.enabled)}
>
  <span class="ww-switch__thumb" />
</button>
```

- [ ] **Step 3: Start the server and verify in browser**

```bash
cd /Users/bryan/Projects/webwatch && cargo run
```

Test:
1. Click "+ watch" → should navigate to `/watches/new` showing the builder pane
2. Fill in URL and name
3. Switch to "Build rules" tab → add a condition
4. Change subject, op, toggle truth
5. For element/value/price: verify locator row appears with page/css/xpath toggle
6. Switch to "Describe" tab → type "back in stock and under $25" → click Understand
7. Verify schedule presets (5m/15m/1h/6h/daily) toggle correctly
8. Verify enabled/paused switch works
9. Click Cancel → returns to home
10. Fill in a complete watch and click "Create watch" → verify it appears in the list
11. Click a watch in the list → detail view with "edit" button
12. Click edit → builder pane pre-populated with watch data

- [ ] **Step 4: Commit any fixes**

```bash
git add -A
git commit -m "fix: resolve build errors and polish builder integration"
```

---

## Notes

**Backend limitations for edit mode:** The current PATCH endpoint only supports `{ enabled: boolean }`. Full watch editing (name, url, conditions) via the builder would require expanding the PATCH handler to accept all fields. For now, the edit UI works for viewing the watch configuration and toggling enabled, but saving full changes requires a backend update.

**Unsupported condition ops:** `value|equals` and `value|changed` are included in the catalog UI but rejected by `validateAndBuild` since the backend has no corresponding wire kinds. When the backend adds these, remove them from the `UNSUPPORTED_OPS` set and add wire mappings.

**Dry-run engine:** The prototype's dry-run engine (`engine.js`) tests conditions against mock page data. This requires a backend API endpoint that fetches a page and returns structured results. Not included in this plan — conditions are saved and the backend checks them on schedule.

**NL interpreter:** The mock keyword interpreter demonstrates the UX flow. In production, this would call a backend endpoint that uses an LLM to interpret the user's description and return structured conditions.
