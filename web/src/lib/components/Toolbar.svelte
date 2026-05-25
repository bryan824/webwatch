<!-- web/src/lib/components/Toolbar.svelte -->
<script lang="ts">
  import { Button } from '$lib/components/ui/button';
  import * as AlertDialog from '$lib/components/ui/alert-dialog';
  import ThemeToggle from './ThemeToggle.svelte';
  import { createReloadMutation, createNotifyMutation } from '$lib/api/mutations';

  let { onOpenToken, updatedLabel }:
    { onOpenToken: () => void; updatedLabel: string } = $props();

  const reload = createReloadMutation();
  const notify = createNotifyMutation();
  let confirmOpen = $state(false);
</script>

<header class="flex h-12 items-center justify-between border-b border-border/70 bg-card/40 px-4 backdrop-blur">
  <div class="flex items-center gap-2.5">
    <span class="relative flex h-2 w-2" aria-hidden="true">
      <span class="absolute inline-flex h-full w-full animate-ping rounded-full bg-primary opacity-60"></span>
      <span class="relative inline-flex h-2 w-2 rounded-full bg-primary"></span>
    </span>
    <span class="font-mono text-sm font-semibold tracking-tight">webwatch</span>
    <span class="hidden font-mono text-[11px] text-muted-foreground sm:inline">/ updated {updatedLabel}</span>
  </div>
  <div class="flex items-center gap-1.5">
    <Button
      variant="outline"
      size="sm"
      class="h-8 font-mono text-xs"
      disabled={reload.isPending}
      onclick={() => reload.mutate()}
    >
      {reload.isPending ? 'reloading…' : 'reload'}
    </Button>
    <Button size="sm" class="h-8 font-mono text-xs" onclick={() => (confirmOpen = true)}>send report</Button>
    <Button
      variant="ghost"
      size="icon"
      class="h-8 w-8 text-muted-foreground hover:text-foreground"
      aria-label="Settings"
      onclick={onOpenToken}
    >
      {'⚙'}
    </Button>
    <ThemeToggle />
  </div>
</header>

<AlertDialog.Root bind:open={confirmOpen}>
  <AlertDialog.Content>
    <AlertDialog.Header>
      <AlertDialog.Title class="font-mono tracking-tight">Send Discord status report?</AlertDialog.Title>
      <AlertDialog.Description>
        This re-checks every enabled target and posts one report to Discord. It can take a while.
      </AlertDialog.Description>
    </AlertDialog.Header>
    <AlertDialog.Footer>
      <AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
      <AlertDialog.Action disabled={notify.isPending} onclick={() => notify.mutate()}>
        {notify.isPending ? 'sending…' : 'send report'}
      </AlertDialog.Action>
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>
