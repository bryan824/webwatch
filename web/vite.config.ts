import { defineConfig } from 'vite';
import solid from 'vite-plugin-solid';

const API_PATHS = ['/health', '/targets', '/notify'];

export default defineConfig({
  plugins: [solid()],
  server: {
    proxy: Object.fromEntries(
      API_PATHS.map((p) => [p, { target: 'http://127.0.0.1:3000', changeOrigin: true }])
    ),
  },
  build: {
    outDir: 'build',
    emptyOutDir: true,
  },
});
