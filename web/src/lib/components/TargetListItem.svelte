<!-- web/src/lib/components/TargetListItem.svelte -->
<script lang="ts">
  import StatusDot from './StatusDot.svelte';
  import { deriveStatus } from '$lib/status';
  import { formatRelative } from '$lib/format';
  import type { TargetStatus } from '$lib/api/types';

  let { target, selected, index = 0 }:
    { target: TargetStatus; selected: boolean; index?: number } = $props();
  const s = $derived(deriveStatus(target));
</script>

<a
  href={`/targets/${encodeURIComponent(target.target_id)}`}
  data-sveltekit-noscroll
  style={`animation-delay:${Math.min(index, 12) * 35}ms`}
  class={`group flex animate-in items-center gap-2.5 border-l-2 px-3 py-2.5 fade-in slide-in-from-left-1 fill-mode-both transition-colors ${
    selected
      ? 'border-l-primary bg-accent/60'
      : 'border-l-transparent hover:border-l-border hover:bg-accent/40'
  } ${target.enabled ? '' : 'opacity-60'}`}
  aria-current={selected ? 'page' : undefined}
>
  <StatusDot kind={s.kind} />
  <span class="min-w-0 flex-1">
    <span class={`block truncate text-sm font-medium ${selected ? 'text-foreground' : 'text-foreground/90'}`}>
      {target.name}
    </span>
    <span class="block truncate font-mono text-[11px] text-muted-foreground">
      {target.enabled ? '' : 'OFF · '}{s.kind === 'error' ? 'ERR · ' : ''}{formatRelative(target.last_success_at ?? target.last_error_at)}
    </span>
  </span>
</a>
