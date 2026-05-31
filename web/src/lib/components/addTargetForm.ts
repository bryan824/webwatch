import type { ConditionInput, ConditionWireKind, TargetInput } from '$lib/api/types';

export type AddTargetConditionDraft = {
  kind: ConditionWireKind;
  value: string;
  selector: string;
  price: string;
  price_selector: string;
};

export type AddTargetDraft = {
  name: string;
  url: string;
  enabled: boolean;
  interval: string;
  conditions: AddTargetConditionDraft[];
};

export type AddTargetBuildResult =
  | { ok: true; input: TargetInput }
  | { ok: false; error: string };


export type AddTargetConditionDraftWithId = AddTargetConditionDraft & {
  id: number;
};

export type AddTargetFormDefaults = {
  name: string;
  url: string;
  enabled: boolean;
  interval: string;
  conditions: AddTargetConditionDraftWithId[];
};

export const CONDITION_KINDS: { value: ConditionWireKind; label: string }[] = [
  { value: 'text_appears', label: 'text appears' },
  { value: 'text_disappears', label: 'text disappears' },
  { value: 'selector_exists', label: 'selector exists' },
  { value: 'selector_missing', label: 'selector missing' },
  { value: 'selector_text_contains', label: 'selector text contains' },
  { value: 'selector_text_not_contains', label: 'selector text not contains' },
  { value: 'price_below', label: 'price below' },
  { value: 'price_above', label: 'price above' },
  { value: 'price_changed', label: 'price changed' }
];

export function fieldsForCondition(kind: ConditionWireKind) {
  return {
    value:
      kind === 'text_appears' ||
      kind === 'text_disappears' ||
      kind === 'selector_text_contains' ||
      kind === 'selector_text_not_contains',
    selector:
      kind === 'selector_exists' ||
      kind === 'selector_missing' ||
      kind === 'selector_text_contains' ||
      kind === 'selector_text_not_contains',
    threshold: kind === 'price_below' || kind === 'price_above',
    priceSelector: kind === 'price_below' || kind === 'price_above' || kind === 'price_changed'
  };
}


export function blankConditionDraft(id: number): AddTargetConditionDraftWithId {
  return {
    id,
    kind: 'text_appears',
    value: '',
    selector: '',
    price: '',
    price_selector: ''
  };
}

export function blankAddTargetDefaults(id: number): AddTargetFormDefaults {
  return {
    name: '',
    url: '',
    enabled: true,
    interval: '',
    conditions: [blankConditionDraft(id)]
  };
}

export function buildTargetInput(draft: AddTargetDraft): AddTargetBuildResult {
  if (!draft.name.trim()) {
    return { ok: false, error: 'Name is required.' };
  }

  const url = draft.url.trim();
  try {
    new URL(url);
  } catch {
    return { ok: false, error: 'Enter a valid absolute URL (https://…).' };
  }

  const conditions: ConditionInput[] = [];
  for (const conditionDraft of draft.conditions) {
    const fields = fieldsForCondition(conditionDraft.kind);
    const condition: ConditionInput = { kind: conditionDraft.kind };

    if (fields.value) {
      if (!conditionDraft.value.trim()) {
        return { ok: false, error: 'A text value is required for the selected condition.' };
      }
      condition.value = conditionDraft.value.trim();
    }

    if (fields.selector) {
      if (!conditionDraft.selector.trim()) {
        return { ok: false, error: 'A CSS selector is required for the selected condition.' };
      }
      condition.selector = conditionDraft.selector.trim();
    }

    if (fields.threshold) {
      const dollars = Number.parseFloat(conditionDraft.price);
      if (!Number.isFinite(dollars)) {
        return { ok: false, error: 'A price threshold (USD) is required.' };
      }
      condition.threshold_cents = Math.round(dollars * 100);
    }

    if (fields.priceSelector && conditionDraft.price_selector.trim()) {
      condition.price_selector = conditionDraft.price_selector.trim();
    }

    conditions.push(condition);
  }

  const input: TargetInput = {
    name: draft.name.trim(),
    url,
    enabled: draft.enabled,
    conditions
  };

  const mins = Number.parseFloat(draft.interval);
  if (draft.interval.trim() && Number.isFinite(mins) && mins > 0) {
    input.interval_secs = Math.round(mins * 60);
  }

  return { ok: true, input };
}
