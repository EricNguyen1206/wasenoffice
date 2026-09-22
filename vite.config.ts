import { defineConfig } from 'vite';

export default defineConfig({
  // tauri.conf.json build.devUrl expects this exact port
  server: {
    port: 1420,
    strictPort: true,
    // engines/ (AGPL artifacts) and upstream/ (read-only submodules) are not
    // frontend sources — keep the dev server from crawling them.
    watch: { ignored: ['**/engines/**', '**/upstream/**'] },
  },
  optimizeDeps: {
    entries: ['index.html', 'src/**/*.ts', 'src/**/*.js'],
  },
});
