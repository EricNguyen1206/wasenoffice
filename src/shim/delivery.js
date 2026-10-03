/**
 * Open-delivery chain (T-045, review candidate C3) — the named module for the
 * desktop open contract: the engine asks for the document (LocalStartOpen,
 * relayed by desktopEditor.js) → the MAIN frame fetches the converted bytes
 * (Rust `get_document_for_delivery`) → the bytes travel DOWN the frame chain
 * → the ENGINE frame calls window.DesktopOfflineAppDocumentEndLoad
 * (desktop-sdk client_renderer_wrapper.cpp "onlocaldocument_loadend").
 *
 * Plain script (no imports/exports), same double-duty as desktopEditor.js:
 *   1. Tauri initialization script injected into ALL frames
 *      (src-tauri/src/lib.rs — injected AFTER desktopEditor.js)
 *   2. vite side-effect import on the start page (src/main.ts — imported AFTER
 *      desktopEditor.js)
 * Both orders are load-bearing: this file consumes the shared plumbing that
 * desktopEditor.js publishes as `window.__nanooffice_shim` (tauriInvoke +
 * forwardLog). Nothing else crosses the file boundary — desktopEditor.js only
 * calls `window.__nanooffice_delivery.deliverDocument()`.
 *
 * ── PROBE GATE (mechanism — T-045) ────────────────────────────────────────
 * Every diagnostics probe below (font-oracle, canvas probes ×2, DocInfo,
 * cc-wrapper, probe decode) sits behind the single predicate `probesEnabled()`.
 * It reads `window.__NANO_OFFICE_PROBE__`, a boolean set by a one-line Rust
 * initialization script (src-tauri/src/lib.rs) built from the env var
 * NANO_OFFICE_PROBE: default ON in debug builds (dev evidence depends on it),
 * OFF in release; `NANO_OFFICE_PROBE=0` forces OFF, `=1` forces ON.
 * New diagnostics MUST go behind the same predicate — no ungated probe logs.
 *
 * ── LOG CONTRACT — FROZEN (gate 9) ────────────────────────────────────────
 * harness/scripts/smoke-render.sh (verify.sh gate 9) greps the app stdout for
 * EXACT substrings (both ends carry this cross-reference; change in the same
 * commit or the gate breaks silently):
 *   ① `asc_onDocumentContentReady` — contentReady hook below
 *   ② `viewer=<W>x<H>` in the font-oracle line (pattern
 *      `viewer=[0-9]+x[1-9]…`), fallback `canvas probe` + size in the two
 *      canvas probes (pattern `canvas probe.*[0-9]+x[1-9]…`)
 *   ③ `GET /fonts/` — Rust side only (src-tauri/src/protocol/filesrv.rs:60),
 *      not emitted by this module
 * The probes emitting ② are gated (above) but default ON in debug — the smoke
 * gate always runs a debug build (`pnpm tauri dev`), so signals stay live.
 *
 * ── SAVE-END LANE (T-010) ─────────────────────────────────────────────────
 * The same DOWN lane also carries the save result: Rust converts Editor.bin +
 * changes0.json back to the original file (x2t, params XML per the desktop
 * save contract in docs/reference.map.md) and evals
 * `window.__nanooffice_delivery.saveEnd({ok, path, code})` in the MAIN frame;
 * this module forwards it DOWN and the ENGINE frame answers the contract:
 * LocalFileSetSaved(ok) → LocalFileSetSourcePath(path) →
 * window.DesktopOfflineAppDocumentEndSave(code) (0 ok / 2 convert error). The
 * engine then refreshes its title via LocalFileGetSourcePath/SetDocumentName.
 *
 * gate 10 (harness/scripts/smoke-save.sh) greps the completion line
 * `save-end delivered to engine` from consumeSaveEnd below — keep verbatim
 * (log contract, change in the same commit as the gate).
 */
