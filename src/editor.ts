/**
 * L01 Nano Light editor slice (DD docs/design/editor-a4/DD.md) — the EDIT
 * side of the A3 viewer surface: the SAME pure layout module
 * (`src/viewer/layout.ts`, consumed VERBATIM — never forked), plus a caret
 * drawn from the hit-test coordinate system and a command-layer edit loop.
 * The `Document` model lives Rust-side in window-scoped sessions; every
 * mutation re-resolves server-side and returns a fresh render model to
 * redraw. Typing is plain `keydown` (no synthetic keystrokes anywhere —
 * rule 6b); the gate-15 probe (NANO_OFFICE_NL_PROBE) drives the SAME edit()
 * the keydown handler uses. No engine shim is loaded on this page.
 */
import {
  hitTest,
  layoutBlocks,
  MARGIN,
  PT_PX,
  runFont,
  twip,
  type Block,
  type FontMetrics,
  type LayoutItem,
  type LineModel,
  type Measurer,
  type Run,
} from './viewer/layout';

interface Caret {
  para: number;
  run: number;
  offset: number;
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

// V5 scroll ceiling inherited: WebKit caps canvas at 16384 device px (8192
// logical @dpr2). Pagination is post-A3 work.
const MAX_CANVAS_H = 8192;

/** Image OPC parts in document order (deduped) — same walk as the shell's. */
function imageParts(blocks: Block[], out: Set<string>): void {
  for (const block of blocks) {
    if ('Paragraph' in block) {
      for (const r of block.Paragraph.runs) {
        for (const img of r.images) out.add(img.part);
      }
    } else {
      for (const row of block.Table.rows) {
        for (const cell of row) imageParts(cell.blocks, out);
      }
    }
  }
}

/** Fetch every image part once (loopback filesrv, keyed by fileId — no disk
 * paths in URLs). Failures log and leave the part absent. */
async function loadMedia(blocks: Block[], base: string, bitmaps: Map<string, ImageBitmap>): Promise<void> {
  const parts = new Set<string>();
  imageParts(blocks, parts);
  await Promise.all(
    [...parts].map(async (part) => {
      try {
        const resp = await fetch(`${base}/${encodeURI(part)}`);
        if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
        bitmaps.set(part, await createImageBitmap(await resp.blob()));
      } catch (e) {
        await invoke('frontend_log', { level: 'warn', message: `nano light media-failed: part=${part} err=${e}` });
      }
    }),
  );
}

const itemLen = (it: LayoutItem): number => (it.tab ? 1 : it.text.length);

/**
 * Forward map (para, run, offset) → (x, baseline, line-height) — the mirror
 * of `hitTest` on the SAME model + measurer: the item whose run/charStart
 * contains the offset (caret BEFORE char `offset`); end-of-run clamps to the
 * LAST item of that run (visual end of a wrapped run); an empty paragraph
 * resolves to its line start (hitTest's placeholder semantics).
 */
function caretXY(model: LineModel, measure: Measurer, caret: Caret): { x: number; y: number; lh: number } | null {
  let fallback: { x: number; y: number; lh: number } | null = null;
  for (const line of model.lines) {
    if (line.para !== caret.para) continue;
    if (line.items.length === 0) {
      fallback ??= { x: line.x, y: line.baseline, lh: line.lh };
      continue;
    }
    for (const it of line.items) {
      if (it.ri !== caret.run) continue;
      const end = it.charStart + itemLen(it);
      if (caret.offset >= it.charStart && caret.offset < end) {
        const x = it.tab ? it.x : it.x + measure(it.r, it.text.slice(0, caret.offset - it.charStart));
        return { x, y: line.baseline, lh: line.lh };
      }
      if (caret.offset === end) {
        fallback = { x: it.x + it.w, y: line.baseline, lh: line.lh };
      }
    }
  }
  return fallback;
}

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
    if (nl.mediaBase) await loadMedia(body, nl.mediaBase, bitmaps);
    const measure: Measurer = (r: Run, text: string): number => {
      ctx.font = runFont(r);
      return ctx.measureText(text).width;
    };
    const updateStatus = (): void => {
      dotEl.classList.toggle('hidden', !dirty);
    };

