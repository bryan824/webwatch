<!-- web/src/lib/components/TargetDetail.svelte -->
<script lang="ts">
  import { Button } from '$lib/components/ui/button';
  import { Separator } from '$lib/components/ui/separator';
  import StatusBadge from './StatusBadge.svelte';
  import ConditionResultRow from './ConditionResultRow.svelte';
  import { deriveStatus } from '$lib/status';
  import { formatPrice, formatRelative } from '$lib/format';
  import type { TargetStatus } from '$lib/api/types';

  let { target, checking, onCheckNow }:
    { target: TargetStatus; checking: boolean; onCheckNow: () => void } = $props();
  const s = $derived(deriveStatus(target));
</script>

<div class="flex h-full flex-col gap-4 p-4">
  <div class="flex items-start justify-between gap-4">
    <div class="min-w-0">
      <h1 class="truncate text-xl font-semibold">{target.name}</h1>
      <a href={target.url} target="_blank" rel="noreferrer" class="truncate text-sm text-muted-foreground underline">{target.url}</a>
    </div>
    <Button onclick={onCheckNow} disabled={checking}>{checking ? 'Checking…' : 'Check now'}</Button>
  </div>

  <div class="flex flex-wrap items-center gap-4 text-sm">
    <StatusBadge {target} />
    <span class="text-muted-foreground">engine: <span class="text-foreground">{target.engine_used ?? '—'}</span></span>
    <span class="text-muted-foreground">price: <span class="text-foreground">{formatPrice(target.price_cents)}</span></span>
    <span class="text-muted-foreground">last success: <span class="text-foreground">{formatRelative(target.last_success_at)}</span></span>
    <span class="text-muted-foreground">last alert: <span class="text-foreground">{formatRelative(target.last_alert_at)}</span></span>
  </div>

  {#if s.kind === 'error' && target.last_error}
    <div class="rounded-md border border-red-300 bg-red-50 p-3 text-sm text-red-700 dark:bg-red-950/40">
      {target.last_error} <span class="text-xs opacity-70">({formatRelative(target.last_error_at)})</span>
    </div>
  {/if}

  {#if s.kind === 'unknown' && target.condition_results.length === 0}
    <p class="text-sm text-muted-foreground">Not checked yet. Click <strong>Check now</strong> to evaluate this target.</p>
  {:else}
    <Separator />
    <section>
      <h2 class="mb-1 text-xs font-medium uppercase tracking-wide text-muted-foreground">Evidence</h2>
      {#if target.evidence.length}
        {#each target.evidence as e}<p class="text-sm">{e}</p>{/each}
      {:else}<p class="text-sm text-muted-foreground">No evidence.</p>{/if}
    </section>
    <section>
      <h2 class="mb-1 text-xs font-medium uppercase tracking-wide text-muted-foreground">Conditions</h2>
      {#each target.condition_results as c (c.condition_id)}<ConditionResultRow {c} />{/each}
      {#if target.condition_results.length === 0}<p class="text-sm text-muted-foreground">No condition results.</p>{/if}
    </section>
  {/if}
</div>