(function (window) {
  'use strict';

  if (window.__nanooffice_delivery_installed) return;
  window.__nanooffice_delivery_installed = true;

  var core = window.__nanooffice_shim;
  if (!core || typeof core.forwardLog !== 'function') {
    // Load-order contract violated — fail loudly instead of silently dropping
    // document delivery (desktopEditor.js runs first in every frame and
    // publishes window.__nanooffice_shim).
    console.error('[nanooffice-delivery] window.__nanooffice_shim missing — desktopEditor.js must be installed first');
    return;
  }
  var forwardLog = core.forwardLog;
  var tauriInvoke = core.tauriInvoke;

  // Delivery ordering as named constants (review C3: were magic numbers).
  var DELIVERY_DELAY_MS = 1500; // post-LocalStartOpen settle (the real shell converts first)
  var RETRY_INTERVAL_MS = 500;  // consume-retry poll
  var RETRY_MAX = 120;          // ×500ms ⇒ the 60s ceiling named in the abort log
  var HOP_GUARD = 6;            // DOWN hops ceiling before a payload is dropped
  // Render-probe cadence (T-050): the one-shot timers (6/8/15s) were swallowed
  // whole by WKWebView timer throttling on back-to-back boots — diagnostics
  // 30.09 saw a failed gate run with 0 probe lines. An interval is at worst
  // slowed down, never dropped; 60 × 2s stays inside the 240s gate timeout.
  var PROBE_INTERVAL_MS = 2000;
  var PROBE_MAX_TICKS = 60;
  var renderProbeTimer = null;  // T-050 repeating render-probe interval (engine frame)

  var DOWN_TYPE = '__nanooffice_down'; // main frame → engine frame (document delivery)

  /**
   * Single gate for every diagnostics probe in this module (mechanism + env
   * var documented in the header). Production delivery logs never route
   * through here.
   */
  function probesEnabled() {
    return window.__NANO_OFFICE_PROBE__ === true;
  }

  /**
   * PH-0 oracle probe: dump the same font-pipeline fields observed in the real
   * app via CDP (docs/diagnostics.md) so the two runtimes can be diffed from
   * stdout alone. T-050: no longer one-shot — called every tick of
   * startRenderProbes until the viewer has real height.
   * gate 9 signal ②: the `viewer=WxH` substring matches the smoke-render.sh
   * pattern `viewer=[0-9]+x[1-9]…` — keep verbatim.
   * T-048: build-aware — variant/glyph/registry come from the FONT_VARIANTS
   * row detected by desktopEditor.js (single home).
   * Returns true once the viewer has height>0 (signal ② proven).
   */
  function logFontOracleState() {
    try {
      var v = core.detectFontVariant ? core.detectFontVariant() : null;
      var fm = window.AscFonts;
      var reg = v ? fm[v.registry] : null;
      var mgr = v && fm ? fm[v.glyph] : null;
      var root = document.getElementById('editor_sdk') || document.body;
      var viewer = document.getElementById('id_viewer');
      var sdkBox = root.getBoundingClientRect();
      forwardLog('info', 'font-oracle: variant=' + (v ? v.loader : 'unknown')
        + ' mgr=' + (mgr ? 'yes' : 'undef')
        + ' streams=' + (reg ? reg.length : 'undef')
        + ' viewer=' + (viewer ? viewer.clientWidth + 'x' + viewer.clientHeight : 'none')
        + ' geom[inner=' + window.innerWidth + 'x' + window.innerHeight
        + ' editor_sdk=' + Math.round(sdkBox.width) + 'x' + Math.round(sdkBox.height) + ']');
      return !!viewer && viewer.clientHeight > 0;
    } catch (e) {
      forwardLog('error', 'font-oracle probe failed: ' + e);
      return false;
    }
  }

  /**
   * One canvas-probe tick. gate 9 signal ② fallback: the `canvas probe` prefix
   * matches the smoke-render.sh pattern `canvas probe.*[0-9]+x[1-9]…` — keep
   * verbatim. `detailed` is the canvas probe-2 variant (per-canvas ids + editor
   * state, same string as the pre-T-050 one-shot). Returns true once some
   * canvas has real height (render proven).
   */
  function logCanvasProbe(prefix, detailed) {
    try {
      var root = document.getElementById('editor_sdk') || document.body;
      var canvases = root.querySelectorAll('canvas');
      var info = [];
      var anyTall = false;
      for (var i = 0; i < canvases.length; i++) {
        if (canvases[i].height > 0) anyTall = true;
        if (detailed) {
          info.push('#' + i + ':' + canvases[i].id + ' ' + canvases[i].width + 'x' + canvases[i].height);
        } else if (i < 4) {
          info.push(canvases[i].width + 'x' + canvases[i].height);
        }
      }
      if (detailed) {
        var ed = window.Asc && (window.Asc.editor || window.editor);
        forwardLog('info', prefix + ' (' + root.id + '): ' + canvases.length + ' canvas(es) ' + info.join(' ')
          + ' | editor state: isDocumentContentReady=' + (ed && ed.isDocumentContentReady ? ed.isDocumentContentReady() : '?'));
      } else {
        forwardLog('info', prefix + ' (' + root.id + '): ' + canvases.length + ' canvas(es) ' + info.join(' '));
      }
      return anyTall;
    } catch (e) {
      forwardLog('error', prefix + ' failed: ' + e);
      return false;
    }
  }

  /**
   * T-050 render-probe loop: the three probes (canvas probe, font-oracle,
   * canvas probe-2) used to fire once at 6/8/15s after EndLoad and WKWebView
   * sometimes dropped them whole (0 probe lines in a failed gate run —
   * diagnostics 30.09). Now they repeat on a 2s interval, logging the EXACT
   * same strings every tick (the gate greps patterns — extra occurrences are
   * harmless); each probe stops once its own signal is proven and the loop
   * ends when all three are done or PROBE_MAX_TICKS ticks have run.
   * A second EndLoad (document reopened inside a still-running loop) restarts
   * the loop instead of being swallowed by the guard — code-review 30.09.
   */
  function startRenderProbes() {
    if (renderProbeTimer) clearInterval(renderProbeTimer);
    var ticks = 0;
    var doneCanvas = false, doneOracle = false, doneDetail = false;
    renderProbeTimer = setInterval(function () {
      ticks++;
      doneCanvas = doneCanvas || logCanvasProbe('canvas probe', false);
      doneOracle = doneOracle || logFontOracleState();
      doneDetail = doneDetail || logCanvasProbe('canvas probe-2', true);
      if ((doneCanvas && doneOracle && doneDetail) || ticks >= PROBE_MAX_TICKS) {
        clearInterval(renderProbeTimer);
        renderProbeTimer = null;
      }
    }, PROBE_INTERVAL_MS);
  }

  /** Main frame only: fetch the registered document and send it DOWN to the
   *  engine frame (which asked for it via LocalStartOpen). Called by
   *  desktopEditor.js when the LocalStartOpen relay arrives. T-059: states
   *  WHICH document — the fileId its window got from open_document (stamped
   *  by main.ts into window.__nanooffice_file_id__) — instead of letting
   *  Rust resolve an implicit "latest" registration (wrong-open race against
   *  quick-view windows). */
  function deliverDocument() {
    var invoke = tauriInvoke();
    if (!invoke) return;
    invoke('get_document_for_delivery', { fileId: window.__nanooffice_file_id__ }).then(function (doc) {
      // The real shell delivers well after LocalStartOpen (it converts first);
      // give the engine's post-LocalStartOpen init a beat to settle.
      setTimeout(function () {
        postDown({ type: DOWN_TYPE, deliver: doc });
      }, DELIVERY_DELAY_MS);
    }).catch(function (err) {
      forwardLog('error', 'document delivery failed: ' + err);
    });
  }

  /** Send a payload one hop DOWN (into every child iframe). */
  function postDown(data) {
    var frames = document.querySelectorAll('iframe');
    for (var i = 0; i < frames.length; i++) {
      try {
        data.__hop = (data.__hop || 0) + 1;
        frames[i].contentWindow.postMessage(data, '*');
      } catch (e) { /* cross-origin not expected here */ }
    }
  }

  /**
   * Readiness predicate for the offline open (named — review C3; was inline).
   * The engine api object must exist, the wasm font engine must be fully
   * initialized (the build's glyph manager silently no-ops while its wasm
   * handle is absent, so a font queued too early is never loaded), and the
   * EndLoad entry must be a function. T-048: the glyph-manager global + wasm
   * handle are minified per build — taken from the detected FONT_VARIANTS
   * row (single home in desktopEditor.js; word=Wg.vue, cell=hj.RIe,
   * slide=mi.Zie — pdf editor reuses the word build). The individual fields
   * are returned for the waiting/abort logs (kept byte-identical).
   */
  function engineReadiness() {
    var editorApi = window.Asc && (window.Asc.editor || window.editor);
    var v = core.detectFontVariant ? core.detectFontVariant() : null;
    var fm = window.AscFonts;
    var wasmReady = !!(fm && v && fm[v.glyph] && fm[v.glyph][v.wasm]);
    var endloadType = typeof window.DesktopOfflineAppDocumentEndLoad;
    return {
      ready: !!editorApi && !!wasmReady && endloadType === 'function',
      api: !!editorApi,
      wasm: !!wasmReady,
      endloadType: endloadType,
    };
  }

  /** Engine frame only: consume the delivered document via the engine's own
   *  offline-open entry (defined by sdk-all.js). The engine must pass
   *  engineReadiness() first (the real desktop app delivers late — after file
   *  conversion) — retry until ready. */
  function consumeDeliveredDocument(deliver) {
    var ready = engineReadiness();
    if (!ready.ready) {
      if (deliver.__tries === undefined) {
        deliver.__tries = 0;
        forwardLog('info', 'delivery waiting: api=' + ready.api
          + ' wasmReady=' + ready.wasm
          + ' endload=' + ready.endloadType);
      }
      if (++deliver.__tries > RETRY_MAX) {
        forwardLog('error', 'document delivery aborted: engine not ready after 60s'
          + ' (api=' + ready.api + ' wasmReady=' + ready.wasm + ')');
        return;
      }
      setTimeout(function () { consumeDeliveredDocument(deliver); }, RETRY_INTERVAL_MS);
      return;
    }
    try {
      // EndLoad expects the file as a BASE64 string + decoded byte length
      // (its $$ helper is AscCommon.zP.decode(b64, false, len)). The head
      // bytes below are only for the PH-0 evidence log.
      var bin = atob(deliver.b64);
      var decodedLen = bin.length;
      var head = [bin.charCodeAt(0), bin.charCodeAt(1), bin.charCodeAt(2), bin.charCodeAt(3)];
      var src = deliver.src || (deliver.folder + '/' + deliver.name);
      window.AscDesktopEditor.LocalFileRecoverFolder(deliver.folder);
      window.AscDesktopEditor.LocalFileSetSourcePath(src);
      window.AscDesktopEditor.LocalFileSetSaved(true);
      forwardLog('info', 'delivering document to engine: ' + deliver.name
        + ' (' + decodedLen + ' bytes, head ' + head.join(',') + ')');
      if (probesEnabled()) {
        try {
          var probe = AscCommon.zP.decode(deliver.b64, false, decodedLen);
          forwardLog('info', 'probe decode: type=' + (probe && probe.constructor && probe.constructor.name)
            + ' len=' + (probe && probe.length)
            + ' head=' + (probe ? [probe[0], probe[1], probe[2], probe[3]].join(',') : '?'));
        } catch (dbgErr) {
          forwardLog('error', 'probe decode failed: ' + dbgErr);
        }
      }
      // Definitive open-signal hooks — MUST attach BEFORE EndLoad so
      // synchronous open errors are not missed.
      var ed2 = window.Asc && (window.Asc.editor || window.editor);
      if (ed2 && ed2.attachEvent) {
        ed2.attachEvent('asc_onError', function () {
          // api emits (errorMessage, errorLevel) — log raw values
          forwardLog('error', 'asc_onError: '
            + Array.prototype.slice.call(arguments).map(String).join(' | '));
        });
        // gate 9 signal ①: smoke-render.sh greps `asc_onDocumentContentReady`
        // (hook name + log line below) — keep verbatim.
        ed2.attachEvent('asc_onDocumentContentReady', function () {
          forwardLog('info', '=== asc_onDocumentContentReady — DOCUMENT OPENED ===');
          scheduleSaveProbe(ed2);
        });
        ed2.attachEvent('asc_onBeginActionEnd', function () {
          forwardLog('info', 'asc_onBeginActionEnd (open action finished)');
        });
        ed2.attachEvent('asc_onAuthChanged', function () {
          forwardLog('info', 'asc_onAuthChanged');
        });
      } else {
        forwardLog('error', 'editor api has no attachEvent — cannot hook open signals');
      }
      if (probesEnabled()) {
        // Observe the api event stream directly (asc_onError reaching the UI
        // layer without touching our attachEvent hooks still passes through
        // the api's internal sendEvent).
        if (ed2 && ed2.cc && !ed2.__nanooffice_cc_wrapped) {
          ed2.__nanooffice_cc_wrapped = true;
          var origCc = ed2.cc;
          ed2.cc = function (ev) {
            var s = String(ev);
            if (s.indexOf('Error') !== -1) {
              forwardLog('error', 'api event ' + s + ' args='
                + Array.prototype.slice.call(arguments, 1).map(String).join(' | ').slice(0, 300));
            }
            return origCc.apply(this, arguments);
          };
        }
        try {
          var di = ed2 && ed2.DocInfo;
          forwardLog('info', 'DocInfo: offlineApp=' + (di ? di.get_OfflineApp() : 'noDocInfo')
            + ' documentUrl=' + (ed2 && ed2.documentUrl !== undefined ? ed2.documentUrl : '?')
            + ' docId=' + (ed2 && ed2.documentId !== undefined ? JSON.stringify(ed2.documentId) : '?'));
        } catch (dbgErr) {
          forwardLog('error', 'DocInfo probe failed: ' + dbgErr);
        }
      }
      // THE open call itself (desktop-sdk client_renderer_wrapper.cpp:6480):
      // window.DesktopOfflineAppDocumentEndLoad("<folder>", "<base64>", <decodedLen>).
      // Everything above this line is probe/hook setup only — the engine
      // learns about the document HERE and nowhere else.
      window.DesktopOfflineAppDocumentEndLoad(deliver.folder, deliver.b64, decodedLen);
      forwardLog('info', 'DesktopOfflineAppDocumentEndLoad executed');
      // The shell's loadend sequence continues with the signatures hook
      // (wrapper cpp fires it unconditionally right after EndLoad).
      if (typeof window.DesktopOfflineAppDocumentSignatures === 'function') {
        window.DesktopOfflineAppDocumentSignatures('');
      }
      if (probesEnabled()) {
        // PH-0 render evidence — T-050: repeating probes instead of the
        // one-shot 6/8/15s timers (dropped whole under WKWebView timer
        // throttling, diagnostics 30.09). Same log strings every tick.
        startRenderProbes();
      }
    } catch (e) {
      forwardLog('error', 'DesktopOfflineAppDocumentEndLoad failed: ' + e);
    }
  }

  /**
   * Probe-flag-gated save proof (T-010 test automation, NO keystrokes — the
   * T-003 lesson). Rust injects `window.__NANO_OFFICE_SAVE_PROBE_TEXT__` from
   * env NANO_OFFICE_SAVE_PROBE; when it is set (and probes are enabled), the
   * engine frame inserts the text via the public api (asc_AddText — the same
   * method used in the CDP oracle; asc_enterText is NOT in the word export
   * table) and calls asc_Save() — the exact user-equivalent chain.
   */
  function scheduleSaveProbe(ed) {
    var probeText = window.__NANO_OFFICE_SAVE_PROBE_TEXT__;
    if (!probesEnabled() || !probeText) return;
    setTimeout(function () {
      try {
        if (typeof ed.asc_AddText !== 'function') {
          forwardLog('error', 'save probe: asc_AddText unavailable');
          return;
        }
        ed.asc_AddText(probeText);
        forwardLog('info', 'save probe: text inserted (' + probeText.length + ' chars)');
        setTimeout(function () {
          try {
            ed.asc_Save();
            forwardLog('info', 'save probe: asc_Save() called');
          } catch (e) {
            forwardLog('error', 'save probe: asc_Save failed: ' + e);
          }
        }, 800);
      } catch (e) {
        forwardLog('error', 'save probe failed: ' + e);
      }
    }, 2500);
  }

  /**
   * Engine frame only: answer the save contract after the Rust-side x2t
   * conversion finished (see the SAVE-END LANE note in the header). Mirrors
   * the real shell's post-save eval sequence (docs/reference.map.md).
   */
  function consumeSaveEnd(info) {
    try {
      window.AscDesktopEditor.LocalFileSetSaved(!!info.ok);
      if (info.path) window.AscDesktopEditor.LocalFileSetSourcePath(info.path);
      window.DesktopOfflineAppDocumentEndSave(info.code || 0);
      forwardLog('info', 'save-end delivered to engine: code=' + (info.code || 0)
        + ' path=' + (info.path || ''));
    } catch (e) {
      forwardLog('error', 'save-end delivery failed: ' + e);
    }
  }

  /** Every frame: receive the payload travelling DOWN and either consume it
   *  (this is the engine frame) or keep it moving toward the children. This
   *  listener owns the DOWN lane since T-045; the UP lanes (LOG/RELAY) stay in
   *  desktopEditor.js's relay. */
  function installDownReceiver() {
    window.addEventListener('message', function (event) {
      var data = event.data;
      if (!data || data.type !== DOWN_TYPE) return;

      if (data.saveEnd) {
        if ((data.__hop || 0) > HOP_GUARD) return;
        if (typeof window.DesktopOfflineAppDocumentEndSave === 'function') {
          consumeSaveEnd(data.saveEnd);
        } else {
          postDown(data); // not the engine frame — keep it moving DOWN
        }
        return;
      }
      if (!data.deliver) return;

      if ((data.__hop || 0) > HOP_GUARD) return; // loop guard
      // Deliberately weaker than engineReadiness(): forwarding only needs an
      // EndLoad entry; full readiness (api + wasm) is retried inside
      // consumeDeliveredDocument.
      var engineReady = typeof window.DesktopOfflineAppDocumentEndLoad === 'function';
      if (engineReady) {
        consumeDeliveredDocument(data.deliver);
      } else {
        // Not the editor frame (or not ready yet) — keep it moving DOWN.
        postDown(data);
      }
    });
  }

  installDownReceiver();

  // Public surface consumed by desktopEditor.js — kept minimal (T-045); T-010
  // adds saveEnd (Rust evals it in the MAIN frame after the x2t save).
  window.__nanooffice_delivery = {
    deliverDocument: deliverDocument,
    saveEnd: function (info) { postDown({ type: DOWN_TYPE, saveEnd: info }); },
  };
})(window);
