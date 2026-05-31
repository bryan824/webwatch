<!-- web/src/lib/components/TargetDetail.svelte -->
<script lang="ts">
  import { Button } from '$lib/components/ui/button';
  import * as AlertDialog from '$lib/components/ui/alert-dialog';
  import StatusBadge from './StatusBadge.svelte';
  import ConditionResultRow from './ConditionResultRow.svelte';
  import { deriveStatus } from '$lib/status';
  import { formatPrice, formatRelative } from '$lib/format';
  import type { TargetStatus } from '$lib/api/types';

  let { target, checking, mutating, onCheckNow, onToggleEnabled, onDelete }: {
    target: TargetStatus;
    checking: boolean;
    mutating: boolean;
    onCheckNow: () => void;
    onToggleEnabled: () => void;
    onDelete: () => void;
  } = $props();

  let confirmDelete = $state(false);
  const s = $derived(deriveStatus(target));
  const meta = $derived<[string, string][]>([
    ['engine', target.engine_used ?? '—'],
    ['price', formatPrice(target.price_cents)],
    ['last success', formatRelative(target.last_success_at)],
    ['last alert', formatRelative(target.last_alert_at)]
  ]);
</script>

<div class="mx-auto flex h-full max-w-3xl animate-in flex-col gap-6 p-6 fade-in duration-300">
  <div class="flex items-start justify-between gap-4">
    <div class="min-w-0">
      <div class="flex items-center gap-2">
        <h1 class="truncate font-mono text-xl font-semibold tracking-tight">{target.name}</h1>
        {#if !target.enabled}
          <span class="shrink-0 rounded border border-border px-1.5 py-0.5 font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
            disabled
          </span>
        {/if}
      </div>
      <a
        href={target.url}
        target="_blank"
        rel="noreferrer"
        class="mt-1 inline-block max-w-full truncate font-mono text-xs text-muted-foreground hover:text-primary"
      >
        {target.url} ↗
      </a>
    </div>
    <div class="flex shrink-0 items-center gap-2">
      <Button variant="outline" class="font-mono text-xs" onclick={onToggleEnabled} disabled={mutating}>
        {target.enabled ? 'disable' : 'enable'}
      </Button>
      <Button class="font-mono text-xs" onclick={onCheckNow} disabled={checking}>
        {checking ? 'checking…' : 'check now'}
      </Button>
      <Button
        variant="ghost"
        class="font-mono text-xs text-red-400 hover:text-red-300"
        onclick={() => (confirmDelete = true)}
        disabled={mutating}
      >
        delete
      </Button>
    </div>
  </div>

  <StatusBadge {target} />

  {#if s.kind === 'error' && target.last_error}
    <div class="rounded-md border border-red-500/30 bg-red-500/10 p-3 font-mono text-xs text-red-300">
      {target.last_error}
      <span class="opacity-60"> · {formatRelative(target.last_error_at)}</span>
    </div>
  {/if}

  {#if s.kind === 'unknown' && target.condition_results.length === 0}
    <p class="font-mono text-sm text-muted-foreground">
      not checked yet — click <span class="text-primary">check now</span> to evaluate this target.
    </p>
  {:else}
    <div class="grid grid-cols-2 gap-px overflow-hidden rounded-md border border-border/70 bg-border/70 sm:grid-cols-4">
      {#each meta as [label, value]}
        <div class="bg-card p-3">
          <div class="font-mono text-[10px] uppercase tracking-wider text-muted-foreground">{label}</div>
          <div class="mt-1 truncate font-mono text-sm">{value}</div>
        </div>
      {/each}
    </div>

    <section>
      <h2 class="mb-2 font-mono text-[10px] uppercase tracking-[0.18em] text-muted-foreground">Evidence</h2>
      {#if target.evidence.length}
        <div class="space-y-1 rounded-md border border-border/70 bg-card/50 p-3">
          {#each target.evidence as e}<p class="break-words font-mono text-xs text-foreground/90">{e}</p>{/each}
        </div>
      {:else}
        <p class="font-mono text-xs text-muted-foreground">no evidence</p>
      {/if}
    </section>

    <section>
      <h2 class="mb-1 font-mono text-[10px] uppercase tracking-[0.18em] text-muted-foreground">Conditions</h2>
      <div class="divide-y divide-border/60">
        {#each target.condition_results as c (c.condition_id)}<ConditionResultRow {c} />{/each}
      </div>
      {#if target.condition_results.length === 0}
        <p class="font-mono text-xs text-muted-foreground">no condition results</p>
      {/if}
    </section>
  {/if}
</div>

<AlertDialog.Root bind:open={confirmDelete}>
  <AlertDialog.Content>
    <AlertDialog.Header>
      <AlertDialog.Title class="font-mono tracking-tight">Delete {target.name}?</AlertDialog.Title>
      <AlertDialog.Description>
        Removes the target and its check history. This cannot be undone.
      </AlertDialog.Description>
    </AlertDialog.Header>
    <AlertDialog.Footer>
      <AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
      <AlertDialog.Action onclick={onDelete} disabled={mutating}>
        {mutating ? 'deleting…' : 'delete'}
      </AlertDialog.Action>
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>
