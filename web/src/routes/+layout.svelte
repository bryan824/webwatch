<script lang="ts">
  import '../app.css';
  import { QueryClientProvider } from '@tanstack/svelte-query';
  import { ModeWatcher } from 'mode-watcher';
  import { Toaster } from 'svelte-sonner';
  import { queryClient } from '$lib/api/query-client';
  import AppFrame from '$lib/components/AppFrame.svelte';

  let { children } = $props();
</script>

<QueryClientProvider client={queryClient}>
  <AppFrame {children} />
  <Toaster richColors position="top-right" />
  <ModeWatcher />
</QueryClientProvider>
