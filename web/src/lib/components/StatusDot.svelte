<!-- web/src/lib/components/StatusDot.svelte -->
<script lang="ts">
  import type { StatusKind } from '$lib/status';
  let { kind }: { kind: StatusKind } = $props();
  const color: Record<StatusKind, string> = {
    matched: 'bg-green-500',
    no_match: 'bg-muted-foreground',
    unknown: 'bg-amber-500',
    error: 'bg-red-500'
  };
</script>

<span class={`inline-block h-2.5 w-2.5 rounded-full ${color[kind]}`} aria-hidden="true"></span>
