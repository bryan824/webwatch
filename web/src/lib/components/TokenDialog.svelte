<!-- web/src/lib/components/TokenDialog.svelte -->
<script lang="ts">
  import * as Dialog from '$lib/components/ui/dialog';
  import { Input } from '$lib/components/ui/input';
  import { Button } from '$lib/components/ui/button';
  import { token, setToken, clearToken } from '$lib/stores/token';

  let { open = $bindable(false) }: { open?: boolean } = $props();
  let value = $state($token ?? '');

  function save() { setToken(value); open = false; }
  function forget() { clearToken(); value = ''; }
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="sm:max-w-md">
    <Dialog.Header>
      <Dialog.Title class="font-mono tracking-tight">Authentication</Dialog.Title>
      <Dialog.Description>
        Paste your <code class="font-mono text-xs">WEBWATCH_API_TOKEN</code>. Stored in this browser only.
      </Dialog.Description>
    </Dialog.Header>
    <label class="font-mono text-xs uppercase tracking-wider text-muted-foreground" for="token-input">
      API token
    </label>
    <Input id="token-input" type="password" bind:value placeholder="bearer token…" class="font-mono" />
    <Dialog.Footer>
      <Button variant="ghost" onclick={forget}>Forget</Button>
      <Button onclick={save}>Save token</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
