<!-- web/src/lib/components/TargetListItem.svelte -->
<script lang="ts">
  import StatusDot from './StatusDot.svelte';
  import { deriveStatus } from '$lib/status';
  import { formatRelative } from '$lib/format';
  import type { TargetStatus } from '$lib/api/types';

  let { target, selected }: { target: TargetStatus; selected: boolean } = $props();
  const s = $derived(deriveStatus(target));
</script>

<a
  href={`/targets/${encodeURIComponent(target.target_id)}`}
  data-sveltekit-noscroll
  class={`flex items-center gap-2 rounded-md px-2.5 py-2 text-sm transition-colors hover:bg-muted ${selected ? 'bg-muted ring-1 ring-primary' : ''}`}
  aria-current={selected ? 'page' : undefined}
>
  <StatusDot kind={s.kind} />
  <span class="flex-1 min-w-0">
    <span class="block truncate font-medium">{target.name}</span>
    <span class="block truncate text-xs text-muted-foreground">
      {s.kind === 'error' ? 'error ' : ''}{formatRelative(target.last_success_at ?? target.last_error_at)}
    </span>
  </span>
</a>
