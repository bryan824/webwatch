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

<div class="instrument-grid flex h-screen flex-col bg-background">
  <Toolbar onOpenToken={() => (tokenOpen = true)} {updatedLabel} />
  <div class="grid flex-1 grid-cols-[300px_1fr] overflow-hidden">
    <aside class="overflow-hidden border-r border-border/70 bg-card/30">
      {#if targets.isPending}
        <div class="space-y-2 p-3">
          {#each Array.from({ length: 5 }) as _}<Skeleton class="h-11 w-full" />{/each}
        </div>
      {:else if targets.error}
        <div class="p-4">
          <p class="break-words font-mono text-xs text-red-400">{(targets.error as Error).message}</p>
          <button
            class="mt-3 font-mono text-xs text-primary underline-offset-2 hover:underline"
            onclick={() => targets.refetch()}
          >
            retry
          </button>
          {#if !$hasToken}
            <button
              class="mt-2 block font-mono text-xs text-primary underline-offset-2 hover:underline"
              onclick={() => (tokenOpen = true)}
            >
              enter API token
            </button>
          {/if}
        </div>
      {:else if (targets.data ?? []).length === 0}
        <p class="p-4 font-mono text-xs text-muted-foreground">
          no targets yet — use <span class="text-primary">+ target</span> to add one.
        </p>
      {:else}
        <TargetList targets={targets.data ?? []} {selectedId} />
      {/if}
    </aside>
    <main class="overflow-auto">{@render children()}</main>
  </div>
</div>
<TokenDialog bind:open={tokenOpen} />