    const render = (): LineModel => {
      const pageW = nl.page?.w ? twip(nl.page.w) : box.clientWidth;
      colW = Math.min(pageW, box.clientWidth) - 2 * MARGIN;
      const m = layoutBlocks({
        blocks: body,
        x0: MARGIN,
        w: colW,
        y: MARGIN + 16,
        fonts: nl.fonts,
        measure,
      });
      const docH = Math.min(Math.max(Math.ceil(m.bottom) + MARGIN, box.clientHeight), MAX_CANVAS_H);
      cv.width = box.clientWidth * dpr;
      cv.height = docH * dpr;
      cv.style.width = `${box.clientWidth}px`;
      cv.style.height = `${docH}px`;
      ctx.scale(dpr, dpr);
      draw(m);
      model = m;
      return m;
    };

    /** Draw the model + the caret (a static rect — blink is a later slice,
     * DD L01 §2). Same per-item sequence as the viewer's draw. */
    const draw = (m: LineModel): void => {
      for (const media of m.media) {
        if (bitmaps.has(media.part)) {
          ctx.drawImage(bitmaps.get(media.part)!, media.x, media.y, media.w, media.h);
        } else {
          ctx.strokeStyle = '#bbb';
          ctx.strokeRect(media.x, media.y, media.w, media.h);
        }
      }
      for (const line of m.lines) {
        let x = line.x;
        for (const it of line.items) {
          ctx.font = runFont(it.r);
          ctx.fillStyle = it.r.color ? `#${it.r.color}` : '#000';
          if (it.r.spacing != null) ctx.letterSpacing = `${(it.r.spacing / 20) * PT_PX}px`;
          if (it.text) ctx.fillText(it.text, x, line.baseline);
          if (it.r.spacing != null) ctx.letterSpacing = '0px';
          if (it.r.underline && it.text) ctx.fillRect(x, line.baseline + 2, it.w, 1);
          x += it.w;
        }
      }
      const c = caretXY(m, measure, caret);
      if (c) {
        ctx.fillStyle = '#000';
        ctx.fillRect(c.x - 0.75, c.y - c.lh + 2, 1.5, c.lh - 1);
      }
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
        message: `nano light evidence: ${JSON.stringify({
          colw: colW,
          page_w: nl.page?.w ?? null,
          lines: model.lines.map((l) => {
            const ev: { pi: number; x: number; y: number; t: string; al: string | null; c?: number } = {
              pi: l.para,
              x: l.x,
              y: l.baseline,
              t: l.items.map((it) => (it.tab ? '\t' : it.text)).join(''),
              al: l.align,
            };
            if (l.inCell) ev.c = 1;
            return ev;
          }),
        })}`,
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
        caret = { para: h.para, run: h.run, offset: h.offset };
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
    // resize re-layout (the canvas used to go stale) — rAF coalesces
    let raf = 0;
    window.addEventListener('resize', () => {
      cancelAnimationFrame(raf);
      raf = requestAnimationFrame(() => render());
    });

    model = render();
    const ms = performance.now() - t0;
    await invoke('frontend_log', {
      level: 'info',
      message: `nano light first-draw ${ms.toFixed(1)} ms (parse ${nl.parseMs.toFixed(1)} ms, body blocks ${body.length}, canvas ${cv.style.height})`,
    });
    logLayoutEvidence();
    // media evidence from the first render only (text edits never move them)
    for (const media of model.media) {
      const d = Math.max(1, Math.round(media.w * dpr));
      const dv = Math.max(1, Math.round(media.h * dpr));
      const px = ctx.getImageData(Math.round(media.x * dpr), Math.round(media.y * dpr), d, dv).data;
      let ink = 0;
      for (let i = 0; i < px.length; i += 4) {
        if (px[i + 3] > 0 && !(px[i] > 250 && px[i + 1] > 250 && px[i + 2] > 250)) ink++;
      }
      await invoke('frontend_log', {
        level: 'info',
        message: `nano light media: part=${media.part} emu=${media.emu[0]}x${media.emu[1]} drawn=${media.w.toFixed(1)}x${media.h.toFixed(1)} at=${media.x.toFixed(1)},${media.y.toFixed(1)} ink=${ink}`,
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
        if (h) caret = { para: h.para, run: h.run, offset: h.offset };
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
