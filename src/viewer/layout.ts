/**
 * Viewer layout core (T-V7, DD V01 §3.4/§5.7) — PURE TypeScript: no DOM, no
 * canvas. Input: the resolved blocks + the OS/2 font metrics + a text
 * measurer; output: the LineModel the viewer draws, plus `hitTest`, the
 * INVERSE mapping (x, y) → (paragraph, line, run, offset) A4 caret work
 * builds on. The wrap/tab/line-height/media logic is ported VERBATIM from
 * the former interleaved draw code (T-V2/V3 viewer.ts) — this layout is the
 * parity asset (OQ-V01-1, ε=2.0px): the port must not drift, and editor work
 * (T-A27) must CONSUME this module, never fork it.
 */

export interface ImageRef {
  rel_id: string;
  part: string; // OPC part name, e.g. word/media/image1.png
  cx_emu: number;
  cy_emu: number;
}

export interface Run {
  text: string;
  font: string | null;
  size: number | null; // half-points; null = nothing declared anywhere
  bold: boolean;
  italic: boolean;
  underline: string | null;
  color: string | null; // hex RGB; null = default ink
  spacing: number | null; // twips
  images: ImageRef[];
}

export interface Ind {
  left: number | null;
  right: number | null;
  firstLine: number | null;
  hanging: number | null;
}

export interface Paragraph {
  align: string | null;
  ind: Ind | null;
  runs: Run[];
}

export interface TableCell {
  blocks: Block[];
}

export interface Table {
  rows: TableCell[][];
}

export type Block = { Paragraph: Paragraph } | { Table: Table };

/** family → [ascent, descent, lineGap], OS/2 scaled 1000/em (C4-a). */
export type FontMetrics = Record<string, [number, number, number]>;

/** Text measurement, injected so this module stays DOM-free. The viewer
 * binds it to canvas `measureText`; the unit tests use an exact fake. */
export type Measurer = (r: Run, text: string) => number;

export const PT_PX = 96 / 72;
/** 1in page margin — pgMar is not in the model yet (DD V01 §3.2 has pgSz only). */
export const MARGIN = 96;
// w:defaultTabStop (§17.3.1.15) lives in settings.xml, which the model does
// not parse; 720 twips (0.5in) is the Word convention, not a spec default.
export const TAB_STOP_TW = 720;
// ponytail: flat 1.15× for families with no bundled OS/2 metrics — V4 parity
// measures the error before any smarter fallback
export const FALLBACK_SPACING = 1.15;
export const DEFAULT_SIZE = 22; // half-points = 11pt — terminal fallback of the chain
// wp:extent (§20.4.2.7) is in EMUs: 914400/inch (open-source cross-check:
// Apache POI Units.java, Apache-2.0 — EMU_PER_INCH=914400, EMU_PER_PIXEL=9525)
// at the same 96-dpi CSS px the rest of the viewer uses.
export const EMU_PX = 9525;

export const twip = (t: number): number => (t / 20) * PT_PX;
const imgPx = (emu: number): number => emu / EMU_PX;

export function runFont(r: Run): string {
  const size = r.size ?? DEFAULT_SIZE;
  const px = Math.round((size / 2) * PT_PX * 10) / 10;
  const italic = r.italic ? 'italic ' : '';
  const bold = r.bold ? 'bold ' : '';
  return `${italic}${bold}${px}px ${r.font ?? 'sans-serif'}`;
}

/** Line height in px: ascent + |descent| + lineGap from OS/2 (C4-a). */
export function lineHeight(r: Run, fonts: FontMetrics): number {
  const px = ((r.size ?? DEFAULT_SIZE) / 2) * PT_PX;
  const m = r.font ? fonts[r.font] : undefined;
  if (!m) return px * FALLBACK_SPACING;
  return ((m[0] - m[1] + m[2]) / 1000) * px;
}

/** One laid-out item (a word, a space, a tab jump or an image) with its
 * absolute position. `ri`/`charStart` locate it inside the paragraph: the
 * item's text starts at `charStart` of run `ri`'s text (tabs/breaks are the
 * control chars in the run text — T-V1). */
