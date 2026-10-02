/**
 * Global declarations for the NanoOffice bridge.
 * Method surface mirrors docs/reference.map.md (lower bound ~130, waves added as implemented).
 */

interface DocsApiConfig {
  type?: string;
  width?: string;
  height?: string;
  documentType?: string;
  document: {
    fileType: string;
    key: string;
    title: string;
    url: string;
  };
  editorConfig?: {
    mode?: string;
    user?: { id: string; name: string };
    customization?: Record<string, unknown>;
  };
  events?: Record<string, unknown>;
}

declare global {
  interface Window {
    /** Injected by src/shim/desktopEditor.js — Rust-backed AscDesktopEditor shim.
     *  Return type is `unknown` BY CONTRACT: LOCAL_TRUTHS methods answer
     *  SYNCHRONOUSLY (the CEF shell does the same — call-sites use them in
     *  `if` conditions), everything else resolves a Promise; typed Promise-only
     *  would lie about ~28 sync methods. Gate: harness/scripts/check-bridge-surface.mjs. */
    AscDesktopEditor: Record<string, (...args: unknown[]) => unknown> & {
      execCommand: (cmd: string, json: string) => unknown;
    };
    /** Env object normally injected by native (theme, lang, rtl, printer...). */
    RendererProcessVariable: Record<string, unknown>;
    /** Native → JS message router stub (13 known cmds, see docs/reference.map.md). */
    on_native_message: (cmd: string, param?: unknown) => void;
    /** Tauri global API (withGlobalTauri in tauri.conf.json). */
    __TAURI__: {
      core: {
        invoke: <T = unknown>(cmd: string, args?: Record<string, unknown>) => Promise<T>;
      };
    };
    /** ONLYOFFICE web-apps integration API (loaded from editors/web-apps/apps/api). */
    DocsAPI: {
      DocEditor: new (placeholder: string, config: DocsApiConfig) => unknown;
    };
  }
}

export {};
