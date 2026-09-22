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
    'LocalFileSetSaved', 'LocalFileRecoverFolder', 'LocalFileSetSourcePath',
    'LocalFileGetSourcePath', 'LocalFileSetOpenChangesCount', 'CheckUserId',
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
    LocalFileSetSaved: function () { return null; },
    LocalFileRecoverFolder: function () { return null; },
    LocalFileSetSourcePath: function (p) { localSourcePath = p; return null; },
    LocalFileGetSourcePath: function () { return localSourcePath; },
    LocalFileSetOpenChangesCount: function () { return null; },
  };

  var localSourcePath = '';

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
    if (!editorApi || typeof window.DesktopOfflineAppDocumentEndLoad !== 'function') {
      if (deliver.__tries === undefined) deliver.__tries = 0;
      if (++deliver.__tries > 40) {
        forwardLog('error', 'document delivery aborted: engine api not ready after 20s');
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
        ed2.attachEvent('asc_onError', function (err) {
          forwardLog('error', 'asc_onError: ' + ((err && (err.id + ' ' + err.msg)) || JSON.stringify(err)));
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
      try {
        var di = ed2 && ed2.DocInfo;
        forwardLog('info', 'DocInfo: offlineApp=' + (di ? di.get_OfflineApp() : 'noDocInfo')
          + ' documentUrl=' + (ed2 && ed2.documentUrl !== undefined ? ed2.documentUrl : '?')
          + ' docId=' + (ed2 && ed2.documentId !== undefined ? JSON.stringify(ed2.documentId) : '?'));
      } catch (dbgErr) {
        forwardLog('error', 'DocInfo probe failed: ' + dbgErr);
      }
      window.DesktopOfflineAppDocumentEndLoad(deliver.folder, deliver.b64, decodedLen);
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
    window.AscDesktopEditor = bridge;

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
