import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';
export default {
  preprocess: vitePreprocess(),
  onwarn: (w, handler) => {
    // Pages intentionally read route props once at mount (they are re-created per route).
    if (w.code === 'state_referenced_locally' || w.code?.startsWith('a11y_')) return;
    handler(w);
  }
};
