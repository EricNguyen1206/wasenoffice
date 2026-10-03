/**
 * L01 Nano Light editor slice (DD docs/design/editor-a4/DD.md) — the EDIT
 * side of the A3 viewer surface: the SAME pure layout module
 * (`src/viewer/layout.ts`, consumed VERBATIM — never forked), plus a caret
 * drawn from the hit-test coordinate system and a command-layer edit loop.
 * The canvas/media/evidence machinery lives in `src/viewer/render.ts`
 * (shared with the viewer since T-060 C1); this entry owns the session
 * state, the edit commands, the caret overlay and the save button. The
 * `Document` model lives Rust-side in window-scoped sessions; every
 * mutation re-resolves server-side and returns a fresh render model to
 * redraw. Typing is plain `keydown` (no synthetic keystrokes anywhere —
 * rule 6b); the gate-15 probe (NANO_OFFICE_NL_PROBE) drives the SAME edit()
 * the keydown handler uses. No engine shim is loaded on this page.
 */
import {
  canvasMeasurer,
  columnWidth,
  drawModel,
  loadMedia,
  mediaInk,
  resizeRelayout,
  toEvidence,
} from './viewer/render';
import { caretXY, hitTest, layoutBlocks, MARGIN, type Block, type FontMetrics, type LineModel } from './viewer/layout';

interface Caret {
  para: number;
  run: number;
  offset: number;
  /** T-060 C3: the hit's cell identity — `[row, col]` when the caret sits
   * inside a table, null at top level. The command layer refuses edits on
   * `cell !== null` (round-1 scope, DD L01 §2). */
  cell: [number, number] | null;
}

interface NanoLightDoc {
  parseMs: number;
  doc: { body: Block[] };
  fonts: FontMetrics;
  /** `w:pgSz` of the final section, twips (§17.6.11). */
  page: { w: number | null; h: number | null; orient: string | null } | null;
  /** `<origin>/media/<fileId>` prefix (loopback filesrv); null = media off. */
  mediaBase: string | null;
  caret: Caret;
  name: string;
}

interface EditOutcome {
  doc: { body: Block[] };
  caret: Caret;
  dirty: boolean;
}

type EditCmd =
  | { kind: 'insert'; text: string; caret: Caret }
  | { kind: 'backspace'; caret: Caret }
  | { kind: 'delete'; caret: Caret }
  | { kind: 'enter'; caret: Caret }
  | { kind: 'undo' }
  | { kind: 'redo' };

const { invoke } = (window as unknown as { __TAURI__: { core: { invoke: <T>(cmd: string, args?: unknown) => Promise<T> } } }).__TAURI__.core;

