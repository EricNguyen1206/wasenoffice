/**
 * Bridge shim — JS side of `window.AscDesktopEditor` (Tauri).
 *
 * Injected into the WebView BEFORE the engine loads (see docs/conventions.md).
 * Forwards every `window.AscDesktopEditor.<Method>()` call to Rust via
 * `invoke('asc_desktop_editor', { method, payload })`.
 *
 * Contract (harness/skills/bridge-shim/SKILL.md):
 * - Method names EXACTLY as upstream.
 * - Rust replies `UNIMPL:<method>` → we log the call-site hit and rethrow,
 *   so missing surface is visible during PH-0 instead of silently breaking.
 * - `window.RendererProcessVariable` and `window.on_native_message` router are
 *   stubbed here too (native → JS channel).
 */

type DesktopBridgeMethod = (...args: unknown[]) => Promise<unknown>;
type DesktopBridge = Record<string, DesktopBridgeMethod> & {
  execCommand: (cmd: string, json: string) => Promise<unknown>;
};

// Methods observed in upstream call-sites (lower bound, docs/reference.map.md).
// PH-0 first wave is marked; everything else still gets an auto-forwarding stub.
const KNOWN_METHODS: string[] = [
  // file group (PH-0 wave)
  'IsLocalFile', 'OpenFilenameDialog', 'LocalFileGetImageUrl', 'LocalFileSave',
  'LocalFileSaveDialog', 'LocalFileGetOpenChangesCount', 'LocalStartOpen',
  'loadLocalFile', 'GetDropFiles', 'convertFile', 'startExternalConvertation',
  'ResaveFile', 'CreateEditorApi',
  // fonts/media
  'LoadFontBase', 'GetImageBase', 'GetImageFormat', 'GetImageOriginalSize',
  'MediaStart', 'CallMediaPlayerCommand', 'AddVideo', 'AddAudio',
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

function forwardToRust(method: string, args: unknown): Promise<unknown> {
  // Tauri global injected with withGlobalTauri: true
  const tauri = (window as unknown as { __TAURI__?: { core?: { invoke?: (cmd: string, args?: unknown) => Promise<unknown> } } }).__TAURI__;
  const invoke = tauri?.core?.invoke;
  if (!invoke) {
    console.error('[nanooffice-bridge] Tauri invoke unavailable; method', method);
    return Promise.resolve(null);
  }
  return invoke('asc_desktop_editor', { method, payload: { args } }).catch((err: unknown) => {
    const msg = String(err);
    if (msg.startsWith('UNIMPL:')) {
      // Visible marker during PH-0: which bridge surface is still missing.
      console.warn('[nanooffice-bridge] UNIMPL hit:', msg, 'args:', args);
    }
    throw err;
  });
}

export function installDesktopEditorBridge(): void {
  if (window.AscDesktopEditor) return; // already installed

  const bridge = {} as DesktopBridge;
  for (const method of KNOWN_METHODS) {
    (bridge as Record<string, DesktopBridgeMethod>)[method] = (
      ...args: unknown[]
    ) => forwardToRust(method, args);
  }
  // Unknown methods still reach Rust (which logs them as UNIMPL) — no silent drops.
  bridge.execCommand = (cmd: string, json: string) => forwardToRust('execCommand', { cmd, json });

  window.AscDesktopEditor = bridge;

  // Native → JS channel stubs (real handlers arrive with PH-0/PH-1 work).
  window.RendererProcessVariable = window.RendererProcessVariable || {};
  if (!window.on_native_message) {
    window.on_native_message = (cmd: string, param?: unknown) => {
      console.debug('[nanooffice-bridge] on_native_message (stub):', cmd, param);
    };
  }
}
