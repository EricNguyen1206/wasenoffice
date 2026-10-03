/**
 * L01 Nano Light editor slice (DD docs/design/editor-a4/DD.md) — the EDIT
 * side of the A3 viewer surface: the SAME pure layout module
 * (`src/viewer/layout.ts`, consumed VERBATIM — never forked), plus caret and
 * selection drawn from the hit-test coordinate system and a command-layer
 * edit loop. The canvas/media/evidence machinery lives in
 * `src/viewer/render.ts` (shared with the viewer since T-060 C1); this entry
 * owns the session state, the edit commands, the caret/selection overlays
 * and the save button. The `Document` model lives Rust-side in
 * window-scoped sessions; every mutation re-resolves server-side and returns
 * a fresh render model to redraw. T-A28 (DD §5.3): text SELECTION —
 * anchor+focus carets ordered by (para, run, offset), keyboard navigation
 * (arrows with goal-x, Home/End on the layout line, Cmd+Left/Right paragraph
 * home/end, shift extends), mouse drag + shift+click, a per-line highlight
 * in the drawModel overlay, and RANGE-AWARE mutations (Rust deletes the
 * selection before the verb; undo snapshots carry the selection). The
 * caret/selection `para` is the BODY BLOCK index (`Hit.block` — §5.3a;
 * `caretXY` matches on block). Typing is plain `keydown` (no synthetic
 * keystrokes anywhere — rule 6b); the gate-15 probe (NANO_OFFICE_NL_PROBE)
 * drives the SAME edit()/navigation the handlers use. No engine shim is
 * loaded on this page.
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
import {
  caretXY,
  hitTest,
  layoutBlocks,
  MARGIN,
  type Block,
  type FontMetrics,
  type LineModel,
  type Run,
} from './viewer/layout';

interface Caret {
  /** The BODY BLOCK index (DD L01 §5.3a — `Hit.block`; `Hit.para`'s `pi`
   * shares values with table-cell content and cannot address the body). */
  para: number;
  run: number;
  offset: number;
  /** T-060 C3: the hit's cell identity — `[row, col]` when the caret sits
   * inside a table, null at top level. The command layer refuses edits on
   * `cell !== null` (round-1 scope, DD L01 §2). */
  cell: [number, number] | null;
}

/** T-A28 §5.3a: a text selection — anchor (gesture start) + focus (the
 * moving end), UNORDERED; ordering happens at use time via the
 * (para, run, offset) key. anchor == focus ≡ plain caret (sel = null).
 * Rust mirror: src-tauri/src/editor.rs `SelRange` (C5 — one camelCase
 * contract home). */
interface SelRange {
  anchor: Caret;
  focus: Caret;
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
  /** Collapsed after a mutation; restored by undo/redo (DD L01 §5.3a). */
  sel: SelRange | null;
}