export interface LayoutItem {
  r: Run;
  ri: number;
  text: string; // '' for tab/image items
  x: number;
  w: number;
  charStart: number;
  tab?: boolean;
  img?: ImageRef;
  h?: number;
}

/** One visual TEXT line (image lines surface in `media`, not here — the
 * gate's uniform-height asserts stay text-only, as since T-V2). */
export interface TextLine {
  items: LayoutItem[];
  para: number; // body block index — `pi` in the evidence JSON
  /** T-060 C3 (additive): the TOP-LEVEL body block this line belongs to (for
   * a table's cell lines: the table block's own index). Unlike `para` (the
   * `pi` walk counter, which cell content shares with the block after the
   * table), `block` is collision-free across tables. */
  block: number;
  /** T-060 C3 (additive): `[row, col]` when the line sits inside a table cell
   * (only `rows[0]` is laid out — V3 ceiling), else null. The edit layer
   * refuses in-cell carets on this identity, not on heuristics. */
  cell: [number, number] | null;
  /** T-A30 §5.5a (additive): index of the paragraph inside the cell's blocks;
   * null at top level AND for nested-table content (unaddressable, refused). */
  cellPara: number | null;
  x: number; // aligned line start
  baseline: number;
  lh: number;
  total: number; // sum of item widths
  align: string | null;
  inCell: boolean;
}

export interface LayoutMedia {
  part: string;
  emu: [number, number];
  w: number;
  h: number;
  x: number;
  y: number;
}

export interface LineModel {
  lines: TextLine[]; // text lines in draw order
  media: LayoutMedia[];
  maxw: number; // widest TEXT line
  metrics: [number, number, number] | null; // first line font with OS/2 metrics
  /** Bottom of everything laid out (text baselines + image rects) — the
   * canvas height the scroll view needs. */
  bottom: number;
}

export interface LayoutInput {
  blocks: Block[];
  x0: number; // text column left
  w: number; // text column width
  y: number; // first baseline
  fonts: FontMetrics;
  measure: Measurer;
}

type Tok =
  | { kind: 'word' | 'space'; r: Run; ri: number; text: string; start: number }
  | { kind: 'tab'; r: Run; ri: number; start: number }
  | { kind: 'break'; r: Run; ri: number; start: number }
  | { kind: 'image'; r: Run; ri: number; img: ImageRef; start: number };

/** Runs → tokens: words/spaces/tabs, hard breaks from `w:br`, then images.
 * The parser flattens intra-run child order, so a run's images render after
 * its text — recorded as a V3 ceiling (ponytail) in DD V01. T-V7: each token
 * also carries its offset inside the run text (hit-test inverse mapping). */
function tokenize(runs: Run[]): Tok[] {
  const toks: Tok[] = [];
  runs.forEach((r, ri) => {
    let pos = 0;
    r.text.split(/[\n\f\v]/).forEach((seg, i) => {
      if (i > 0) {
        toks.push({ kind: 'break', r, ri, start: pos });
        pos += 1;
      }
      seg.split('\t').forEach((piece, j) => {
        if (j > 0) {
          toks.push({ kind: 'tab', r, ri, start: pos });
          pos += 1;
        }
        if (piece) {
          for (const word of piece.split(/( +)/)) {
            if (word) {
              toks.push({ kind: / /.test(word) ? 'space' : 'word', r, ri, text: word, start: pos });
              pos += word.length;
            }
          }
        }
      });
    });
    for (const img of r.images) toks.push({ kind: 'image', r, ri, img, start: pos });
  });
  return toks;
}

/** Greedy-wrap one paragraph into visual lines (each line = laid-out items,
 * x assigned by the caller after alignment). Line items may carry an image:
 * `w`/`h` are the wp:extent size in px. */
