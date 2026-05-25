<!-- web/src/lib/components/ThemeToggle.svelte -->
<script lang="ts">
  import { Button } from '$lib/components/ui/button';
  import { toggleMode } from 'mode-watcher';
</script>

<Button
  variant="ghost"
  size="icon"
  onclick={toggleMode}
  aria-label="Toggle theme"
  class="h-8 w-8 text-muted-foreground hover:text-foreground"
>
  <span class="dark:hidden">{'☾'}</span>
  <span class="hidden dark:inline">{'☀'}</span>
</Button>
