<!-- web/src/lib/components/AddTargetDialog.svelte -->
<script lang="ts">
  import * as Dialog from '$lib/components/ui/dialog';
  import { Input } from '$lib/components/ui/input';
  import { Button } from '$lib/components/ui/button';
  import { createAddTargetMutation } from '$lib/api/mutations';
  import type { ConditionWireKind, ConditionInput, TargetInput } from '$lib/api/types';

  let { open = $bindable(false) }: { open?: boolean } = $props();

  const add = createAddTargetMutation();

  type Draft = {
    id: number;
    kind: ConditionWireKind;
    value: string;
    selector: string;
    price: string;
    price_selector: string;
  };

  const KINDS: { value: ConditionWireKind; label: string }[] = [
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

  let nextId = 0;
  function blankDraft(): Draft {
    return { id: nextId++, kind: 'text_appears', value: '', selector: '', price: '', price_selector: '' };
  }

  let name = $state('');
  let url = $state('');
  let enabled = $state(true);
  let interval = $state('');
  let conditions = $state<Draft[]>([blankDraft()]);
  let error = $state('');

  const needsValue = (k: ConditionWireKind) =>
    k === 'text_appears' ||
    k === 'text_disappears' ||
    k === 'selector_text_contains' ||
    k === 'selector_text_not_contains';
  const needsSelector = (k: ConditionWireKind) =>
    k === 'selector_exists' ||
    k === 'selector_missing' ||
    k === 'selector_text_contains' ||
    k === 'selector_text_not_contains';
  const needsThreshold = (k: ConditionWireKind) => k === 'price_below' || k === 'price_above';
  const isPrice = (k: ConditionWireKind) =>
    k === 'price_below' || k === 'price_above' || k === 'price_changed';

  function addCondition() {
    conditions = [...conditions, blankDraft()];
  }
  function removeCondition(id: number) {
    conditions = conditions.filter((c) => c.id !== id);
  }
  function reset() {
    name = '';
    url = '';
    enabled = true;
    interval = '';
    conditions = [blankDraft()];
    error = '';
  }

  function buildConditions(): ConditionInput[] | null {
    const out: ConditionInput[] = [];
    for (const d of conditions) {
      const c: ConditionInput = { kind: d.kind };
      if (needsValue(d.kind)) {
        if (!d.value.trim()) {
          error = 'A text value is required for the selected condition.';
          return null;
        }
        c.value = d.value.trim();
      }
      if (needsSelector(d.kind)) {
        if (!d.selector.trim()) {
          error = 'A CSS selector is required for the selected condition.';
          return null;
        }
        c.selector = d.selector.trim();
      }
      if (needsThreshold(d.kind)) {
        const dollars = Number.parseFloat(d.price);
        if (!Number.isFinite(dollars)) {
          error = 'A price threshold (USD) is required.';
          return null;
        }
        c.threshold_cents = Math.round(dollars * 100);
      }
      if (isPrice(d.kind) && d.price_selector.trim()) c.price_selector = d.price_selector.trim();
      out.push(c);
    }
    return out;
  }

  function submit() {
    error = '';
    if (!name.trim()) {
      error = 'Name is required.';
      return;
    }
    try {
      new URL(url.trim());
    } catch {
      error = 'Enter a valid absolute URL (https://…).';
      return;
    }
    const built = buildConditions();
    if (!built) return;

    const input: TargetInput = { name: name.trim(), url: url.trim(), enabled, conditions: built };
    const mins = Number.parseFloat(interval);
    if (interval.trim() && Number.isFinite(mins) && mins > 0) input.interval_secs = Math.round(mins * 60);

    add.mutate(input, {
      onSuccess: () => {
        reset();
        open = false;
      }
    });
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="sm:max-w-lg">
    <Dialog.Header>
      <Dialog.Title class="font-mono tracking-tight">Add target</Dialog.Title>
      <Dialog.Description>
        Watch a page and alert when <em>every</em> condition matches. Saved to the database.
      </Dialog.Description>
    </Dialog.Header>

    <div class="max-h-[60vh] space-y-3 overflow-y-auto pr-1">
      <div class="space-y-1">
        <label class="font-mono text-xs uppercase tracking-wider text-muted-foreground" for="t-name">name</label>
        <Input id="t-name" bind:value={name} placeholder="Campfire Mug" class="font-mono" />
      </div>
      <div class="space-y-1">
        <label class="font-mono text-xs uppercase tracking-wider text-muted-foreground" for="t-url">url</label>
        <Input id="t-url" bind:value={url} placeholder="https://example.com/product" class="font-mono" />
      </div>
      <div class="flex flex-wrap items-center gap-4">
        <label class="flex items-center gap-2 font-mono text-xs text-muted-foreground">
          <input type="checkbox" bind:checked={enabled} class="accent-primary" /> enabled
        </label>
        <div class="flex items-center gap-2">
          <label class="font-mono text-xs uppercase tracking-wider text-muted-foreground" for="t-interval">
            interval (min)
          </label>
          <Input id="t-interval" bind:value={interval} placeholder="default" class="h-8 w-24 font-mono text-xs" />
        </div>
      </div>

      <div class="space-y-2">
        <div class="flex items-center justify-between">
          <span class="font-mono text-xs uppercase tracking-wider text-muted-foreground">conditions</span>
          <button type="button" class="font-mono text-xs text-primary hover:underline" onclick={addCondition}>
            + add
          </button>
        </div>
        {#each conditions as c (c.id)}
          <div class="space-y-2 rounded-md border border-border/70 bg-card/40 p-2.5">
            <div class="flex items-center gap-2">
              <select
                bind:value={c.kind}
                class="h-8 flex-1 rounded-md border border-input bg-background px-2 font-mono text-xs"
              >
                {#each KINDS as k}<option value={k.value}>{k.label}</option>{/each}
              </select>
              {#if conditions.length > 1}
                <button
                  type="button"
                  class="px-1 font-mono text-xs text-muted-foreground hover:text-red-400"
                  onclick={() => removeCondition(c.id)}
                  aria-label="remove condition"
                >
                  ✕
                </button>
              {/if}
            </div>
            {#if needsSelector(c.kind)}
              <Input bind:value={c.selector} placeholder="CSS selector — e.g. .price" class="h-8 font-mono text-xs" />
            {/if}
            {#if needsValue(c.kind)}
              <Input bind:value={c.value} placeholder="text to match" class="h-8 font-mono text-xs" />
            {/if}
            {#if needsThreshold(c.kind)}
              <Input bind:value={c.price} placeholder="price threshold (USD) — e.g. 50.00" class="h-8 font-mono text-xs" />
            {/if}
            {#if isPrice(c.kind)}
              <Input bind:value={c.price_selector} placeholder="price selector (optional)" class="h-8 font-mono text-xs" />
            {/if}
          </div>
        {/each}
      </div>

      {#if error}
        <p class="font-mono text-xs text-red-400">{error}</p>
      {/if}
    </div>

    <Dialog.Footer>
      <Button
        variant="ghost"
        onclick={() => {
          reset();
          open = false;
        }}
      >
        Cancel
      </Button>
      <Button onclick={submit} disabled={add.isPending}>
        {add.isPending ? 'adding…' : 'add target'}
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