function wrapParagraph(
  p: Paragraph,
  xLeft: number,
  xRight: number,
  measure: Measurer,
): LayoutItem[][] {
  const lines: LayoutItem[][] = [[]];
  let x = xLeft;
  const breakLine = () => {
    lines.push([]);
    x = xLeft;
  };
  const toks = tokenize(p.runs);
  for (let i = 0; i < toks.length; i++) {
    const tok = toks[i];
    if (tok.kind === 'break') {
      breakLine();
    } else if (tok.kind === 'image') {
      // V3 ceiling (ponytail): an image takes its OWN line — inline wrap
      // around a drawing is not modelled yet (DD V01 §4/V3). The break after
      // it happens only when more tokens follow — no phantom empty line.
      if (lines[lines.length - 1].length > 0) breakLine();
      lines[lines.length - 1].push({
        r: tok.r,
        ri: tok.ri,
        text: '',
        w: imgPx(tok.img.cx_emu),
        h: imgPx(tok.img.cy_emu),
        img: tok.img,
        charStart: tok.start,
        x: 0,
      });
      if (i + 1 < toks.length) breakLine();
    } else if (tok.kind === 'tab') {
      const step = twip(TAB_STOP_TW);
      let adv = step - ((x - xLeft) % step);
      if (x + adv > xRight && x > xLeft) {
        breakLine();
        adv = step; // tab on a full line: one step into the fresh line
      }
      lines[lines.length - 1].push({
        r: tok.r,
        ri: tok.ri,
        text: '',
        w: adv,
        tab: true,
        charStart: tok.start,
        x: 0,
      });
      x += adv;
    } else {
      const w = measure(tok.r, tok.text);
      if (tok.kind === 'space') {
        if (lines[lines.length - 1].length === 0) continue; // no leading space after a wrap
        if (x + w > xRight) continue; // trailing space never forces a wrap
      } else if (x + w > xRight && lines[lines.length - 1].length > 0) {
        breakLine();
      }
      lines[lines.length - 1].push({ r: tok.r, ri: tok.ri, text: tok.text, w, charStart: tok.start, x: 0 });
      x += w;
    }
  }
  return lines;
}

/** Line start x for the paragraph alignment against a content width. */
function alignX(p: Paragraph, xLeft: number, colW: number, contentW: number): number {
  return p.align === 'center'
    ? xLeft + (colW - contentW) / 2
    : p.align === 'right'
      ? xLeft + colW - contentW
      : xLeft;
}

/** Lay out `blocks` into the LineModel. Ported verbatim from the former
 * drawParagraph/drawBlocks layout halves (the `pi` block counting, the
 * first-table-row-only walk and the +4 row gap included — ceilings kept). */
