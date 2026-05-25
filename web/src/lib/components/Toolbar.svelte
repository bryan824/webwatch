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

<header class="flex items-center justify-between border-b px-4 py-2">
  <div class="flex items-center gap-3">
    <strong>webwatch</strong>
    <span class="text-xs text-muted-foreground">updated {updatedLabel}</span>
  </div>
  <div class="flex items-center gap-2">
    <Button variant="outline" size="sm" disabled={reload.isPending} onclick={() => reload.mutate()}>
      {reload.isPending ? 'Reloading…' : 'Reload'}
    </Button>
    <Button size="sm" onclick={() => (confirmOpen = true)}>Send report</Button>
    <Button variant="ghost" size="icon" aria-label="Settings" onclick={onOpenToken}>⚙</Button>
    <ThemeToggle />
  </div>
</header>

<AlertDialog.Root bind:open={confirmOpen}>
  <AlertDialog.Content>
    <AlertDialog.Header>
      <AlertDialog.Title>Send Discord status report?</AlertDialog.Title>
      <AlertDialog.Description>
        This re-checks every enabled target and posts one report to Discord. It can take a while.
      </AlertDialog.Description>
    </AlertDialog.Header>
    <AlertDialog.Footer>
      <AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
      <AlertDialog.Action disabled={notify.isPending} onclick={() => notify.mutate()}>
        {notify.isPending ? 'Sending…' : 'Send report'}
      </AlertDialog.Action>
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>
