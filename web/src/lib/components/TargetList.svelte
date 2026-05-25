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

<div class="flex h-full flex-col gap-2 p-2">
  <Input placeholder="Search targets…" bind:value={q} />
  <p class="px-1 text-xs text-muted-foreground">
    {targets.length} targets · {matched} matched · {errored} error
  </p>
  <ScrollArea class="flex-1">
    <div class="flex flex-col gap-0.5">
      {#each filtered as t (t.target_id)}
        <TargetListItem target={t} selected={t.target_id === selectedId} />
      {/each}
      {#if filtered.length === 0}
        <p class="px-2 py-6 text-center text-sm text-muted-foreground">No matching targets.</p>
      {/if}
    </div>
  </ScrollArea>
</div>
