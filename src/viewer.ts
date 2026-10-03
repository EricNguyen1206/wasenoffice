/**
 * V01 quick-view slice (DD docs/design/viewer-a3/DD.md §3.4): parse-only via
 * the A2 crate (`viewer_open` command), draw body text on canvas 2D. No
 * layout engine — one visual line per paragraph, props resolved here ad hoc;
 * V1 moves resolve to Rust and V2 adds real line breaking. No engine shim is
 * loaded on this page (see viewer.html).
 */

interface Run {
  text: string;
  bold: boolean;
  italic: boolean;
  underline: string | null;
  size: number | null; // half-points
}

interface Paragraph {
  style: string | null;
  align: string | null;
  runs: Run[];
}

interface TableCell {
  blocks: Block[];
}

interface Table {
  rows: TableCell[][];
}

type Block = { Paragraph: Paragraph } | { Table: Table };

interface StyleDef {
  kind: string;
  align: string | null;
  bold: boolean | null;
  italic: boolean | null;
  underline: string | null;
  size: number | null;
}

interface QuickView {
  parse_ms: number;
  doc: { body: Block[]; styles: Record<string, StyleDef> };
}

const { invoke } = (window as unknown as { __TAURI__: { core: { invoke: <T>(cmd: string, args?: unknown) => Promise<T> } } }).__TAURI__.core;

const PT_PX = 96 / 72;
const MARGIN = 96; // 1in — V2 does pgSz properly
// ponytail: flat 1.15× until the C4-a OS/2 line-metrics accessor lands (V2)
const LINE_SPACING = 1.15;
const DEFAULT_SIZE = 22; // half-points = 11pt

function styleOf(doc: QuickView['doc'], p: Paragraph): StyleDef | undefined {
  return p.style ? doc.styles[p.style] : undefined;
}

function runFont(r: Run, s: StyleDef | undefined): string {
  const size = r.size ?? s?.size ?? DEFAULT_SIZE;
  const px = Math.round((size / 2) * PT_PX * 10) / 10;
  const italic = r.italic || s?.italic === true ? 'italic ' : '';
  const bold = r.bold || s?.bold === true ? 'bold ' : '';
  return `${italic}${bold}${px}px sans-serif`; // V1: w:rFonts → family
}

/** One paragraph at the current baseline; returns the y advance. */
function drawParagraph(ctx: CanvasRenderingContext2D, doc: QuickView['doc'], p: Paragraph, x0: number, w: number, y: number): number {
  const s = styleOf(doc, p);
  const runs = p.runs.length > 0 ? p.runs : [{ text: '', bold: false, italic: false, underline: null, size: null }];
  const widths = runs.map((r) => {
    ctx.font = runFont(r, s);
    return ctx.measureText(r.text).width;
  });
  const total = widths.reduce((a, b) => a + b, 0);
  let x = p.align === 'center' ? x0 + (w - total) / 2 : p.align === 'right' ? x0 + w - total : x0;
  const size = runs[0].size ?? s?.size ?? DEFAULT_SIZE;
  runs.forEach((r, i) => {
    ctx.font = runFont(r, s);
    ctx.fillText(r.text, x, y);
    if (r.underline || s?.underline) {
      ctx.fillRect(x, y + 2, widths[i], 1);
    }
    x += widths[i];
  });
  return (size / 2) * PT_PX * LINE_SPACING;
}

/** Blocks starting at baseline y; x0/w is the text column. Returns the bottom y. */
function drawBlocks(ctx: CanvasRenderingContext2D, doc: QuickView['doc'], blocks: Block[], x0: number, w: number, y: number): number {
  for (const block of blocks) {
    if ('Paragraph' in block) {
      y += drawParagraph(ctx, doc, block.Paragraph, x0, w, y);
    } else {
      // table: each cell's blocks at its column offset; the row advances by
      // the tallest cell (V2 replaces this with tblGrid widths)
      const row = block.Table.rows[0];
      if (!row) continue;
      const colW = w / row.length;
      let rowBottom = y;
      row.forEach((cell, col) => {
        rowBottom = Math.max(rowBottom, drawBlocks(ctx, doc, cell.blocks, x0 + col * colW, colW, y));
      });
      y = rowBottom + 4;
    }
  }
  return y;
}

async function main(): Promise<void> {
  const t0 = performance.now();
  const cv = document.getElementById('cv') as HTMLCanvasElement;
  const dpr = window.devicePixelRatio || 1;
  cv.width = innerWidth * dpr;
  cv.height = innerHeight * dpr;
  cv.style.width = `${innerWidth}px`;
  cv.style.height = `${innerHeight}px`;
  const ctx = cv.getContext('2d')!;
  ctx.scale(dpr, dpr);
  ctx.fillStyle = '#000';

  try {
    const qv = await invoke<QuickView>('viewer_open');
    drawBlocks(ctx, qv.doc, qv.doc.body, MARGIN, innerWidth - 2 * MARGIN, MARGIN + 16);
    const ms = performance.now() - t0;
    // render evidence: count non-white pixels — text actually hit the canvas
    const img = ctx.getImageData(0, 0, cv.width, cv.height).data;
    let inked = 0;
    for (let i = 0; i < img.length; i += 4 * 97) {
      if (img[i] < 128) inked++; // red channel of every 97th pixel
    }
    const msg = `viewer first-draw ${ms.toFixed(1)} ms (parse ${qv.parse_ms.toFixed(1)} ms, body blocks ${qv.doc.body.length}, inked ${inked})`;
    await invoke('frontend_log', { level: 'info', message: msg });
  } catch (e) {
    ctx.fillText(`quick view failed: ${e}`, MARGIN, MARGIN);
    await invoke('frontend_log', { level: 'error', message: `viewer failed: ${e}` });
  }
}

void main();
