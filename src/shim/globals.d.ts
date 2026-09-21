/**
 * Global declarations for the NanoOffice bridge.
 * Method surface mirrors docs/reference.map.md (lower bound ~130, waves added as implemented).
 */

declare global {
  interface Window {
    /** Injected by src/shim/desktopEditor.ts — Rust-backed AscDesktopEditor shim. */
    AscDesktopEditor: Record<string, (...args: unknown[]) => Promise<unknown>> & {
      execCommand: (cmd: string, json: string) => Promise<unknown>;
    };
    /** Env object normally injected by native (theme, lang, rtl, printer...). */
    RendererProcessVariable: Record<string, unknown>;
    /** Native → JS message router stub (13 known cmds, see docs/reference.map.md). */
    on_native_message: (cmd: string, param?: unknown) => void;
  }
}

export {};
