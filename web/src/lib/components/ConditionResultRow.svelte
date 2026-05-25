<!-- web/src/lib/components/ConditionResultRow.svelte -->
<script lang="ts">
  import { formatPrice } from '$lib/format';
  import type { ConditionResult } from '$lib/api/types';
  let { c }: { c: ConditionResult } = $props();
</script>

<div class="flex items-start gap-2 py-1 text-sm">
  <span class={c.matched ? 'text-green-600' : 'text-muted-foreground'}>{c.matched ? '✓' : '○'}</span>
  <div class="flex-1">
    <span class="font-mono text-xs">{c.kind}</span>
    {#if c.observed_price_cents !== null}<span class="text-muted-foreground"> · {formatPrice(c.observed_price_cents)}</span>{/if}
    {#each c.evidence as e}<div class="text-xs text-muted-foreground">{e}</div>{/each}
    {#if c.error}<div class="text-xs text-red-600">{c.error}</div>{/if}
  </div>
</div>
