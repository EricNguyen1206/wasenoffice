import { defineConfig } from 'vite';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  plugins: [tailwindcss()],
  // tauri.conf.json build.devUrl expects this exact port
  server: {
    port: 1420,
    strictPort: true,
    // engines/ (AGPL artifacts) and upstream/ (read-only submodules) are not
    // frontend sources — keep the dev server from crawling them.
    watch: { ignored: ['**/engines/**', '**/upstream/**'] },
  },
  // three entries: the engine start page + the V01 quick-view page (no shim)
  // + the L01 Nano Light editor page (no shim either)
  build: {
    rollupOptions: {
      input: { main: 'index.html', viewer: 'viewer.html', editor: 'editor.html' },
    },
  },
  optimizeDeps: {
    entries: ['index.html', 'src/**/*.ts', 'src/**/*.js'],
  },
});
