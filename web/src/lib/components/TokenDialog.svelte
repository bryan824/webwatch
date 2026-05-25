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
  <Dialog.Content>
    <Dialog.Header>
      <Dialog.Title>Authentication</Dialog.Title>
      <Dialog.Description>Paste your WEBWATCH_API_TOKEN. Stored in this browser only.</Dialog.Description>
    </Dialog.Header>
    <label class="text-sm font-medium" for="token-input">API token</label>
    <Input id="token-input" type="password" bind:value placeholder="Bearer token" />
    <Dialog.Footer>
      <Button variant="ghost" onclick={forget}>Forget</Button>
      <Button onclick={save}>Save</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
