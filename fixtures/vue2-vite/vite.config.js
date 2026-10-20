import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue2';

export default defineConfig({
  base: '/vue2/',
  plugins: [vue()],
  // Prebundling moves import.meta.url without moving the Worker/WASM assets.
  optimizeDeps: {
    exclude: ['viewer', 'viewer-core', 'viewer-word', 'viewer-excel', 'viewer-ppt', 'viewer-pdf']
      .map(name => `@office-view-suite/${name}`),
  },
  build: { target: 'es2022' },
});