export function layoutBlocks(input: LayoutInput): LineModel {
  const { blocks, fonts, measure } = input;
  const lines: TextLine[] = [];
  const media: LayoutMedia[] = [];
  let maxw = 0;
  let metrics: [number, number, number] | null = null;

  // One paragraph at baseline y; returns the y advance.
  const layoutParagraph = (
    p: Paragraph,
    x0: number,
    w: number,
    y: number,
    pi: number,
    inCell: boolean,
    block: number,
    cell: [number, number] | null,
    cellPara: number | null,
  ): number => {
    const xLeft = x0 + ((p.ind?.left ?? 0) / 20) * PT_PX;
    const wrapped = wrapParagraph(p, xLeft, x0 + w, measure);
    let yCur = y;
    for (const line of wrapped) {
      const imgItem = line.find((it) => it.img);
      if (imgItem?.img && imgItem.h != null) {
        // image line: the image's own bottom is its baseline; the line
        // advances by the image height
        const x = alignX(p, xLeft, w, imgItem.w);
        media.push({
          part: imgItem.img.part,
          emu: [imgItem.img.cx_emu, imgItem.img.cy_emu],
          w: imgItem.w,
          h: imgItem.h,
          x,
          y: yCur,
        });
        yCur += imgItem.h;
        continue;
      }
      const lh = line.length
        ? Math.max(...line.map((it) => lineHeight(it.r, fonts)))
        : lineHeight({ size: null, font: null } as Run, fonts); // empty paragraph (hard break) — fallback spacing
      const total = line.reduce((a, b) => a + b.w, 0);
      maxw = Math.max(maxw, total);
      if (!metrics) {
        const withFont = line.find((it) => it.r.font && fonts[it.r.font]);
        if (withFont) {
          metrics = fonts[withFont.r.font!];
        }
      }
      const x = alignX(p, xLeft, w, total);
      let ix = x;
      for (const it of line) {
        it.x = ix;
        ix += it.w;
      }
      lines.push({ items: line, para: pi, block, cell, cellPara, x, baseline: yCur, lh, total, align: p.align, inCell });
      yCur += lh;
    }
    return yCur - y;
  };

  // Blocks starting at baseline y; x0/w is the text column. Returns the bottom y.
  // T-060 C3: `tableBlock`/`cell` carry the top-level block identity down the
  // recursion — a top-level paragraph owns its own index; everything inside a
  // table (nested tables included) belongs to that table's block + cell.
  // T-A30 §5.5a: `cellBlocks` = a first-level cell's blocks (cellPara = block index);
  // nested-table content walks with false → cellPara null → edit-refused.
  const walk = (
    bs: Block[],
    x0: number,
    w: number,
    y: number,
    piBase: number,
    inCell: boolean,
    tableBlock: number | null,
    cell: [number, number] | null,
    cellBlocks: boolean,
  ): number => {
    let pi = piBase;
    let yCur = y;
    for (let i = 0; i < bs.length; i++) {
      const block = bs[i];
      if ('Paragraph' in block) {
        yCur += layoutParagraph(block.Paragraph, x0, w, yCur, pi, inCell, tableBlock ?? i, cell, cellBlocks ? i : null);
        pi += 1;
      } else {
        // table: each cell's blocks at its column offset; the row advances by
        // the tallest cell (tblGrid widths land with the V3 media slice — the
        // resolved model does not carry the grid yet)
        const row = block.Table.rows[0];
        pi += 1;
        if (!row) continue;
        const tbl = tableBlock ?? i;
        const colW = w / row.length;
        let rowBottom = yCur;
        row.forEach((tc, col) => {
          // only rows[0] is walked (V3 ceiling) → row 0 in the cell identity
          rowBottom = Math.max(rowBottom, walk(tc.blocks, x0 + col * colW, colW, yCur, pi, true, tbl, [0, col], !inCell));
        });
        yCur = rowBottom + 4;
      }
    }
    return yCur;
  };

  walk(blocks, input.x0, input.w, input.y, 0, false, null, null, false);

  let bottom = 0;
  lines.forEach((l) => {
    bottom = Math.max(bottom, l.baseline + l.lh);
  });
  for (const m of media) bottom = Math.max(bottom, m.y + m.h);

  return { lines, media, maxw, metrics, bottom };
}

/** The hit-test result: `caret` is the editor-coordinate caret — `para` is
 * the TOP-LEVEL body block (tables own their cells' lines — the walk `pi`
 * cannot address the body and is deliberately NOT exposed here); `cell` is
 * the `[row, col]` identity for in-cell lines, null for top-level ones —
 * the edit layer refuses on `cell !== null`; `cellPara` locates the
 * paragraph inside the cell's blocks (null at top level / nested —
 * refused). `caret` feeds `caretXY` unchanged (round-trip without glue —
 * review #6 C1); `line`/`run`/`offset` stay top-level for the probe's
 * per-line checks. Image/tab items resolve to their run-text position. */
export interface Hit {
  caret: {
    para: number;
    run: number;
    offset: number;
    cell: [number, number] | null;
    cellPara: number | null;
  };
  line: number;
  run: number;
  offset: number;
}

const itemCharLen = (it: LayoutItem): number => (it.tab ? 1 : it.text.length);

/** Inverse mapping (x, y) → caret. y resolves to the line with the NEAREST
 * baseline (boundaries at midpoints; above the first / below the last line
 * clamps); T-A30 §5.5d: exact baseline ties (cells of one row) break by x-span —
 * the line whose span holds x wins (table-free layout never collides). x
 * clamps to the line's first/last caret position; inside the line it walks
 * the item whose span contains x and binary-searches the char boundaries
 * with the same measurer the forward layout used: x within char i's advance
 * → the caret sits BEFORE char i (no half-char centering — the measured
 * advances are the only truth; A4 can layer centering on this same model). */