async function main(): Promise<void> {
  const t0 = performance.now();
  const cv = document.getElementById('cv') as HTMLCanvasElement;
  const box = cv.parentElement as HTMLElement;
  const dpr = window.devicePixelRatio || 1;
  const ctx = cv.getContext('2d')!;
  const nameEl = document.getElementById('doc-name')!;
  const dotEl = document.getElementById('dirty-dot')!;
  const saveBtn = document.querySelector<HTMLButtonElement>('#save-btn')!;

  try {
    const nl = await invoke<NanoLightDoc>('nano_light_open');
    nameEl.textContent = nl.name;
    let body = nl.doc.body;
    let caret = nl.caret;
    let dirty = false;
    let model: LineModel | null = null;
    let colW = 0;
    const bitmaps = new Map<string, ImageBitmap>();
    if (nl.mediaBase) await loadMedia(body, nl.mediaBase, bitmaps, 'nano light', invoke);
    const measure = canvasMeasurer(ctx);
    const updateStatus = (): void => {
      dotEl.classList.toggle('hidden', !dirty);
    };

    const render = (): LineModel => {
      colW = columnWidth(nl.page, box.clientWidth);
      const m = layoutBlocks({
        blocks: body,
        x0: MARGIN,
        w: colW,
        y: MARGIN + 16,
        fonts: nl.fonts,
        measure,
      });
      // the caret overlay: a static rect (blink is a later slice, DD L01 §2)
      drawModel(m, cv, ctx, dpr, box.clientWidth, box.clientHeight, bitmaps, () => {
        const c = caretXY(m, measure, caret);
        if (c) {
          ctx.fillStyle = '#000';
          ctx.fillRect(c.x - 0.75, c.y - c.lh + 2, 1.5, c.lh - 1);
        }
      });
      model = m;
      return m;
    };

    /** Layout + evidence lines (ADDITIVE log contract with
     * harness/scripts/smoke-nano-light.sh — gate 15): change both in the
     * SAME commit. Same shapes as the viewer's lines. */
    const logLayoutEvidence = (): void => {
      if (!model) return;
      const m = model.metrics;
      const baselines = model.lines.slice(0, 40).map((l) => l.baseline.toFixed(1)).join(',');
      const lhs = model.lines.slice(0, 40).map((l) => l.lh.toFixed(1)).join(',');
      void invoke('frontend_log', {
        level: 'info',
        message: `nano light layout: lines=${model.lines.length} colw=${colW.toFixed(1)} maxw=${model.maxw.toFixed(1)} metrics=${m ? `${m[0]},${m[1]},${m[2]}` : 'none'} lhs=${lhs} baselines=${baselines}`,
      });
      void invoke('frontend_log', {
        level: 'info',
        message: `nano light evidence: ${JSON.stringify(toEvidence(model, colW, nl.page))}`,
      }).catch(() => {});
    };

    /** One mutation through the command layer → re-render + re-evidence.
     * BOTH the keydown handler and the gate probe go through here — the
     * real edit path, never a shortcut. */
    const edit = async (cmd: EditCmd): Promise<void> => {
      try {
        const out = await invoke<EditOutcome>('nano_light_edit', { cmd });
        body = out.doc.body;
        caret = out.caret;
        dirty = out.dirty;
        updateStatus();
        render();
        logLayoutEvidence();
      } catch (e) {
        await invoke('frontend_log', { level: 'error', message: `nano light failed: edit ${e}` });
      }
    };

    const save = async (): Promise<void> => {
      try {
        await invoke('nano_light_save');
        dirty = false;
        updateStatus();
      } catch (e) {
        await invoke('frontend_log', { level: 'error', message: `nano light failed: save ${e}` });
      }
    };

    // caret via click = the hit-test inverse mapping, on the live model
    cv.addEventListener('mousedown', (ev) => {
      if (!model) return;
      const h = hitTest(model, measure, ev.offsetX, ev.offsetY);
      if (h) {
        caret = { para: h.para, run: h.run, offset: h.offset, cell: h.cell };
        render();
      }
    });
    saveBtn.addEventListener('click', () => void save());
    window.addEventListener('keydown', (ev) => {
      const meta = ev.metaKey || ev.ctrlKey;
      if (meta && ev.key.toLowerCase() === 'z') {
        ev.preventDefault();
        void edit({ kind: ev.shiftKey ? 'redo' : 'undo' });
        return;
      }
      if (meta && ev.key.toLowerCase() === 's') {
        ev.preventDefault();
        void save();
        return;
      }
      if (meta || ev.altKey) return;
      switch (ev.key) {
        case 'Backspace':
          ev.preventDefault();
          void edit({ kind: 'backspace', caret });
          break;
        case 'Delete':
          ev.preventDefault();
          void edit({ kind: 'delete', caret });
          break;
        case 'Enter':
          ev.preventDefault();
          void edit({ kind: 'enter', caret });
          break;
        default:
          if (ev.key.length === 1) {
            ev.preventDefault();
            void edit({ kind: 'insert', text: ev.key, caret });
          }
      }
    });
    resizeRelayout(() => render());

    model = render();
    const ms = performance.now() - t0;
    await invoke('frontend_log', {
      level: 'info',
      message: `nano light first-draw ${ms.toFixed(1)} ms (parse ${nl.parseMs.toFixed(1)} ms, body blocks ${body.length}, canvas ${cv.style.height})`,
    });
    logLayoutEvidence();
    // media evidence from the first render only (text edits never move them)
    for (const media of model.media) {
      await invoke('frontend_log', {
        level: 'info',
        message: `nano light media: part=${media.part} emu=${media.emu[0]}x${media.emu[1]} drawn=${media.w.toFixed(1)}x${media.h.toFixed(1)} at=${media.x.toFixed(1)},${media.y.toFixed(1)} ink=${mediaInk(ctx, media, dpr)}`,
      });
    }

    // Gate-15 edit driver (pattern NANO_OFFICE_SAVE_PROBE): caret from a
    // genuine hitTest, then the SAME edit()/save() the handlers use, then a
    // reopen in a second window. Product default (no env): never runs.
    const probe = (window as unknown as { __NANO_OFFICE_NL_PROBE__?: string }).__NANO_OFFICE_NL_PROBE__;
    if (probe && model.lines.length > 0) {
      try {
        const first = model.lines[0];
        const h = hitTest(model, measure, first.x + first.total / 2, first.baseline);
        if (h) caret = { para: h.para, run: h.run, offset: h.offset, cell: h.cell };
        await edit({ kind: 'insert', text: probe, caret });
        await edit({ kind: 'insert', text: ' XY', caret });
        for (let i = 0; i < 3; i++) await edit({ kind: 'backspace', caret });
        await edit({ kind: 'enter', caret });
        await edit({ kind: 'insert', text: 'Nano Light paragraph two', caret });
        await edit({ kind: 'undo' });
        await edit({ kind: 'redo' });
        await save();
        await invoke('nano_light_reopen');
        await invoke('frontend_log', { level: 'info', message: 'nano light probe: chain done (edit → save → reopen)' });
      } catch (e) {
        await invoke('frontend_log', { level: 'error', message: `nano light failed: probe ${e}` });
      }
    }
  } catch (e) {
    cv.width = box.clientWidth * dpr;
    cv.height = box.clientHeight * dpr;
    cv.style.width = `${box.clientWidth}px`;
    cv.style.height = `${box.clientHeight}px`;
    ctx.scale(dpr, dpr);
    ctx.fillStyle = '#000';
    ctx.fillText(`nano light failed: ${e}`, MARGIN, MARGIN);
    await invoke('frontend_log', { level: 'error', message: `nano light failed: ${e}` });
  }
}

void main();
