/**
 * Bridge shim — JS side of `window.AscDesktopEditor` (Tauri).
 *
 * Plain script (no imports/exports) so one file serves as:
 *   1. Tauri initialization script injected into ALL frames (src-tauri/src/lib.rs)
 *   2. vite side-effect import on the start page (src/main.ts)
 *
 * Frame topology (PH-0): start page (main frame, has __TAURI__) → DocsAPI boot
 * page iframe → editor iframe. The engine lives in the GRANDCHILD.
 *
 * Contract (harness/skills/bridge-shim/SKILL.md):
 * - Method names EXACTLY as upstream (case-sensitive).
 * - Sync-returning methods are answered locally (LOCAL_TRUTHS) — the CEF shell
 *   answers those synchronously and an async bridge cannot (call-sites use them
 *   inside `if` conditions / at script load). Values mirror the Rust dispatch
 *   in src-tauri/src/bridge/mod.rs — keep both in sync.
 * - Everything else relays to the main frame → Rust invoke; UNIMPL rejections
 *   stay visible instead of silently breaking.
 * - Document delivery follows the desktop open contract (desktop-sdk
 *   client_renderer_wrapper.cpp "onlocaldocument_loadend"): LocalStartOpen
 *   (engine → native) → Rust reads the registered file → main frame relays the
 *   bytes back down → engine frame calls window.DesktopOfflineAppDocumentEndLoad.
 */
