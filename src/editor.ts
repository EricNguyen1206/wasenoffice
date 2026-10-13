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
 * keystrokes anywhere — rule 6b); the gate-15 probe (WASEN_OFFICE_NL_PROBE)
 * drives the SAME edit()/navigation the handlers use. No engine shim is
 * loaded on this page. T-A29 (DD §5.4): IME composition (preedit overlay,
 * commit = 1 insert = 1 undo unit), plain-text clipboard (Cmd-C/X/V via
 * `navigator.clipboard` + a session stash fallback) and Cmd-A select-all.
 * T-A30 (DD §5.5): SINGLE-CELL table editing — carets carry `cellPara`
 * (paragraph index inside the cell), the same-cell selection rule, arrows
 * that move within the cell's lines, double-click word / triple-click
 * paragraph selection (manual click count — WebKit has no triple event),
 * and the 500 ms caret blink (repaint from the cached model, solid while
 * composing).
 */
import {
  layoutMetricLine,
  loadMedia,
  mediaEvidenceLine,
  resizeRelayout,
  toEvidence,
} from './viewer/render';
import {
  caretXY,
  hitTest,
  type Block,
  type FontMetrics,
  type LineModel,
  type Run,
  type TextLine,
} from './viewer/layout';
import { DocumentSurface } from './viewer/surface';