export function hitTest(model: LineModel, measure: Measurer, x: number, y: number): Hit | null {
  const ls = model.lines;
  if (ls.length === 0) return null;
  const inSpan = (l: TextLine): boolean => x >= l.x && x <= l.x + l.total;
  let k = 0;
  for (let i = 1; i < ls.length; i++) {
    const di = Math.abs(y - ls[i].baseline);
    const dk = Math.abs(y - ls[k].baseline);
    if (di < dk || (di === dk && inSpan(ls[i]) && !inSpan(ls[k]))) k = i;
  }
  const line = ls[k];
  const hit = (run: number, offset: number): Hit => ({
    caret: { para: line.block, run, offset, cell: line.cell, cellPara: line.cellPara },
    line: k,
    run,
    offset,
  });
  if (line.items.length === 0) return hit(0, 0); // empty line (hard break): caret at paragraph start
  const first = line.items[0];
  const last = line.items[line.items.length - 1];
  if (x <= first.x) return hit(first.ri, first.charStart);
  if (x >= last.x + last.w) return hit(last.ri, last.charStart + itemCharLen(last));
  for (const it of line.items) {
    if (x < it.x + it.w) {
      if (it.tab) {
        // a tab occupies one control char; its width splits before/after
        return hit(it.ri, it.charStart + (x - it.x < it.w / 2 ? 0 : 1));
      }
      const local = x - it.x;
      let lo = 0;
      let hi = it.text.length;
      while (lo < hi) {
        const mid = (lo + hi + 1) >> 1;
        if (measure(it.r, it.text.slice(0, mid)) <= local) lo = mid;
        else hi = mid - 1;
      }
      return hit(it.ri, it.charStart + lo);
    }
  }
  return hit(last.ri, last.charStart + itemCharLen(last)); // float safety
}

/**
 * Forward map (para, run, offset) → (x, baseline, line-height, line index) —
 * the mirror of `hitTest` on the SAME model + measurer: the item whose
 * run/charStart contains the offset (caret BEFORE char `offset`); end-of-run
 * clamps to the LAST item of that run (visual end of a wrapped run); an
 * empty paragraph resolves to its line start (hitTest's placeholder
 * semantics). T-060 C3: moved here from src/editor.ts so the hit seam
 * (hitTest ↔ caretXY) lives in one tested module — gate 14 asserts their
 * round-trip (review #6 C1: `hitTest(...).caret` goes in unmodified).
 * `para` matches the BODY BLOCK index (the editor `Caret.para` coordinate);
 * `line` rides along (additive) for the editor's Up/Down/Home/End moves.
 * T-A30 §5.5a: the cell identity is REQUIRED (explicit nulls at top level) —
 * the match is one code path; block-only carets at a TABLE block resolve to
 * nothing (cell lines carry identity; such carets are refused upstream). */
export function caretXY(
  model: LineModel,
  measure: Measurer,
  caret: {
    para: number;
    run: number;
    offset: number;
    cell: [number, number] | null;
    cellPara: number | null;
  },
): { x: number; y: number; lh: number; line: number } | null {
  const cellEq = (
    a: [number, number] | null,
    b: [number, number] | null,
  ): boolean => (a === null || b === null ? a === b : a[0] === b[0] && a[1] === b[1]);
  let fallback: { x: number; y: number; lh: number; line: number } | null = null;
  for (const [li, line] of model.lines.entries()) {
    if (line.block !== caret.para) continue;
    if ((line.cellPara ?? null) !== caret.cellPara || !cellEq(line.cell, caret.cell)) continue;
    if (line.items.length === 0) {
      fallback ??= { x: line.x, y: line.baseline, lh: line.lh, line: li };
      continue;
    }
    for (const it of line.items) {
      if (it.ri !== caret.run) continue;
      const end = it.charStart + itemCharLen(it);
      if (caret.offset >= it.charStart && caret.offset < end) {
        const x = it.tab ? it.x : it.x + measure(it.r, it.text.slice(0, caret.offset - it.charStart));
        return { x, y: line.baseline, lh: line.lh, line: li };
      }
      if (caret.offset === end) {
        fallback = { x: it.x + it.w, y: line.baseline, lh: line.lh, line: li };
      }
    }
  }
  return fallback;
}
