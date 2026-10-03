/**
 * V01 quick-view slice (DD docs/design/viewer-a3/DD.md §3.4), V1: the crate
 * resolves every style chain (§17.7.2 hierarchy) — `viewer_open` returns the
 * RENDER model, this page merges nothing. One visual line per paragraph
 * still (wrapping is V2); break controls inside run text (`\n` line, `\f`
 * page, `\v` column — all from `w:br`, `\t` from `w:tab`) advance the
 * baseline. No engine shim is loaded on this page (see viewer.html).
 */

interface Run {
  text: string;
  font: string | null;
  size: number | null; // half-points; null = nothing declared anywhere
  bold: boolean;
  italic: boolean;
  underline: string | null;
  color: string | null; // hex RGB; null = default ink
  spacing: number | null; // twips
}

interface Ind {
  left: number | null;
  right: number | null;
  firstLine: number | null;
  hanging: number | null;
}

interface Paragraph {
  align: string | null;
  ind: Ind | null;
  runs: Run[];
}

interface TableCell {
  blocks: Block[];
}

interface Table {
  rows: TableCell[][];
}

type Block = { Paragraph: Paragraph } | { Table: Table };

interface QuickView {
  parse_ms: number;
  doc: { body: Block[] };
}

const { invoke } = (window as unknown as { __TAURI__: { core: { invoke: <T>(cmd: string, args?: unknown) => Promise<T> } } }).__TAURI__.core;

const PT_PX = 96 / 72;
const MARGIN = 96; // 1in — V2 does pgSz properly
// ponytail: flat 1.15× until the C4-a OS/2 line-metrics accessor lands (V2)
const LINE_SPACING = 1.15;
const DEFAULT_SIZE = 22; // half-points = 11pt — terminal fallback of the chain

function runFont(r: Run): string {
  const size = r.size ?? DEFAULT_SIZE;
  const px = Math.round((size / 2) * PT_PX * 10) / 10;
  const italic = r.italic ? 'italic ' : '';
  const bold = r.bold ? 'bold ' : '';
  return `${italic}${bold}${px}px ${r.font ?? 'sans-serif'}`;
}

/** One paragraph at the current baseline; returns the y advance. */
function drawParagraph(ctx: CanvasRenderingContext2D, p: Paragraph, x0: number, w: number, y: number): number {
  const xLeft = x0 + ((p.ind?.left ?? 0) / 20) * PT_PX;
  const width = x0 + w - xLeft;
  // flatten runs into visual lines at the break controls (`\t` is a flat
  // 4-space advance until V2 does tab stops)
  const lines: Array<Array<{ r: Run; text: string }>> = [[]];
  for (const r of p.runs) {
    r.text.split(/\n|\f|\v/).forEach((seg, i) => {
      if (i > 0) lines.push([]);
      lines[lines.length - 1].push({ r, text: seg.replace(/\t/g, '    ') });
    });
  }
  let yCur = y;
  for (const line of lines) {
    const widths = line.map(({ r, text }) => {
      ctx.font = runFont(r);
      return ctx.measureText(text).width;
    });
    const total = widths.reduce((a, b) => a + b, 0);
    const align = p.align;
    let x = align === 'center' ? xLeft + (width - total) / 2 : align === 'right' ? xLeft + width - total : xLeft;
    const size = line[0]?.r.size ?? DEFAULT_SIZE;
    line.forEach(({ r, text }, i) => {
      ctx.font = runFont(r);
      ctx.fillStyle = r.color ? `#${r.color}` : '#000';
      if (r.spacing != null) ctx.letterSpacing = `${(r.spacing / 20) * PT_PX}px`;
      ctx.fillText(text, x, yCur);
      if (r.spacing != null) ctx.letterSpacing = '0px';
      if (r.underline) ctx.fillRect(x, yCur + 2, widths[i], 1);
      x += widths[i];
    });
    yCur += (size / 2) * PT_PX * LINE_SPACING;
  }
  return yCur - y;
}

/** Blocks starting at baseline y; x0/w is the text column. Returns the bottom y. */
function drawBlocks(ctx: CanvasRenderingContext2D, blocks: Block[], x0: number, w: number, y: number): number {
  for (const block of blocks) {
    if ('Paragraph' in block) {
      y += drawParagraph(ctx, block.Paragraph, x0, w, y);
    } else {
      // table: each cell's blocks at its column offset; the row advances by
      // the tallest cell (V2 replaces this with tblGrid widths)
      const row = block.Table.rows[0];
      if (!row) continue;
      const colW = w / row.length;
      let rowBottom = y;
      row.forEach((cell, col) => {
        rowBottom = Math.max(rowBottom, drawBlocks(ctx, cell.blocks, x0 + col * colW, colW, y));
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
    drawBlocks(ctx, qv.doc.body, MARGIN, innerWidth - 2 * MARGIN, MARGIN + 16);
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