(function (window) {
  'use strict';

  if (window.__nanooffice_bridge_installed) return;
  window.__nanooffice_bridge_installed = true;

  // Methods observed in upstream call-sites (lower bound, docs/reference.map.md).
  var KNOWN_METHODS = [
    // file group (PH-0 wave)
    'IsLocalFile', 'OpenFilenameDialog', 'LocalFileGetImageUrl', 'LocalFileSave',
    'LocalFileSaveDialog', 'LocalFileGetOpenChangesCount', 'LocalStartOpen',
    'loadLocalFile', 'GetDropFiles', 'convertFile', 'startExternalConvertation',
    'ResaveFile', 'CreateEditorApi',
    // desktop local-open delivery (ack'd; bytes flow via DesktopOfflineAppDocumentEndLoad)
    'LocalFileSetSaved', 'LocalFileGetSaved', 'LocalFileRecoverFolder', 'LocalFileSetSourcePath',
    'LocalFileGetSourcePath', 'LocalFileSetOpenChangesCount',
    'LocalFileGetOpenChangesCount', 'onDocumentModifiedChanged', 'CheckUserId',
    // desktop recents (see LOCAL_TRUTHS note)
    'LocalFileRecents', 'LocalFileRecovers', 'LocalFileOpenRecent', 'LocalFileOpenRecover',
    // fonts/media
    'LoadFontBase', 'LoadFontBase64', 'GetImageBase', 'GetImageFormat',
    'GetImageOriginalSize', 'MediaStart', 'CallMediaPlayerCommand', 'AddVideo', 'AddAudio',
    // print
    'Print', 'Print_Start', 'Print_Page', 'Print_End', 'IsFilePrinting',
    'emulateCloudPrinting', 'SetPdfCloudPrintFileInfo', 'localSaveToDrawingFormat',
    // spell/clipboard
    'SpellCheck', 'getDictionariesPath', 'asc_desktop_copypaste',
    // crypto/signature (PH-3)
    'GetCryptoMode', 'SetCryptoMode', 'GetSupportCryptoModes', 'OpenFileCrypt',
    'CryptoDownloadAs', 'buildCryptedStart', 'buildCryptedEnd', 'PreloadCryptoImage',
    'GetEncryptedHeader', 'Sign', 'SelectCertificate', 'ViewCertificate',
    'GetDefaultCertificate', 'RemoveSignature', 'RemoveAllSignatures', 'IsSignaturesSupport',
    // system/window
    'SetDocumentName', 'SetFullscreen', 'SetLocalRestrictions', 'sendSystemMessage',
    'CallInAllWindows', 'getEngineVersion', 'GetExternalClouds', 'updateClouds',
    'checkAuth', 'GetLocalFeatures', 'updateVersion',
    // plugins
    'PluginInstall', 'PluginUninstall', 'GetInstallPlugins', 'isSupportPlugins', 'isSupportMacroses',
    // viewer/merge
    'NativeViewerOpen', 'OpenWorkbook', 'MergeDocumentUrl', 'MergeDocumentFile',
    'CompareDocumentUrl', 'CompareDocumentFile', 'SetAdvancedOptions',
  ];

  var BRIDGE_CMD = 'asc_desktop_editor';
  var RELAY_TYPE = '__nanooffice_bridge';   // engine frame → main frame (log + dispatch)
  var LOG_TYPE = '__nanooffice_log';        // any frame → main frame (stdout)
  var DOWN_TYPE = '__nanooffice_down';      // main frame → engine frame (document delivery)

  /**
   * Methods answered WITHOUT a Rust round-trip. The upstream editor calls these
   * synchronously inside `if` conditions or at script load — an async answer
   * would always read falsy and break the open flow. Keep in sync with the
   * Rust dispatch (src-tauri/src/bridge/mod.rs). Evidence per entry lives in
   * docs/reference.map.md.
   */
  var LOCAL_TRUTHS = {
    IsLocalFile: function () { return true; },
    getEngineVersion: function () { return '0.1.0'; },
    CreateEditorApi: function () { return null; },
    execCommand: function () { return null; },
    GetEncryptedHeader: function () { return 'ENCRYPTED;'; },
    CheckUserId: function () { return 'nanooffice-ph0'; },
    GetInstallPlugins: function () {
      // CPluginsManager::GetPluginsJson shape: [systemFolder, userFolder],
      // each with a pluginsData array (empty = nothing installed).
      return '[{"url":"","pluginsData":[]},{"url":"","pluginsData":[]}]';
    },
    isSupportPlugins: function () { return true; },
    isSupportMacroses: function () { return true; },
    SetDocumentName: function () { return null; },
    IsSignaturesSupport: function () { return false; },
    // local-open delivery acks (bytes themselves flow via DesktopOfflineAppDocumentEndLoad)
    LocalFileSetSaved: function (v) { localSaved = v !== false; return null; },
    LocalFileGetSaved: function () { return localSaved; },
    LocalFileRecoverFolder: function () { return null; },
    LocalFileSetSourcePath: function (p) { localSourcePath = p; return null; },
    LocalFileGetSourcePath: function () { return localSourcePath; },
    LocalFileSetOpenChangesCount: function () { return null; },
    // sync-truth: PH-0 documents open clean (0 unsaved changes)
    LocalFileGetOpenChangesCount: function () { return 0; },
    // desktop recents (fire-and-forget: the shell pushes lists back via the
    // onupdaterecents/onupdaterecovers globals — PH-2; must EXIST because the
    // editor UI calls them unconditionally after open — loginpage panels.js)
    LocalFileRecents: function () { return null; },
    LocalFileRecovers: function () { return null; },
    LocalFileOpenRecent: function () { return null; },
    LocalFileOpenRecover: function () { return null; },
    // modified-flag sink: the engine reports dirty state; the real shell tracks
    // it for the title/save UI (PH-1). Must exist — called right after open.
    onDocumentModifiedChanged: function () { return null; },
  };

  var localSourcePath = '';
  var localSaved = true; // fresh local open = saved (desktop-sdk m_bLocalIsSaved)

  /**
   * PH-0 oracle probe: dump the same font-pipeline fields observed in the real
   * app via CDP (docs/diagnostics.md) so the two runtimes can be diffed from
   * stdout alone. Runs once, 8s after the document reaches the engine.
   */
  function logFontOracleState() {
    try {
      var Wg = window.AscFonts && window.AscFonts.Wg;
      var jM = window.AscCommon && window.AscCommon.jM;
      var iEc = window.AscFonts && window.AscFonts.iEc;
      var Hcc = window.AscFonts && window.AscFonts.Hcc;
      var loaded = [];
      if (iEc) {
        for (var i = 0; i < iEc.length; i++) {
          if (iEc[i].UB !== -1) loaded.push(iEc[i].Va.slice(-40) + ' UB=' + iEc[i].UB);
        }
      }
      var root = document.getElementById('editor_sdk') || document.body;
      var viewer = document.getElementById('id_viewer');
      var sdkBox = root.getBoundingClientRect();
      forwardLog('info', 'font-oracle: Wg.Of=' + (Wg ? Wg.Of.length : '?')
        + ' aVc=' + (Wg ? Object.keys(Wg.aVc || {}).length : '?')
        + ' jM.SIa=' + (jM && jM.SIa ? jM.SIa.length : 'undef')
        + ' jM.PVd=' + (jM && jM.PVd ? jM.PVd.length : 'undef')
        + ' Hcc=' + (Hcc ? Hcc.length : 'undef')
        + ' loadedFonts=' + loaded.length + (loaded.length ? ' [' + loaded.slice(0, 6).join(' | ') + ']' : '')
        + ' viewer=' + (viewer ? viewer.clientWidth + 'x' + viewer.clientHeight : 'none')
        + ' geom[inner=' + window.innerWidth + 'x' + window.innerHeight
        + ' editor_sdk=' + Math.round(sdkBox.width) + 'x' + Math.round(sdkBox.height) + ']');
    } catch (e) {
      forwardLog('error', 'font-oracle probe failed: ' + e);
    }
  }

  function safeArgs(args) {
    try {
      var s = JSON.stringify(args);
      return s === undefined ? '[]' : (s.length > 500 ? s.slice(0, 500) + '…' : s);
    } catch (e) {
      return '[unserializable]';
    }
  }

  function tauriInvoke() {
    var tauri = window.__TAURI__;
    return (tauri && tauri.core && tauri.core.invoke) || null;
  }

  function isMainFrame() {
    return !!tauriInvoke();
  }

  function forwardLog(level, message) {
    var invoke = tauriInvoke();
    if (invoke) {
      invoke('frontend_log', { level: level, message: message }).catch(function () {});
    } else if (window.parent && window.parent !== window) {
      try {
        window.parent.postMessage({ type: LOG_TYPE, level: level, message: message }, '*');
      } catch (e) { /* diagnostic only */ }
    }
  }

  /** Main frame only: fetch the registered document and send it DOWN to the
   *  engine frame (which asked for it via LocalStartOpen). */
  function deliverDocument() {
    var invoke = tauriInvoke();
    if (!invoke) return;
    invoke('get_document_for_delivery').then(function (doc) {
      // The real shell delivers well after LocalStartOpen (it converts first);
      // give the engine's post-LocalStartOpen init a beat to settle.
      setTimeout(function () {
        postDown({ type: DOWN_TYPE, deliver: doc });
      }, 1500);
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

  /** Engine frame only: consume the delivered document via the engine's own
   *  offline-open entry (defined by sdk-all.js). The engine api object must be
   *  fully initialized first (the real desktop app delivers late — after file
   *  conversion) — retry until Asc.editor exists. */
  function consumeDeliveredDocument(deliver) {
    var editorApi = window.Asc && (window.Asc.editor || window.editor);
    // The wasm font engine must be fully initialized BEFORE the open: the
    // glyph manager (AscFonts.Wg.II) silently no-ops while its wasm handle
    // (vue) is absent, so a font queued too early is never loaded.
    var wasmReady = window.AscFonts && window.AscFonts.Wg && !!window.AscFonts.Wg.vue;
    if (!editorApi || !wasmReady || typeof window.DesktopOfflineAppDocumentEndLoad !== 'function') {
      if (deliver.__tries === undefined) {
        deliver.__tries = 0;
        forwardLog('info', 'delivery waiting: api=' + !!editorApi
          + ' wasmReady=' + !!wasmReady
          + ' endload=' + (typeof window.DesktopOfflineAppDocumentEndLoad));
      }
      if (++deliver.__tries > 120) {
        forwardLog('error', 'document delivery aborted: engine not ready after 60s'
          + ' (api=' + !!editorApi + ' wasmReady=' + !!wasmReady + ')');
        return;
      }
      setTimeout(function () { consumeDeliveredDocument(deliver); }, 500);
      return;
    }
    try {
      // EndLoad expects the file as a BASE64 string + decoded byte length
      // (its $$ helper is AscCommon.zP.decode(b64, false, len)). The head
      // bytes below are only for the PH-0 evidence log.
      var bin = atob(deliver.b64);
      var decodedLen = bin.length;
      var head = [bin.charCodeAt(0), bin.charCodeAt(1), bin.charCodeAt(2), bin.charCodeAt(3)];
      var src = deliver.folder + '/' + deliver.name;
      window.AscDesktopEditor.LocalFileRecoverFolder(deliver.folder);
      window.AscDesktopEditor.LocalFileSetSourcePath(src);
      window.AscDesktopEditor.LocalFileSetSaved(true);
      forwardLog('info', 'delivering document to engine: ' + deliver.name
        + ' (' + decodedLen + ' bytes, head ' + head.join(',') + ')');
      try {
        var probe = AscCommon.zP.decode(deliver.b64, false, decodedLen);
        forwardLog('info', 'probe decode: type=' + (probe && probe.constructor && probe.constructor.name)
          + ' len=' + (probe && probe.length)
          + ' head=' + (probe ? [probe[0], probe[1], probe[2], probe[3]].join(',') : '?'));
      } catch (dbgErr) {
        forwardLog('error', 'probe decode failed: ' + dbgErr);
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
        ed2.attachEvent('asc_onDocumentContentReady', function () {
          forwardLog('info', '=== asc_onDocumentContentReady — DOCUMENT OPENED ===');
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
      // PH-0 render evidence: canvas count/size a few seconds after open.
      setTimeout(function () {
        var root = document.getElementById('editor_sdk') || document.body;
        var canvases = root.querySelectorAll('canvas');
        var info = [];
        for (var i = 0; i < canvases.length && i < 4; i++) {
          info.push(canvases[i].width + 'x' + canvases[i].height);
        }
        forwardLog('info', 'canvas probe (' + root.id + '): ' + canvases.length + ' canvas(es) ' + info.join(' '));
      }, 6000);
      setTimeout(logFontOracleState, 8000);
      setTimeout(function () {
        var root = document.getElementById('editor_sdk') || document.body;
        var canvases = root.querySelectorAll('canvas');
        var info = [];
        for (var i = 0; i < canvases.length; i++) {
          info.push('#' + i + ':' + canvases[i].id + ' ' + canvases[i].width + 'x' + canvases[i].height);
        }
        var ed = window.Asc && (window.Asc.editor || window.editor);
        forwardLog('info', 'canvas probe-2 (' + root.id + '): ' + canvases.length + ' canvas(es) ' + info.join(' ')
          + ' | editor state: isDocumentContentReady=' + (ed && ed.isDocumentContentReady ? ed.isDocumentContentReady() : '?'));
      }, 15000);
    } catch (e) {
      forwardLog('error', 'DesktopOfflineAppDocumentEndLoad failed: ' + e);
    }
  }

  /** Every frame: route framed messages to where they belong.
   *  - UP (LOG/RELAY): forward toward the main frame; the main frame invokes Rust.
   *  - DOWN (DOWN_TYPE): forward into child iframes; the editor frame consumes. */
  function installRelay() {
    window.addEventListener('message', function (event) {
      var data = event.data;
      if (!data) return;

      if (data.type === LOG_TYPE) {
        var invoke = tauriInvoke();
        if (invoke) {
          invoke('frontend_log', { level: data.level, message: data.message }).catch(function () {});
        } else if (window.parent && window.parent !== window) {
          try { window.parent.postMessage(data, '*'); } catch (e) { /* diagnostic */ }
        }
        return;
      }

      if (data.type === RELAY_TYPE) {
        if (isMainFrame()) {
          if (data.method === 'LocalStartOpen') {
            // Engine is ready — start the delivery chain.
            mainFrameInvoke('LocalStartOpen', data.args).catch(function () {});
            deliverDocument();
          } else {
            mainFrameInvoke(data.method, data.args).catch(function () {
              /* UNIMPL already logged; PH-0 relay is logging-only */
            });
          }
        } else if (window.parent && window.parent !== window) {
          try { window.parent.postMessage(data, '*'); } catch (e) { /* diagnostic */ }
        }
        return;
      }

      if (data.type === DOWN_TYPE && data.deliver) {
        if ((data.__hop || 0) > 6) return; // loop guard
        var engineReady = typeof window.DesktopOfflineAppDocumentEndLoad === 'function';
        if (engineReady) {
          consumeDeliveredDocument(data.deliver);
        } else {
          // Not the editor frame (or not ready yet) — keep it moving DOWN.
          postDown(data);
        }
      }
    });
  }

  function mainFrameInvoke(method, args) {
    var invoke = tauriInvoke();
    if (!invoke) {
      console.warn('[nanooffice-bridge] __TAURI__ unavailable; method', method);
      return Promise.resolve(null);
    }
    return invoke(BRIDGE_CMD, { method: method, payload: { args: args } }).catch(function (err) {
      var msg = String(err);
      if (msg.indexOf('UNIMPL:') === 0) {
        console.warn('[nanooffice-bridge] UNIMPL hit:', msg, 'args:', safeArgs(args));
      }
      throw err;
    });
  }

  function installErrorForwarding() {
    window.addEventListener('error', function (ev) {
      var target = ev.target;
      var resource = target && (target.src || target.href);
      forwardLog(
        'error',
        resource ? 'resource error: ' + resource
                 : (ev.message + ' @ ' + ev.filename + ':' + ev.lineno + ':' + ev.colno)
      );
    }, true);
    window.addEventListener('unhandledrejection', function (ev) {
      forwardLog('error', 'unhandled rejection: ' + String(ev.reason));
    });
  }

  /**
   * Font-file loader adaptation for the FULL desktop build (PH-0).
   *
   * Observed engine behavior (pinned word build): near the END of sdk-all.js
   * the font-file loader entry (`AscFonts.sje.prototype.BSa` / LoadFontAsync)
   * is rerouted to an async XHR of `ascdesktop://fonts/<font path>` — a scheme
   * the CEF desktop shell intercepts natively and answers with the font bytes.
   * Observed real-shell runtime: fonts land in the engine stream registry
   * (`AscFonts.Hcc` entries of `AscFonts.G2b`) with loader status `UB` 2 →
   * 0 (loaded) / 1 (failed), stream index assigned via the loader's `wOb`,
   * and the loader's callbacks fired when the bytes are in.
   *
   * A Tauri WKWebView has no `ascdesktop://` scheme handler, so every font
   * XHR would die (CORS/unknown scheme) and the document open would wait on
   * fonts forever. This adapter swaps the SAME entry point for a same-origin
   * XHR of the font URL itself — the Rust filesrv already serves every font
   * listed in `__fonts_files` under its (rewritten) URL — then feeds the
   * registry/status/callback contract described above. Behavior is
   * re-implemented from runtime observation; no upstream code is copied.
   */
  function patchDesktopFontLoader() {
    if (!(window.AscFonts && window.AscFonts.sje && window.AscFonts.Hcc
          && window.AscFonts.sje.prototype)) {
      forwardLog('warn', 'font loader patch: sdkjs font registry not found');
      return;
    }
    var proto = window.AscFonts.sje.prototype;
    if (proto.__nanooffice_patched) return;
    proto.__nanooffice_patched = true;
    proto.BSa = function (_baseUrl, onDone) {
      var self = this;
      if (self.UB !== -1) return true; // loading (2) / loaded (0) / failed (1)
      self.UB = 2;
      self.sV = onDone || null;
      var xhr = new XMLHttpRequest();
      xhr.open('GET', self.Va, true); // same-origin loopback font URL
      xhr.responseType = 'arraybuffer';
      xhr.onload = function () {
        if (xhr.status !== 200 || !xhr.response) {
          self.UB = 1; // failed (final state — the engine treats 1 as done)
          return;
        }
        var bytes = new Uint8Array(xhr.response);
        window.AscFonts.Hcc.push(new window.AscFonts.G2b(bytes, bytes.length));
        self.wOb(window.AscFonts.Hcc.length - 1);
        self.UB = 0;
        if (self.sV) { var cb = self.sV; self.sV = null; cb(); }
        if (self.externalCallback) self.externalCallback();
      };
      xhr.onerror = function () {
        self.UB = 1;
        forwardLog('warn', 'font fetch failed: ' + String(self.Va).slice(0, 110));
      };
      xhr.send();
      return false; // async load in flight
    };
    forwardLog('info', 'desktop font loader patched for loopback delivery');
  }

  function install() {
    if (window.AscDesktopEditor) return;

    var bridge = {};
    KNOWN_METHODS.forEach(function (method) {
      bridge[method] = function () {
        var args = Array.prototype.slice.call(arguments);
        var truth = LOCAL_TRUTHS[method];
        if (truth) {
          relay(method, args);
          return truth.apply(null, args);
        }
        relay(method, args);
        if (isMainFrame()) {
          return mainFrameInvoke(method, args);
        }
        // Engine/boot frames: surface the missing call, keep it observable.
        console.warn('[nanooffice-bridge] UNIMPL (iframe, no forward):', method, safeArgs(args));
        return Promise.reject('UNIMPL:' + method);
      };
    });
    // Status channel web-apps uses heavily at boot (desktopinit.js, Desktop.js);
    // outside KNOWN_METHODS in upstream terms but part of the same contract.
    bridge.execCommand = function (cmd, json) {
      relay('execCommand', [cmd, json]);
      return LOCAL_TRUTHS.execCommand(cmd, json);
    };
    /**
     * sdk-all-min.js is a BOOTSTRAP: the real engine (sdk-all.js) loads through
     * this bridge method in the desktop shell (sdk-all-min rb(): return codes
     * 1 = sync, 2 = async with local_load_add completeLoad, else web fallback).
     * Without it the runtime ends up with an inconsistent dual-build state.
     */
    bridge.LoadJS = function (url) {
      var abs;
      try {
        abs = new URL(url, window.location.href).href;
      } catch (e) {
        abs = url;
      }
      fetch(abs)
        .then(function (r) { return r.text(); })
        .then(function (text) {
          // Emscripten glue resolves the wasm payload via scriptDirectory,
          // which is empty when the script runs through LoadJS eval (no
          // currentScript). Give the glue an explicit locateFile: engine
          // payloads live in fixed sdkjs/engine directories (by basename).
          window.Module = {
            locateFile: function (path) {
              var map = {
                'fonts.wasm': '/editors/sdkjs/common/libfont/engine/',
                'zlib.wasm': '/editors/sdkjs/common/zlib/engine/',
                'engine.wasm': '/editors/sdkjs/common/hash/hash/engine/',
                'drawingfile.wasm': '/editors/sdkjs/pdf/src/engine/',
                'spell.wasm': '/editors/sdkjs/common/spell/spell/',
              };
              return (map[path] || '/editors/sdkjs/common/libfont/engine/') + path;
            },
          };
          // jshint evil:true — the engine payload itself (AGPL artifact).
          (0, eval)(text);
          if (abs.indexOf('sdk-all.js') !== -1) {
            patchDesktopFontLoader();
          }
          var pending = window.__nanooffice_pending_js;
          var done = pending && pending[url];
          if (done) {
            delete pending[url];
            done();
          }
          forwardLog('info', 'LoadJS executed: ' + abs.slice(0, 100));
        })
        .catch(function (err) {
          forwardLog('error', 'LoadJS failed: ' + abs + ' — ' + err);
        });
      return 2; // async; completeLoad fires when executed
    };
    window.AscDesktopEditor = bridge;

    // Coordination tables for LoadJS (see rb() in sdk-all-min).
    window.local_load_add = function (store, name, url) {
      window.__nanooffice_pending_js = window.__nanooffice_pending_js || {};
      window.__nanooffice_pending_js[url] = store.completeLoad;
      return store;
    };
    window.local_load_remove = function (url) {
      if (window.__nanooffice_pending_js) delete window.__nanooffice_pending_js[url];
    };


    // Native → JS channel stubs (real handlers arrive with PH-1/PH-2 work).
    if (!window.RendererProcessVariable) window.RendererProcessVariable = {};
    if (!window.on_native_message) {
      window.on_native_message = function (cmd, param) {
        console.debug('[nanooffice-bridge] on_native_message (stub):', cmd, param);
      };
    }

    installRelay();
    installErrorForwarding();
    forwardLog('info', 'frame ready: ' + window.location.href.slice(0, 120));
  }

  function relay(method, args) {
    // engine/boot frame → parent chain → main frame → Rust (best effort).
    try {
      if (!isMainFrame() && window.parent && window.parent !== window) {
        window.parent.postMessage({ type: RELAY_TYPE, method: method, args: safeArgs(args) }, '*');
      }
    } catch (e) { /* relay is diagnostic only */ }
  }

  install();
})(window);
