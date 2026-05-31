<!-- web/src/lib/components/AddTargetDialog.svelte -->
<script lang="ts">
  import * as Dialog from '$lib/components/ui/dialog';
  import { Input } from '$lib/components/ui/input';
  import { Button } from '$lib/components/ui/button';
  import { createAddTargetMutation } from '$lib/api/mutations';
  import type { ConditionWireKind } from '$lib/api/types';
  import {
    CONDITION_KINDS,
    blankAddTargetDefaults,
    blankConditionDraft,
    buildTargetInput,
    fieldsForCondition,
    type AddTargetConditionDraftWithId
  } from './addTargetForm';

  let { open = $bindable(false) }: { open?: boolean } = $props();

  const add = createAddTargetMutation();

  type Draft = AddTargetConditionDraftWithId;
  const nextDraftId = () => Date.now() + Math.random();
  const defaults = blankAddTargetDefaults(nextDraftId());

  let name = $state(defaults.name);
  let url = $state(defaults.url);
  let enabled = $state(defaults.enabled);
  let interval = $state(defaults.interval);
  let conditions = $state<Draft[]>(defaults.conditions);
  let error = $state('');


  function addCondition() {
    conditions = [...conditions, blankConditionDraft(nextDraftId())];
  }
  function removeCondition(id: number) {
    conditions = conditions.filter((c) => c.id !== id);
  }
  function reset() {
    const defaults = blankAddTargetDefaults(nextDraftId());
    name = defaults.name;
    url = defaults.url;
    enabled = defaults.enabled;
    interval = defaults.interval;
    conditions = defaults.conditions;
    error = '';
  }

  function submit() {
    error = '';
    const result = buildTargetInput({ name, url, enabled, interval, conditions });
    if (!result.ok) {
      error = result.error;
      return;
    }

    add.mutate(result.input, {
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
                {#each CONDITION_KINDS as k}<option value={k.value}>{k.label}</option>{/each}
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
            {#if fieldsForCondition(c.kind).selector}
              <Input bind:value={c.selector} placeholder="CSS selector — e.g. .price" class="h-8 font-mono text-xs" />
            {/if}
            {#if fieldsForCondition(c.kind).value}
              <Input bind:value={c.value} placeholder="text to match" class="h-8 font-mono text-xs" />
            {/if}
            {#if fieldsForCondition(c.kind).threshold}
              <Input bind:value={c.price} placeholder="price threshold (USD) — e.g. 50.00" class="h-8 font-mono text-xs" />
            {/if}
            {#if fieldsForCondition(c.kind).priceSelector}
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
