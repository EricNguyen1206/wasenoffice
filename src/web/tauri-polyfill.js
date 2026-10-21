/**
 * Web dev mode (T-068, DD docs/design/web-dev-mode/DD.md) — Tauri IPC polyfill.
 *
 * Vite injects this inline ahead of every entry's module script in `serve`
 * only (vite.config.ts `webPolyfill`). Under Tauri the real IPC is already
 * installed → the first line returns and nothing changes. In a plain browser
 * it installs the two globals the app code reads:
 *   - `window.__TAURI_INTERNALS__.invoke` — what `@tauri-apps/api` calls lazily
 *   - `window.__TAURI__.core.invoke`        — what viewer.ts / editor.ts and the
 *     shim (`tauriInvoke`) read directly
 * and routes every invoke to `POST /invoke/<cmd>` (vite proxies it to the
 * loopback file server; `src-tauri/src/web_bridge.rs` dispatches).
 *
 * A browser tab stands in for a Tauri window: its label is `?win=<label>`
 * (surface pages opened by the bridge) or a per-tab `web-<rand>` kept in
 * sessionStorage. A result of `{webOpen:{url}}` means "Rust would have opened
 * a window" → open that page in a new tab, resolve undefined like the Tauri
 * command does. Plugin commands that only make sense natively are answered
 * locally (listed in LOCAL). Classic script on purpose: no imports, runs
 * before modules.
 */
(function () {
  if (window.__TAURI_INTERNALS__) return;

  var label = new URLSearchParams(location.search).get('win');
  if (!label) {
    label = sessionStorage.getItem('wasen.web.label');
    if (!label) {
      // Tab identity only (the bridge's gate is the Origin allowlist) — a UUID
      // slice keeps labels unique across tabs opened in the same second.
      label = 'web-' + crypto.randomUUID().slice(0, 8);
      sessionStorage.setItem('wasen.web.label', label);
    }
  }

  /** Plugin commands answered in the page — no native counterpart in web mode. */
  var LOCAL = {
    'plugin:event|listen': function () { return 0; },
    'plugin:event|unlisten': function () {},
    'plugin:window|close': function () { window.close(); },
    'plugin:webview|close': function () { window.close(); },
    'plugin:dialog|open': function () {
      throw "web mode: native picker unavailable — use 'or paste a path…'";
    },
    'plugin:updater|check': function () { throw 'web mode: updater unavailable'; },
  };

  function invoke(cmd, args) {
    if (Object.prototype.hasOwnProperty.call(LOCAL, cmd)) {
      return Promise.resolve().then(function () { return LOCAL[cmd](args); });
    }
    return fetch('/invoke/' + encodeURIComponent(cmd), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'X-Wasen-Window': label },
      body: JSON.stringify(args || {}),
    })
      .then(function (r) {
        if (!r.ok) throw 'web bridge: HTTP ' + r.status + ' for ' + cmd + ' (is the app running with WASEN_OFFICE_WEB_BRIDGE=1?)';
        return r.json();
      })
      .then(function (res) {
        // Tauri rejects with the command's Err value itself (a string here).
        if (!res.ok) throw res.error;
        var out = res.result;
        if (out && typeof out === 'object' && out.webOpen) {
          if (!window.open(out.webOpen.url, '_blank')) location.assign(out.webOpen.url);
          return undefined;
        }
        return out;
      });
  }

  var callbacks = 0;
  window.__TAURI_INTERNALS__ = {
    invoke: invoke,
    transformCallback: function (cb) {
      var id = ++callbacks;
      window['_' + id] = cb;
      return id;
    },
    unregisterCallback: function (id) { delete window['_' + id]; },
    convertFileSrc: function (p) { return p; },
    metadata: {
      currentWindow: { label: label },
      currentWebview: { label: label, windowLabel: label },
      windows: [{ label: label }],
      webviews: [{ label: label, windowLabel: label }],
    },
  };
  window.__TAURI__ = { core: { invoke: invoke } };
  window.__WASEN_WEB_MODE__ = label;
})();
