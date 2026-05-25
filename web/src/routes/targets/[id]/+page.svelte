<script lang="ts">
  import { page } from '$app/state';
  import { createTargetsQuery } from '$lib/api/queries';
  import { createCheckNowMutation } from '$lib/api/mutations';
  import TargetDetail from '$lib/components/TargetDetail.svelte';

  const targets = createTargetsQuery();
  const check = createCheckNowMutation();
  const id = $derived(page.params.id);
  const target = $derived((targets.data ?? []).find((t) => t.target_id === id));
</script>

{#if target}
  <TargetDetail {target} checking={check.isPending} onCheckNow={() => check.mutate(target.target_id)} />
{:else if targets.isPending}
  <div class="p-4 text-sm text-muted-foreground">Loading…</div>
{:else}
  <div class="p-4 text-sm text-muted-foreground">Target <code>{id}</code> not found.</div>
{/if}
