<script lang="ts">
  import type { Snippet } from 'svelte';
  import { page } from '$app/state';
  import { createTargetsQuery } from '$lib/api/queries';
  import { hasToken } from '$lib/stores/token';
  import { ApiError } from '$lib/api/client';
  import { formatRelative } from '$lib/format';
  import Toolbar from './Toolbar.svelte';
  import TargetList from './TargetList.svelte';
  import TokenDialog from './TokenDialog.svelte';
  import { Skeleton } from '$lib/components/ui/skeleton';

  let { children }: { children: Snippet } = $props();
  let tokenOpen = $state(false);

  const targets = createTargetsQuery();
  const selectedId = $derived(page.params.id);
  const updatedLabel = $derived(
    formatRelative(targets.dataUpdatedAt ? new Date(targets.dataUpdatedAt).toISOString() : null)
  );

  $effect(() => {
    const err = targets.error;
    if (err instanceof ApiError && err.status === 401) tokenOpen = true;
  });
</script>

<div class="flex h-screen flex-col">
  <Toolbar onOpenToken={() => (tokenOpen = true)} {updatedLabel} />
  <div class="grid flex-1 grid-cols-[320px_1fr] overflow-hidden">
    <aside class="overflow-hidden border-r">
      {#if targets.isPending}
        <div class="space-y-2 p-3">{#each Array.from({ length: 4 }) as _}<Skeleton class="h-10 w-full" />{/each}</div>
      {:else if targets.error}
        <div class="p-4 text-sm">
          <p class="text-red-600">{(targets.error as Error).message}</p>
          <button class="mt-2 underline" onclick={() => targets.refetch()}>Retry</button>
          {#if !$hasToken}<button class="mt-2 block underline" onclick={() => (tokenOpen = true)}>Enter API token</button>{/if}
        </div>
      {:else if (targets.data ?? []).length === 0}
        <p class="p-4 text-sm text-muted-foreground">No targets. Edit <code>targets.toml</code> then Reload.</p>
      {:else}
        <TargetList targets={targets.data ?? []} {selectedId} />
      {/if}
    </aside>
    <main class="overflow-auto">{@render children()}</main>
  </div>
</div>
<TokenDialog bind:open={tokenOpen} />
