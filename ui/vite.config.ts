import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  plugins: [tailwindcss(), svelte()],
  server: {
    port: 5173,
    proxy: {
      '/api': 'http://127.0.0.1:4180',
      '/view': 'http://127.0.0.1:4180',
      '/online': 'http://127.0.0.1:4180',
      '/download': 'http://127.0.0.1:4180'
    }
  },
  build: { outDir: 'build', emptyOutDir: true, chunkSizeWarningLimit: 900 }
});
