import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vitest/config';

const API_PATHS = ['/health', '/targets', '/notify'];

export default defineConfig({
  plugins: [sveltekit()],
  server: {
    proxy: Object.fromEntries(
      API_PATHS.map((p) => [p, { target: 'http://127.0.0.1:3000', changeOrigin: true }])
    )
  },
  test: {
    environment: 'jsdom',
    setupFiles: ['src/test/setup.ts'],
    globals: true
  }
});
