<!-- web/src/lib/components/TargetList.svelte -->
<script lang="ts">
  import { Input } from '$lib/components/ui/input';
  import { ScrollArea } from '$lib/components/ui/scroll-area';
  import TargetListItem from './TargetListItem.svelte';
  import { deriveStatus } from '$lib/status';
  import type { TargetStatus } from '$lib/api/types';

  let { targets, selectedId }: { targets: TargetStatus[]; selectedId?: string } = $props();
  let q = $state('');

  const filtered = $derived(
    targets.filter((t) => `${t.name} ${t.url}`.toLowerCase().includes(q.toLowerCase()))
  );
  const matched = $derived(targets.filter((t) => deriveStatus(t).kind === 'matched').length);
  const errored = $derived(targets.filter((t) => deriveStatus(t).kind === 'error').length);
</script>

<div class="flex h-full flex-col">
  <div class="space-y-2 p-3">
    <Input placeholder="search targets…" bind:value={q} class="h-8 font-mono text-xs" />
    <p class="flex items-center gap-2 px-0.5 font-mono text-[11px] uppercase tracking-wider text-muted-foreground">
      <span>{targets.length} targets</span>
      <span class="text-primary">{matched} matched</span>
      {#if errored > 0}<span class="text-red-400">{errored} err</span>{/if}
    </p>
  </div>
  <ScrollArea class="flex-1">
    <div class="flex flex-col pb-3">
      {#each filtered as t, i (t.target_id)}
        <TargetListItem target={t} selected={t.target_id === selectedId} index={i} />
      {/each}
      {#if filtered.length === 0}
        <p class="px-3 py-8 text-center font-mono text-xs text-muted-foreground">no matching targets</p>
      {/if}
    </div>
  </ScrollArea>
</div>