type EditCmd =
  | { kind: 'insert'; text: string; caret: Caret; sel: SelRange | null }
  | { kind: 'backspace'; caret: Caret; sel: SelRange | null }
  | { kind: 'delete'; caret: Caret; sel: SelRange | null }
  | { kind: 'enter'; caret: Caret; sel: SelRange | null }
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
    let sel: SelRange | null = null;
    /** Up/Down goal column: the caret x when a vertical gesture starts, kept
     * until a horizontal action, a click or a mutation (DD L01 §5.3b). */
    let goalX: number | null = null;
    let dragging = false;
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
      // the selection + caret overlay (static caret — blink is a later
      // slice, DD L01 §2/§10)
      drawModel(m, cv, ctx, dpr, box.clientWidth, box.clientHeight, bitmaps, (c) => {
        drawSelection(c, m);
        const at = caretXY(m, measure, caret);
        if (at) {
          c.fillStyle = '#000';
          c.fillRect(at.x - 0.75, at.y - at.lh + 2, 1.5, at.lh - 1);
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

    // ── selection + navigation (DD L01 §5.3a/b) — TS-local: movement never
    // mutates the model, so no command round-trip for pure caret moves. ──

    const cmpCaret = (a: Caret, b: Caret): number =>
      a.para - b.para || a.run - b.run || a.offset - b.offset;
    const sameCaret = (a: Caret, b: Caret): boolean => cmpCaret(a, b) === 0;

    const runsOf = (para: number): Run[] | null => {
      const b = body[para];
      return b && 'Paragraph' in b ? b.Paragraph.runs : null;
    };

    /** One character step (a surrogate pair = one step — mirrors the Rust
     * backspace), across run and paragraph boundaries; table boundaries and
     * the document edges are no-ops (round-1 ceiling: arrows do not jump
     * OVER a table). */
    const stepH = (c: Caret, dir: 1 | -1): Caret => {
      const runs = runsOf(c.para);
      const run = runs?.[c.run];
      if (dir === -1) {
        if (run && c.offset > 0) {
          const low = run.text.charCodeAt(c.offset - 1);
          const back = low >= 0xdc00 && low <= 0xdfff && c.offset >= 2 ? 2 : 1;
          return { ...c, offset: c.offset - back };
        }
        if (runs && c.run > 0) {
          return { ...c, run: c.run - 1, offset: runs[c.run - 1].text.length };
        }
      } else if (run && c.offset < run.text.length) {
        const unit = run.text.charCodeAt(c.offset);
        const fwd = unit >= 0xd800 && unit <= 0xdbff ? 2 : 1;
        return { ...c, offset: Math.min(c.offset + fwd, run.text.length) };
      } else if (runs && c.run + 1 < runs.length) {
        return { ...c, run: c.run + 1, offset: 0 };
      }
      for (let p = c.para + dir; p >= 0 && p < body.length; p += dir) {
        const rp = runsOf(p);
        if (!rp) return c; // table boundary — stop
        if (rp.length === 0) return { para: p, run: 0, offset: 0, cell: null };
        return dir === -1
          ? { para: p, run: rp.length - 1, offset: rp[rp.length - 1].text.length, cell: null }
          : { para: p, run: 0, offset: 0, cell: null };
      }
      return c; // document edge
    };

    /** Line-to-line move (Up/Down) keeping goal-x — hitTest clamps the
     * column into each line's own span. In-cell lines are SKIPPED
     * (selection never enters tables, §5.3a); image rows live in `media`,
     * not `lines`, and are stepped over (ceiling §5.3g). */
    const moveV = (c: Caret, dir: 1 | -1): Caret => {
      if (!model) return c;
      const cur = caretXY(model, measure, c);
      if (!cur) return c;
      goalX ??= cur.x;
      let k = cur.line + dir;
      while (k >= 0 && k < model.lines.length && model.lines[k].cell) k += dir;
      if (k < 0 || k >= model.lines.length) return c;
      const h = hitTest(model, measure, goalX, model.lines[k].baseline);
      return h ? { para: h.block, run: h.run, offset: h.offset, cell: h.cell } : c;
    };

    /** Home/End: start/end of the current LAYOUT line (not the paragraph). */
    const lineHomeEnd = (c: Caret, end: boolean): Caret => {
      if (!model) return c;
      const cur = caretXY(model, measure, c);
      if (!cur) return c;
      const line = model.lines[cur.line];
      const h = hitTest(model, measure, end ? line.x + line.total : line.x, line.baseline);
      return h ? { para: h.block, run: h.run, offset: h.offset, cell: h.cell } : c;
    };

    /** Cmd+Left/Right: paragraph start/end through the body runs. */
    const paraHomeEnd = (c: Caret, end: boolean): Caret => {
      const runs = runsOf(c.para);
      if (!end && runs) return { para: c.para, run: 0, offset: 0, cell: null };
      if (!runs || runs.length === 0) return { para: c.para, run: 0, offset: 0, cell: null };
      return {
        para: c.para,
        run: runs.length - 1,
        offset: runs[runs.length - 1].text.length,
        cell: null,
      };
    };

    /** Apply a caret move: plain = collapse the selection; shift = extend it
     * (the anchor sticks). A selection endpoint inside a table refuses to be
     * part of a selection (§5.3a) — the selection collapses to a plain
     * caret at the moved-to position. `vertical` keeps goal-x alive. */
    const moveTo = (next: Caret, extend: boolean, vertical = false): void => {
      if (extend) {
        const anchor = sel ? sel.anchor : caret;
        sel =
          next.cell || anchor.cell || sameCaret(anchor, next)
            ? null
            : { anchor, focus: next };
      } else {
        sel = null;
        if (!vertical) goalX = null;
      }
      caret = next;
      render();
    };

    /** `nano light selection:` — ADDITIVE gate-15 evidence (frozen constant
     * SMOKE_RE_NL_SEL in harness/scripts/smoke-lib.sh — change in the SAME
     * commit). span = char distance between the ORDERED endpoints (same
     * paragraph; cross-para = -1). */
    const emitSel = (): void => {
      if (!sel) return;
      const [a, b] =
        cmpCaret(sel.anchor, sel.focus) <= 0 ? [sel.anchor, sel.focus] : [sel.focus, sel.anchor];
      let span = -1;
      if (a.para === b.para) {
        const runs = runsOf(a.para);
        if (runs) {
          if (a.run === b.run) {
            span = b.offset - a.offset;
          } else {
            span = 0;
            for (let r = a.run; r <= b.run; r++) {
              const len = runs[r]?.text.length ?? 0;
              span += r === a.run ? len - a.offset : r === b.run ? b.offset : len;
            }
          }
        }
      }
      void invoke('frontend_log', {
        level: 'info',
        message: `nano light selection: anchor=${sel.anchor.para}/${sel.anchor.run}/${sel.anchor.offset} focus=${sel.focus.para}/${sel.focus.run}/${sel.focus.offset} span=${span}`,
      });
    };

    /** Per-line highlight rects for the ordered [start, end] range —
     * computed from the SAME LineModel the text is drawn from (the overlay
     * of drawModel; layout.ts untouched, DD L01 §5.3d). A line is skipped
     * when outside the range; partial lines take their x bounds from the
     * endpoints' caretXY; a zero-width rect on a swept empty paragraph
     * becomes a 2px sliver. */
    const drawSelection = (c: CanvasRenderingContext2D, m: LineModel): void => {
      if (!sel || sameCaret(sel.anchor, sel.focus)) return;
      const [s, e] =
        cmpCaret(sel.anchor, sel.focus) <= 0 ? [sel.anchor, sel.focus] : [sel.focus, sel.anchor];
      c.fillStyle = 'rgba(0, 120, 255, 0.28)';
      for (const line of m.lines) {
        if (line.cell) continue; // selection never enters tables (§5.3a)
        const first = line.items[0];
        const last = line.items[line.items.length - 1];
        const lineStart: Caret = {
          para: line.block,
          run: first?.ri ?? 0,
          offset: first?.charStart ?? 0,
          cell: null,
        };
        const lineEnd: Caret = {
          para: line.block,
          run: last?.ri ?? 0,
          offset: last ? last.charStart + (last.tab ? 1 : last.text.length) : 0,
          cell: null,
        };
        if (cmpCaret(e, lineStart) < 0 || cmpCaret(lineEnd, s) < 0) continue; // outside
        const x1 = cmpCaret(s, lineStart) < 0 ? line.x : caretXY(m, measure, s)?.x ?? line.x;
        const x2 =
          cmpCaret(lineEnd, e) < 0 ? line.x + line.total : caretXY(m, measure, e)?.x ?? line.x + line.total;
        const w = x2 - x1;
        if (w > 0) {
          c.fillRect(x1, line.baseline - line.lh + 2, w, line.lh - 1);
        } else if (cmpCaret(s, lineStart) <= 0 && cmpCaret(lineEnd, e) <= 0) {
          c.fillRect(line.x, line.baseline - line.lh + 2, 2, line.lh - 1); // empty para swept
        }
      }
    };

    /** One mutation through the command layer → re-render + re-evidence.
     * BOTH the keydown handler and the gate probe go through here — the
     * real edit path, never a shortcut. The live selection rides along
     * (Rust deletes a non-empty one before the verb); the outcome's sel
     * (collapsed, or restored by undo/redo) becomes the new state. */
    const edit = async (cmd: EditCmd): Promise<void> => {
      try {
        const out = await invoke<EditOutcome>('nano_light_edit', { cmd });
        body = out.doc.body;
        caret = out.caret;
        dirty = out.dirty;
        sel = out.sel;
        goalX = null;
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

    // ── mouse selection (§5.3b): the gate probe calls these three directly
    // — synthetic MouseEvents cannot carry offsetX/Y in WebKit ──
    const hitAt = (x: number, y: number): Caret | null => {
      if (!model) return null;
      const h = hitTest(model, measure, x, y);
      return h ? { para: h.block, run: h.run, offset: h.offset, cell: h.cell } : null;
    };
    const mouseDownAt = (x: number, y: number, extend: boolean): void => {
      const next = hitAt(x, y);
      if (!next) return;
      dragging = true;
      moveTo(next, extend);
    };
    const mouseMoveAt = (x: number, y: number): void => {
      if (!dragging) return;
      const next = hitAt(x, y);
      if (next) moveTo(next, true);
    };
    const mouseUp = (): void => {
      if (!dragging) return;
      dragging = false;
      emitSel(); // once per gesture — mousemove would spam the log
    };
    cv.addEventListener('mousedown', (ev) => mouseDownAt(ev.offsetX, ev.offsetY, ev.shiftKey));
    cv.addEventListener('mousemove', (ev) => mouseMoveAt(ev.offsetX, ev.offsetY));
    window.addEventListener('mouseup', () => mouseUp());
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
      const nav = (next: Caret, vertical = false): void => {
        ev.preventDefault();
        moveTo(next, ev.shiftKey, vertical);
        if (ev.shiftKey) emitSel();
      };
      if (meta && ev.key === 'ArrowLeft') return nav(paraHomeEnd(caret, false));
      if (meta && ev.key === 'ArrowRight') return nav(paraHomeEnd(caret, true));
      if (meta || ev.altKey) return;
      switch (ev.key) {
        case 'ArrowLeft':
          nav(stepH(caret, -1));
          break;
        case 'ArrowRight':
          nav(stepH(caret, 1));
          break;
        case 'ArrowUp':
          nav(moveV(caret, -1), true);
          break;
        case 'ArrowDown':
          nav(moveV(caret, 1), true);
          break;
        case 'Home':
          nav(lineHomeEnd(caret, false));
          break;
        case 'End':
          nav(lineHomeEnd(caret, true));
          break;
        case 'Backspace':
          ev.preventDefault();
          void edit({ kind: 'backspace', caret, sel });
          break;
        case 'Delete':
          ev.preventDefault();
          void edit({ kind: 'delete', caret, sel });
          break;
        case 'Enter':
          ev.preventDefault();
          void edit({ kind: 'enter', caret, sel });
          break;
        default:
          if (ev.key.length === 1) {
            ev.preventDefault();
            void edit({ kind: 'insert', text: ev.key, caret, sel });
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
    // genuine hitTest, then the SAME edit()/navigation the handlers use,
    // then a reopen in a second window. Product default (no env): never
    // runs. The selection chain dispatches REAL KeyboardEvents through the
    // keydown handler (shift+arrows extend, no keystrokes — rule 6b) and
    // replaces the selection through edit() with the live state.
    const probe = (window as unknown as { __NANO_OFFICE_NL_PROBE__?: string }).__NANO_OFFICE_NL_PROBE__;
    if (probe && model.lines.length > 0) {
      try {
        const first = model.lines[0];
        const next = hitAt(first.x + first.total / 2, first.baseline);
        if (next) caret = next;
        await edit({ kind: 'insert', text: probe, caret, sel: null });
        await edit({ kind: 'insert', text: ' XY', caret, sel: null });
        for (let i = 0; i < 3; i++) await edit({ kind: 'backspace', caret, sel: null });
        await edit({ kind: 'enter', caret, sel: null });
        await edit({ kind: 'insert', text: 'Nano Light paragraph two', caret, sel: null });
        await edit({ kind: 'undo' });
        await edit({ kind: 'redo' });
        // selection: the redo caret sits at the END of the probe's line-2
        // insert — 4×shift+Left selects " two" → type-replace → undo
        // (selection restored) → redo → save → reopen (gate 15 asserts the
        // span evidence + "paragraphT28" in the crate parse-back)
        for (let i = 0; i < 4; i++) {
          window.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft', shiftKey: true }));
        }
        await edit({ kind: 'insert', text: 'T28', caret, sel });
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
