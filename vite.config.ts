import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { defineConfig, type Plugin } from 'vite';
import tailwindcss from '@tailwindcss/vite';

/**
 * Web dev mode (T-068, DD W01): in `vite` (serve) ONLY, every entry gets the
 * Tauri IPC polyfill ahead of its module script, so a plain browser drives the
 * Rust command layer over `/invoke` (proxied below). `pnpm build` never sees
 * it — the shipped dist is unchanged; under Tauri the polyfill is a no-op.
 */
function webPolyfill(): Plugin {
  const src = fileURLToPath(new URL('./src/web/tauri-polyfill.js', import.meta.url));
  return {
    name: 'wasen-web-polyfill',
    apply: 'serve',
    transformIndexHtml() {
      return [{ tag: 'script', children: readFileSync(src, 'utf8'), injectTo: 'head-prepend' }];
    },
  };
}

export default defineConfig({
  plugins: [tailwindcss(), webPolyfill()],
  // tauri.conf.json build.devUrl expects this exact port
  server: {
    port: 1420,
    strictPort: true,
    // engines/ (AGPL artifacts) and upstream/ (read-only submodules) are not
    // frontend sources — keep the dev server from crawling them.
    watch: { ignored: ['**/engines/**', '**/upstream/**'] },
    // T-068: the polyfill's `POST /invoke/<cmd>` → the loopback file server's
    // web bridge (same-origin for the page, so no CORS dance in the browser).
    // Override the port when filesrv could not bind its default 14321.
    proxy: { '/invoke': `http://127.0.0.1:${process.env.WASEN_OFFICE_BRIDGE_PORT ?? 14321}` },
  },
  // entries: the engine start page + the V01 quick-view page (no shim)
  // + the L01 Nano Light editor page (no shim either); settings lives INSIDE
  // the start page since T-A46 (K02-Q3 amended — no separate window)
  build: {
    rollupOptions: {
      input: { main: 'index.html', viewer: 'viewer.html', editor: 'editor.html' },
    },
  },
  optimizeDeps: {
    include: [
      '@tauri-apps/api/core',
      '@tauri-apps/api/event',
      '@tauri-apps/api/window',
      '@tauri-apps/api/app',
      '@tauri-apps/plugin-dialog',
      '@tauri-apps/plugin-updater',
    ],
    entries: ['index.html', 'src/**/*.ts', 'src/**/*.js'],
  },
});
