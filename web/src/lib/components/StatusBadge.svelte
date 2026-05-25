<!-- web/src/lib/components/StatusBadge.svelte -->
<script lang="ts">
  import { Badge } from '$lib/components/ui/badge';
  import StatusDot from './StatusDot.svelte';
  import { deriveStatus } from '$lib/status';
  import type { TargetStatus } from '$lib/api/types';

  let { target }: { target: TargetStatus } = $props();
  const s = $derived(deriveStatus(target));
</script>

<Badge
  variant="outline"
  class="gap-1.5 border-border/70 font-mono text-[10px] font-medium uppercase tracking-[0.14em]"
>
  <StatusDot kind={s.kind} />
  {s.label}
</Badge>
