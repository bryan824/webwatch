<!-- web/src/lib/components/StatusDot.svelte -->
<script lang="ts">
  import type { StatusKind } from '$lib/status';
  let { kind }: { kind: StatusKind } = $props();
  const cls: Record<StatusKind, string> = {
    matched: 'bg-primary shadow-[0_0_9px_-2px_var(--primary)]',
    no_match: 'bg-muted-foreground/50',
    unknown: 'bg-amber-400 shadow-[0_0_9px_-2px_oklch(0.8_0.14_80)]',
    error: 'bg-red-500 shadow-[0_0_9px_-2px_oklch(0.62_0.21_25)]'
  };
</script>

<span class={`inline-block h-2 w-2 shrink-0 rounded-full ${cls[kind]}`} aria-hidden="true"></span>
