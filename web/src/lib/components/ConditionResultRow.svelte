<!-- web/src/lib/components/ConditionResultRow.svelte -->
<script lang="ts">
  import { formatPrice } from '$lib/format';
  import type { ConditionResult } from '$lib/api/types';
  let { c }: { c: ConditionResult } = $props();
</script>

<div class="flex items-start gap-2.5 py-2">
  <span class={`mt-0.5 text-sm ${c.matched ? 'text-primary' : 'text-muted-foreground/70'}`}>
    {c.matched ? '✓' : '○'}
  </span>
  <div class="min-w-0 flex-1">
    <div class="flex items-center gap-2">
      <span class="rounded bg-muted px-1.5 py-0.5 font-mono text-[11px] text-foreground/80">{c.kind}</span>
      {#if c.observed_price_cents !== null}
        <span class="font-mono text-xs text-muted-foreground">{formatPrice(c.observed_price_cents)}</span>
      {/if}
    </div>
    {#each c.evidence as e}
      <div class="mt-1 break-words font-mono text-xs text-muted-foreground">{e}</div>
    {/each}
    {#if c.error}
      <div class="mt-1 break-words font-mono text-xs text-red-400">{c.error}</div>
    {/if}
  </div>
</div>