interface Caret {
  /** The BODY BLOCK index (DD L01 §5.3a — `Hit.caret.para`; the layout
   * walk `pi` shares values with table-cell content and cannot address
   * the body). */
  para: number;
  run: number;
  offset: number;
  /** T-060 C3: the hit's cell identity — `[row, col]` when the caret sits
   * inside a table, null at top level. */
  cell: [number, number] | null;
  /** T-A30 §5.5a: the paragraph's index inside the cell's blocks (null at
   * top level; null with a cell = nested-table content, refused Rust-side). */
  cellPara: number | null;
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
  /** `w:pgMar` of the final section (T-V6) — text origin + wrap margins. */
  mar: { top: number | null; right: number | null; bottom: number | null; left: number | null } | null;
  /** `<origin>/media/<fileId>` prefix (loopback filesrv); null = media off. */
  mediaBase: string | null;
  caret: Caret;
  name: string;
  /** T-A31 refuse-warn: constructs the save will drop (DD L01 §5.6d). */
  warnings: string[];
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
  | { kind: 'format'; bold?: boolean; italic?: boolean; underline?: boolean; caret: Caret; sel: SelRange }
  | { kind: 'undo' }
  | { kind: 'redo' };
interface TabState {
  tabId: number;
  name: string;
  body: Block[];
  caret: Caret;
  sel: SelRange | null;
  dirty: boolean;
  fonts: FontMetrics;
  page: { w: number | null; h: number | null; orient: string | null } | null;
  mar: { top: number | null; right: number | null; bottom: number | null; left: number | null } | null;
  bitmaps: Map<string, ImageBitmap>;
}

const { invoke } = (window as unknown as { __TAURI__: { core: { invoke: <T>(cmd: string, args?: unknown) => Promise<T> } } }).__TAURI__.core;

async function main(): Promise<void> {
  const t0 = performance.now();
  let surface: DocumentSurface | null = null;
  const nameEl = document.getElementById('doc-name')!;
  const dotEl = document.getElementById('dirty-dot')!;
  const saveBtn = document.querySelector<HTMLButtonElement>('#save-btn')!;
  const formatBar = document.getElementById('format-toolbar') as HTMLElement | null;
  const fmtBold = document.getElementById('fmt-bold') as HTMLButtonElement | null;
  const fmtItalic = document.getElementById('fmt-italic') as HTMLButtonElement | null;
  const fmtUnderline = document.getElementById('fmt-underline') as HTMLButtonElement | null;
  const tabs = new Map<number, TabState>();
  let activeTabId = 0;

  try {
    surface = new DocumentSurface('cv');
    const nl = await invoke<NanoLightDoc>('wasen_light_open');
    nameEl.textContent = nl.name;
    surface.showWarnings(nl.warnings);
    let body = nl.doc.body;
    let caret = nl.caret;
    let sel: SelRange | null = null;
    /** Up/Down goal column: the caret x when a vertical gesture starts, kept
     * until a horizontal action, a click or a mutation (DD L01 §5.3b). */
    let goalX: number | null = null;
    let dragging = false;
    let dirty = false;
    let composing = false;
    let preedit = '';
    /** In-page clipboard fallback — WKWebView gates `readText`/`writeText` on user activation (DD §5.4b). */
    let clipStash: string | null = null;
    let model: LineModel | null = null;
    let colW = 0;
    const bitmaps = new Map<string, ImageBitmap>();
    if (nl.mediaBase) await loadMedia(body, nl.mediaBase, bitmaps, 'nano light', invoke);
    const measure = surface.measure;
    tabs.set(0, {
      tabId: 0,
      name: nl.name,
      body: nl.doc.body,
      caret: nl.caret,
      sel: null,
      dirty: false,
      fonts: nl.fonts,
      page: nl.page,
      mar: nl.mar,
      bitmaps,
    });

    const renderTabBar = (): void => {
      const tabBar = document.getElementById('tab-bar');
      if (!tabBar) return;
      tabBar.innerHTML = '';

      for (const tab of tabs.values()) {
        const isActive = tab.tabId === activeTabId;
        const tabEl = document.createElement('div');
        tabEl.className = `flex items-center gap-1.5 px-3 py-1 rounded-t-panel cursor-pointer text-xs border-b-2 transition-colors ${
          isActive
            ? 'border-accent bg-panel font-semibold text-fg shadow-xs'
            : 'border-transparent text-fg/60 hover:bg-panel-active hover:text-fg'
        }`;

        const label = document.createElement('span');
        label.className = 'truncate max-w-[140px]';
        label.textContent = tab.name;
        tabEl.appendChild(label);

        if (tab.dirty) {
          const dot = document.createElement('span');
          dot.className = 'h-1.5 w-1.5 rounded-full bg-accent shrink-0';
          tabEl.appendChild(dot);
        }

        if (tabs.size > 1) {
          const closeBtn = document.createElement('button');
          closeBtn.className = 'hover:bg-panel-active rounded p-0.5 text-fg/50 hover:text-fg text-[11px] leading-none';
          closeBtn.textContent = '×';
          closeBtn.onclick = (e) => {
            e.stopPropagation();
            void closeTab(tab.tabId);
          };
          tabEl.appendChild(closeBtn);
        }

        tabEl.onclick = () => switchTab(tab.tabId);
        tabBar.appendChild(tabEl);
      }
    };

    const switchTab = (tabId: number): void => {
      if (tabId === activeTabId) return;
      const cur = tabs.get(activeTabId);
      if (cur) {
        cur.body = body;
        cur.caret = caret;
        cur.sel = sel;
        cur.dirty = dirty;
      }
      const next = tabs.get(tabId);
      if (!next) return;
      activeTabId = tabId;
      body = next.body;
      caret = next.caret;
      sel = next.sel;
      dirty = next.dirty;
      nameEl.textContent = next.name;
      updateStatus();
      render();
      updateFormatToolbar();
    };

    const closeTab = async (tabId: number): Promise<void> => {
      if (tabs.size <= 1) return;
      tabs.delete(tabId);
      void invoke('wasen_light_close_tab', { tabId }).catch(() => {});
      if (activeTabId === tabId) {
        const first = tabs.keys().next().value;
        if (first !== undefined) {
          switchTab(first);
          return;
        }
      }
      renderTabBar();
    };

    const updateStatus = (): void => {
      const cur = tabs.get(activeTabId);
      if (cur) cur.dirty = dirty;
      dotEl.classList.toggle('hidden', !dirty);
      renderTabBar();
    };
    updateStatus();
    /** Caret blink (§5.5f): 500 ms toggle, repaint from the cached model, solid while composing. */
    let caretOn = true;
    let blinkTimer = 0;
    const wakeCaret = (): void => {
      caretOn = true;
      window.clearInterval(blinkTimer);
      blinkTimer = window.setInterval(() => {
        caretOn = composing ? true : !caretOn;
        repaint();
      }, 500);
    };
    const drawOverlay = (c: CanvasRenderingContext2D, m: LineModel): void => {
      drawSelection(c, m);
      const at = caretXY(m, measure, caret);
      if (at && caretOn) {
        c.fillStyle = '#000';
        c.fillRect(at.x - 0.75, at.y - at.lh + 2, 1.5, at.lh - 1);
        if (preedit) {
          // measurer-only fallback run (layout.ts lineHeight precedent)
          const w = measure(
            runsAt(caret)?.[caret.run] ?? ({ text: '', font: null, size: null } as Run),
            preedit,
          );
          c.fillText(preedit, at.x, at.y);
          c.fillRect(at.x, at.y + 2, w, 1); // inline displacement is a ceiling (DD §5.4a)
        }
      }
    };
    const repaint = (): void => {
      if (model) surface!.draw(model, { bitmaps, overlay: (c) => drawOverlay(c, model!) });
    };

    const render = (): LineModel => {
      const res = surface!.layout({
        blocks: body,
        fonts: nl.fonts,
        page: nl.page,
        mar: nl.mar,
      });
      colW = res.colW;
      model = res.model;
      repaint();
      wakeCaret();
      return model;
    };

    /** Layout + evidence lines (ADDITIVE log contract with
     * harness/scripts/smoke-nano-light.sh — gate 15): change both in the
     * SAME commit. Same shapes as the viewer's lines. */
    const logLayoutEvidence = (): void => {
      if (!model) return;
      void invoke('frontend_log', {
        level: 'info',
        message: `nano light layout: ${layoutMetricLine(model, colW)}`,
      });
      void invoke('frontend_log', {
        level: 'info',
        message: `nano light evidence: ${JSON.stringify(toEvidence(model, colW, nl.page))}`,
      }).catch(() => {});
    };

    // ── selection + navigation (DD L01 §5.3a/b) — TS-local: movement never
    // mutates the model, so no command round-trip for pure caret moves. ──

    const cmpCaret = (a: Caret, b: Caret): number =>
      a.para - b.para || (a.cellPara ?? 0) - (b.cellPara ?? 0) || a.run - b.run || a.offset - b.offset;
    const sameCaret = (a: Caret, b: Caret): boolean => cmpCaret(a, b) === 0;
    /** The selection ordered [start, end] — the anchor/focus ordering invariant
     * (mirror of Rust `apply_delete_range`'s sort) in ONE place (review #6 C3);
     * null when there is no non-collapsed selection. */
    const ordered = (): [Caret, Caret] | null =>
      sel && !sameCaret(sel.anchor, sel.focus)
        ? cmpCaret(sel.anchor, sel.focus) <= 0
          ? [sel.anchor, sel.focus]
          : [sel.focus, sel.anchor]
        : null;

    /** The Block[] holding the caret's paragraph + its index there (top level → body+para; cell → cell.blocks+cellPara; nested = null). */
    const containerOf = (c: Caret): { blocks: Block[]; idx: number } | null => {
      if (c.cell) {
        if (c.cellPara == null) return null;
        const t = body[c.para];
        const row = t && 'Table' in t ? t.Table.rows[c.cell[0]] : undefined;
        const tc = row?.[c.cell[1]];
        return tc ? { blocks: tc.blocks, idx: c.cellPara } : null;
      }
      return { blocks: body, idx: c.para };
    };
    const runsAt = (c: Caret): Run[] | null => {
      const k = containerOf(c);
      const b = k?.blocks[k.idx];
      return b && 'Paragraph' in b ? b.Paragraph.runs : null;
    };
    const caretIn = (idx: number, run: number, offset: number, from: Caret): Caret =>
      from.cell
        ? { ...from, cellPara: idx, run, offset }
        : { para: idx, run, offset, cell: null, cellPara: null };

    /** One character step (surrogate pair = one step) across run/paragraph boundaries within the caret's OWN container (§5.5d). */
    const stepH = (c: Caret, dir: 1 | -1): Caret => {
      const k = containerOf(c);
      const runs = runsAt(c);
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
      if (!k) return c;
      for (let p = k.idx + dir; p >= 0 && p < k.blocks.length; p += dir) {
        const b = k.blocks[p];
        const rp = b && 'Paragraph' in b ? b.Paragraph.runs : null;
        if (!rp) return c; // table boundary — stop
        if (rp.length === 0) return caretIn(p, 0, 0, c);
        return dir === -1
          ? caretIn(p, rp.length - 1, rp[rp.length - 1].text.length, c)
          : caretIn(p, 0, 0, c);
      }
      return c; // container edge (cell boundary stops here — DD §5.5d)
    };

    /** Up/Down keeping goal-x: lines outside the caret's context are stepped over (top-level skips cells; in-cell stays in the cell); image rows live in `media` (ceiling). */
    const moveV = (c: Caret, dir: 1 | -1): Caret => {
      if (!model) return c;
      const cur = caretXY(model, measure, c);
      if (!cur) return c;
      goalX ??= cur.x;
      let k = cur.line + dir;
      while (k >= 0 && k < model.lines.length && !lineInCtx(c.cell, model.lines[k])) k += dir;
      if (k < 0 || k >= model.lines.length) return c;
      const h = hitTest(model, measure, goalX, model.lines[k].baseline);
      return h ? h.caret : c;
    };

    /** Home/End: start/end of the current LAYOUT line (not the paragraph). */
    const lineHomeEnd = (c: Caret, end: boolean): Caret => {
      if (!model) return c;
      const cur = caretXY(model, measure, c);
      if (!cur) return c;
      const line = model.lines[cur.line];
      const h = hitTest(model, measure, end ? line.x + line.total : line.x, line.baseline);
      return h ? h.caret : c;
    };

    /** Cmd+Left/Right: paragraph start/end through the caret's own runs. */
    const paraHomeEnd = (c: Caret, end: boolean): Caret => {
      const runs = end ? runsAt(c) : null;
      if (!runs || runs.length === 0) return { ...c, run: 0, offset: 0 };
      return { ...c, run: runs.length - 1, offset: runs[runs.length - 1].text.length };
    };

    /** A line belongs to the caret's edit context (§5.5c): top-level lines for a null cell, or the SAME cell's lines. */
    const lineInCtx = (cell: [number, number] | null, l: TextLine): boolean =>
      cell
        ? l.cell != null && l.cellPara != null && l.cell[0] === cell[0] && l.cell[1] === cell[1]
        : l.cell == null;
    /** Both endpoints share one edit surface (§5.5c) — anything else collapses to a plain caret. */
    const inSameCell = (a: Caret, b: Caret): boolean =>
      a.cell == null && b.cell == null
        ? true
        : a.cell != null &&
          b.cell != null &&
          a.cellPara != null &&
          b.cellPara != null &&
          a.cell[0] === b.cell[0] &&
          a.cell[1] === b.cell[1];

    /** Apply a caret move: plain = collapse the selection; shift = extend it
     * (the anchor sticks). `vertical` keeps goal-x alive. */
    const moveTo = (next: Caret, extend: boolean, vertical = false): void => {
      if (extend) {
        const anchor = sel ? sel.anchor : caret;
        sel =
          !inSameCell(anchor, next) || sameCaret(anchor, next) ? null : { anchor, focus: next };
      } else {
        sel = null;
        if (!vertical) goalX = null;
      }
      caret = next;
      render();
      updateFormatToolbar();
    };

    /** `nano light selection:` — ADDITIVE gate-15 evidence (frozen constant
     * SMOKE_RE_NL_SEL in harness/scripts/smoke-lib.sh — change in the SAME
     * commit). span = char distance between the ORDERED endpoints (same paragraph or cell; cross-paragraph = -1). */
    const emitSel = (): void => {
      updateFormatToolbar();
      const cur = sel;
      const pair = ordered();
      if (!cur || !pair) return;
      const [a, b] = pair;
      const samePara =
        (a.cell == null && b.cell == null && a.para === b.para) ||
        (a.cell != null && b.cell != null && a.cellPara != null && a.cellPara === b.cellPara);
      let span = -1;
      if (samePara) {
        const runs = runsAt(a);
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
        message: `nano light selection: anchor=${cur.anchor.para}/${cur.anchor.run}/${cur.anchor.offset} focus=${cur.focus.para}/${cur.focus.run}/${cur.focus.offset} span=${span}`,
      });
    };

    /** Per-line highlight rects for the ordered [start, end] range (overlay of drawModel; lines outside the selection's context are skipped). */
    const drawSelection = (c: CanvasRenderingContext2D, m: LineModel): void => {
      const se = ordered();
      if (!se) return;
      const [s, e] = se;
      c.fillStyle = 'rgba(0, 120, 255, 0.28)';
      for (const line of m.lines) {
        if (!lineInCtx(s.cell, line)) continue;
        const first = line.items[0];
        const last = line.items[line.items.length - 1];
        const lineStart: Caret = {
          para: line.block,
          run: first?.ri ?? 0,
          offset: first?.charStart ?? 0,
          cell: line.cell,
          cellPara: line.cellPara,
        };
        const lineEnd: Caret = {
          para: line.block,
          run: last?.ri ?? 0,
          offset: last ? last.charStart + (last.tab ? 1 : last.text.length) : 0,
          cell: line.cell,
          cellPara: line.cellPara,
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
        if (cmd.kind !== 'undo' && cmd.kind !== 'redo' && cmd.caret.cell) {
          // `nano light cell:` — ADDITIVE evidence (frozen SMOKE_RE_NL_CELL, same-commit rule)
          void invoke('frontend_log', {
            level: 'info',
            message: `nano light cell: op=${cmd.kind} row=${cmd.caret.cell[0]} col=${cmd.caret.cell[1]} para=${cmd.caret.cellPara}`,
          });
        }
        const out = await invoke<EditOutcome>('wasen_light_edit', { cmd, tabId: activeTabId });
        body = out.doc.body;
        caret = out.caret;
        dirty = out.dirty;
        sel = out.sel;
        goalX = null;
        updateStatus();
        render();
        updateFormatToolbar();
        logLayoutEvidence();
      } catch (e) {
        await invoke('frontend_log', { level: 'error', message: `nano light failed: edit ${e}` });
      }
    };

    const save = async (): Promise<void> => {
      try {
        await invoke('wasen_light_save', { tabId: activeTabId });
        dirty = false;
        updateStatus();
      } catch (e) {
        await invoke('frontend_log', { level: 'error', message: `nano light failed: save ${e}` });
      }
    };

    const updateFormatToolbar = (): void => {
      if (!formatBar) return;
      if (sel && (sel.anchor.para !== sel.focus.para || sel.anchor.run !== sel.focus.run || sel.anchor.offset !== sel.focus.offset)) {
        if (!model) return;
        const at = caretXY(model, measure, sel.focus) || caretXY(model, measure, sel.anchor);
        if (at) {
          const top = Math.max(8, at.y - at.lh - 38);
          const left = Math.min(surface!.container.clientWidth - 110, Math.max(12, at.x - 30));
          formatBar.style.top = `${top}px`;
          formatBar.style.left = `${left}px`;
          formatBar.classList.remove('hidden');
          return;
        }
      }
      formatBar.classList.add('hidden');
    };

    const applyFormat = async (opts: { bold?: boolean; italic?: boolean; underline?: boolean }): Promise<void> => {
      if (!sel) return;
      await edit({
        kind: 'format',
        bold: opts.bold,
        italic: opts.italic,
        underline: opts.underline,
        caret,
        sel,
      });
    };

    // ── IME composition (DD §5.4a) + plain-text clipboard + Cmd-A (§5.4b); the probe drives the SAME fns. ──
    const emitCompose = (state: string, data: string): void => {
      void invoke('frontend_log', {
        level: 'info',
        message: `nano light composition: state=${state} data=${data}`,
      });
    };
    const compositionStart = (): void => {
      composing = true;
      preedit = '';
      emitCompose('start', '');
      render();
    };
    const compositionUpdate = (data: string): void => {
      preedit = data;
      emitCompose('update', data);
      render();
    };
    const compositionEnd = (data: string): Promise<void> => {
      composing = false;
      preedit = '';
      emitCompose('end', data);
      render();
      return data ? edit({ kind: 'insert', text: data, caret, sel }) : Promise.resolve();
    };
    window.addEventListener('compositionstart', () => compositionStart());
    window.addEventListener('compositionupdate', (ev) => compositionUpdate(ev.data));
    window.addEventListener('compositionend', (ev) => void compositionEnd(ev.data));

    /** Plain text of the ordered selection — walked over the selection's own container; cross-para joins '\n'. */
    const selectionText = (): string | null => {
      const se = ordered();
      if (!se) return null;
      const [s, e] = se;
      const k = containerOf(s);
      if (!k) return null;
      const eIdx = e.cell ? (e.cellPara ?? 0) : e.para;
      const runsOfIdx = (i: number): Run[] | null => {
        const b = k.blocks[i];
        return b && 'Paragraph' in b ? b.Paragraph.runs : null;
      };
      const parts: string[] = [];
      for (let p = k.idx; p <= eIdx && p < k.blocks.length; p++) {
        const runs = runsOfIdx(p);
        if (!runs) continue;
        const t = (i: number): string => runs[i]?.text ?? '';
        if (p === k.idx && p === eIdx && s.run === e.run) {
          parts.push(t(s.run).slice(s.offset, e.offset));
        } else if (p === k.idx && p === eIdx) {
          parts.push(
            t(s.run).slice(s.offset) +
              runs.slice(s.run + 1, e.run).map((r) => r.text).join('') +
              t(e.run).slice(0, e.offset),
          );
        } else if (p === k.idx) {
          parts.push(t(s.run).slice(s.offset) + runs.slice(s.run + 1).map((r) => r.text).join(''));
        } else if (p === eIdx) {
          parts.push(runs.slice(0, e.run).map((r) => r.text).join('') + t(e.run).slice(0, e.offset));
        } else {
          parts.push(runs.map((r) => r.text).join(''));
        }
      }
      return parts.join('\n');
    };

    /** `nano light clipboard:` — ADDITIVE gate-15 evidence (frozen SMOKE_RE_NL_CLIP, smoke-lib — same-commit rule). */
    const emitClip = (op: string, chars: number, sys: string, text: string): void => {
      void invoke('frontend_log', {
        level: 'info',
        message: `nano light clipboard: op=${op} chars=${chars} sys=${sys} text=${text.split('\n').join('\\n')}`,
      });
    };
    const copySelection = async (op: 'copy' | 'copy-all' | 'cut'): Promise<boolean> => {
      const text = selectionText();
      if (text === null) return false;
      clipStash = text;
      let sys = 'fail';
      try {
        await navigator.clipboard.writeText(text);
        sys = 'ok';
      } catch {
        sys = 'fail'; // stash still serves the in-page round (DD §5.4b)
      }
      emitClip(op, text.length, sys, text);
      return true;
    };
    const cutSelection = async (): Promise<void> => {
      if (await copySelection('cut')) await edit({ kind: 'backspace', caret, sel });
    };
    const pasteFromClipboard = async (): Promise<void> => {
      let text: string | null = null;
      let sys = 'stash';
      try {
        text = await navigator.clipboard.readText();
        sys = 'system';
      } catch {
        text = null; // permission/activation-gated — the stash is the fallback
      }
      if (!text) {
        if (clipStash === null) return;
        text = clipStash;
        sys = 'stash';
      }
      const parts = text.replace(/\r\n?/g, '\n').split('\n');
      for (let i = 0; i < parts.length; i++) {
        if (i > 0) await edit({ kind: 'enter', caret, sel: null });
        if (parts[i]) await edit({ kind: 'insert', text: parts[i], caret, sel: i === 0 ? sel : null });
      }
      emitClip('paste', text.length, sys, text);
    };
    const selectAll = (): void => {
      let last = -1;
      for (let p = body.length - 1; p >= 0; p--) {
        const b = body[p];
        if (b && 'Paragraph' in b) {
          last = p;
          break;
        }
      }
      if (last < 0) return;
      const runs = (body[last] as { Paragraph: { runs: Run[] } }).Paragraph.runs;
      // Cmd-A stays top-level only (§5.5g) — cell content would be refused as a cross-cell span anyway
      const anchor: Caret = { para: 0, run: 0, offset: 0, cell: null, cellPara: null };
      const focus: Caret = {
        para: last,
        run: Math.max(runs.length - 1, 0),
        offset: runs.length ? runs[runs.length - 1].text.length : 0,
        cell: null,
        cellPara: null,
      };
      sel = sameCaret(anchor, focus) ? null : { anchor, focus };
      caret = focus;
      render();
      emitSel();
    };

    // ── mouse selection (§5.3b/§5.5e): the gate probe calls these directly
    // — synthetic MouseEvents cannot carry offsetX/Y in WebKit ──
    const hitAt = (x: number, y: number): Caret | null => {
      if (!model) return null;
      return hitTest(model, measure, x, y)?.caret ?? null;
    };
    /** Double-click = the word at the caret (non-space run over the layout's word/space tokens); triple = the whole paragraph. */
    const selectWordAt = (h: Caret): void => {
      const run = runsAt(h)?.[h.run];
      const text = run?.text ?? '';
      let sIdx = h.offset;
      let eIdx = h.offset;
      while (sIdx > 0 && text[sIdx - 1] !== ' ') sIdx--;
      while (eIdx < text.length && text[eIdx] !== ' ') eIdx++;
      sel = sIdx === eIdx ? null : { anchor: { ...h, offset: sIdx }, focus: { ...h, offset: eIdx } };
      caret = sel ? sel.focus : h;
      render();
      emitSel();
    };
    const selectParaAt = (h: Caret): void => {
      const runs = runsAt(h);
      const anchor: Caret = { ...h, run: 0, offset: 0 };
      const focus: Caret = {
        ...h,
        run: Math.max((runs?.length ?? 1) - 1, 0),
        offset: runs?.length ? runs[runs.length - 1].text.length : 0,
      };
      sel = sameCaret(anchor, focus) ? null : { anchor, focus };
      caret = focus;
      render();
      emitSel();
    };
    const mouseDownAt = (x: number, y: number, extend: boolean, clicks = 1): void => {
      const next = hitAt(x, y);
      if (!next) return;
      if (!extend && clicks >= 3) {
        selectParaAt(next);
        return;
      }
      if (!extend && clicks === 2) {
        selectWordAt(next);
        return;
      }
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
    /** Manual click count: WebKit has no triple event — mousedowns <500 ms and <4 px apart count up. */
    let lastClick = { t: 0, x: 0, y: 0, n: 0 };
    const clickCount = (x: number, y: number): number => {
      const now = performance.now();
      const n =
        now - lastClick.t < 500 && Math.abs(x - lastClick.x) < 4 && Math.abs(y - lastClick.y) < 4
          ? lastClick.n + 1
          : 1;
      lastClick = { t: now, x, y, n };
      return n;
    };
    surface.cv.addEventListener('mousedown', (ev: MouseEvent) => mouseDownAt(ev.offsetX, ev.offsetY, ev.shiftKey, clickCount(ev.offsetX, ev.offsetY)));
    surface.cv.addEventListener('mousemove', (ev: MouseEvent) => mouseMoveAt(ev.offsetX, ev.offsetY));
    window.addEventListener('mouseup', () => mouseUp());
    saveBtn.addEventListener('click', () => void save());
    fmtBold?.addEventListener('click', (ev) => {
      ev.preventDefault();
      void applyFormat({ bold: true });
    });
    fmtItalic?.addEventListener('click', (ev) => {
      ev.preventDefault();
      void applyFormat({ italic: true });
    });
    fmtUnderline?.addEventListener('click', (ev) => {
      ev.preventDefault();
      void applyFormat({ underline: true });
    });
    window.addEventListener('keydown', (ev) => {
      if (composing || ev.isComposing) return; // the IME owns the keys while composing (DD §5.4a)
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
      if (meta && ev.key.toLowerCase() === 'b') {
        ev.preventDefault();
        void applyFormat({ bold: true });
        return;
      }
      if (meta && ev.key.toLowerCase() === 'i') {
        ev.preventDefault();
        void applyFormat({ italic: true });
        return;
      }
      if (meta && ev.key.toLowerCase() === 'u') {
        ev.preventDefault();
        void applyFormat({ underline: true });
        return;
      }
      if (meta && ev.key.toLowerCase() === 'a') {
        ev.preventDefault();
        selectAll();
        return;
      }
      if (meta && ev.key.toLowerCase() === 'c') {
        ev.preventDefault();
        void copySelection('copy');
        return;
      }
      if (meta && ev.key.toLowerCase() === 'x') {
        ev.preventDefault();
        void cutSelection();
        return;
      }
      if (meta && ev.key.toLowerCase() === 'v') {
        ev.preventDefault();
        void pasteFromClipboard();
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
      message: `nano light first-draw ${ms.toFixed(1)} ms (parse ${nl.parseMs.toFixed(1)} ms, body blocks ${body.length}, canvas ${surface.cv.style.height})`,
    });
    logLayoutEvidence();
    // media evidence from the first render only (text edits never move them)
    for (const media of model.media) {
      await invoke('frontend_log', {
        level: 'info',
        message: `nano light media: ${mediaEvidenceLine(media, surface.ctx, surface.dpr)}`,
      });
    }

    // Gate-15 edit driver (pattern WASEN_OFFICE_SAVE_PROBE): caret from a
    // genuine hitTest, then the SAME edit()/navigation the handlers use,
    // then a reopen in a second window. Product default (no env): never
    // runs. The selection chain dispatches REAL KeyboardEvents through the
    // keydown handler (shift+arrows extend, no keystrokes — rule 6b) and
    // replaces the selection through edit() with the live state.
    const probe = (window as unknown as { __WASEN_OFFICE_NL_PROBE__?: string }).__WASEN_OFFICE_NL_PROBE__;
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
        // T-A29: composition + clipboard round + Cmd-A — dispatched events, SAME handlers (DD §5.4)
        window.dispatchEvent(new CompositionEvent('compositionstart'));
        window.dispatchEvent(new CompositionEvent('compositionupdate', { data: 'Xin chao' }));
        window.dispatchEvent(new CompositionEvent('compositionupdate', { data: 'Xin chào' }));
        await compositionEnd('Xin chào');
        await edit({ kind: 'undo' });
        await edit({ kind: 'redo' });
        for (let i = 0; i < 8; i++) {
          window.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft', shiftKey: true }));
        }
        await copySelection('copy');
        // after shift-extend caret = FOCUS (23) — ArrowRight ×4 lands before "chào" (DD §5.4c)
        for (let i = 0; i < 4; i++) {
          window.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight' }));
        }
        await pasteFromClipboard();
        window.dispatchEvent(new KeyboardEvent('keydown', { key: 'a', metaKey: true }));
        await copySelection('copy-all');
        window.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight' }));
        // T-A30 (§5.5h): dblclick cell B word → T30B · triple-click paragraph → T30P · cell A type+enter → T30C/T30C2
        const cellB = model.lines.find((l) => l.cell && l.cell[1] === 1 && l.cellPara === 0);
        const betaWord = cellB?.items.find((it) => it.text && !it.text.includes(' '));
        if (cellB && betaWord) {
          mouseDownAt(betaWord.x + betaWord.w / 2, cellB.baseline, false, 2);
          await edit({ kind: 'insert', text: 'T30B', caret, sel });
        }
        const topLines = model.lines.filter((l) => !l.cell);
        const lastTop = topLines[topLines.length - 1];
        if (lastTop) {
          mouseDownAt(lastTop.x + lastTop.total / 2, lastTop.baseline, false, 3);
          await edit({ kind: 'insert', text: 'T30P', caret, sel });
        }
        const cellA = model.lines.find((l) => l.cell && l.cell[1] === 0 && l.cellPara === 0);
        if (cellA) {
          mouseDownAt(cellA.x + cellA.total, cellA.baseline, false, 1);
          await edit({ kind: 'insert', text: 'T30C', caret, sel: null });
          await edit({ kind: 'enter', caret, sel: null });
          await edit({ kind: 'insert', text: 'T30C2', caret, sel: null });
        }
        // T-A31: type INSIDE the link text — the group must survive save (rel + target + runs)
        const linkLine = model.lines.find((l) => l.items.some((it) => it.text.includes('fidelity')));
        const linkItem = linkLine?.items.find((it) => it.text.includes('fidelity'));
        if (linkLine && linkItem) {
          const h = hitAt(linkItem.x + linkItem.w / 2, linkLine.baseline);
          if (h) await edit({ kind: 'insert', text: 'T31LINK', caret: h, sel: null });
        }
        await save();
        await invoke('wasen_light_reopen');
        await invoke('frontend_log', { level: 'info', message: 'nano light probe: chain done (edit → save → reopen)' });
      } catch (e) {
        await invoke('frontend_log', { level: 'error', message: `nano light failed: probe ${e}` });
      }
    }
  } catch (e) {
    surface?.showFatal(`nano light failed: ${e}`);
    await invoke('frontend_log', { level: 'error', message: `nano light failed: ${e}` });
  }
}

void main();
