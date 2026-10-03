/**
 * V01 quick-view slice (DD docs/design/viewer-a3/DD.md §3.4), V3: minimal
 * layout — greedy line breaking inside the text column derived from `pgSz`,
 * default tab stops, and line height from the OS/2 metrics the shell resolves
 * (C4-a, `allfonts::line_metrics` — bundled set). Advances still come from
 * canvas `measureText` (C4-b: the platform text stack; hmtx stays out). The
 * crate resolves every style chain (§17.7.2) — this page merges nothing.
 * Break controls inside run text (`\n` line, `\f` page, `\v` column — all
 * from `w:br`, `\t` from `w:tab`) force a new visual line / tab jump.
 * V3 media: images from the run's `ImageRef`s draw on their OWN line at the
 * `wp:extent` size (EMU → px), fetched by fileId from the loopback filesrv
 * (`/media/<id>/<part>`). No engine shim is loaded on this page (see
 * viewer.html).
 */

interface ImageRef {
  rel_id: string;
  part: string; // OPC part name, e.g. word/media/image1.png
  cx_emu: number;
  cy_emu: number;
}

interface Run {
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
  /** family → [ascent, descent, lineGap], OS/2 scaled 1000/em (C4-a). */
  fonts: Record<string, [number, number, number]>;
  /** `w:pgSz` of the final section, twips (§17.6.11). */
  page: { w: number | null; h: number | null; orient: string | null } | null;
  /** V3: `<origin>/media/<fileId>` prefix — append the OPC part name to
   * fetch image bytes from the loopback filesrv; null = media disabled. */
  media_base: string | null;
}

const { invoke } = (window as unknown as { __TAURI__: { core: { invoke: <T>(cmd: string, args?: unknown) => Promise<T> } } }).__TAURI__.core;

const PT_PX = 96 / 72;
const MARGIN = 96; // 1in — pgMar is not in the model yet (DD V01 §3.2 has pgSz only)
// w:defaultTabStop (§17.3.1.15) lives in settings.xml, which the model does
// not parse; 720 twips (0.5in) is the Word convention, not a spec default.
const TAB_STOP_TW = 720;
// ponytail: flat 1.15× for families with no bundled OS/2 metrics — V4 parity
// measures the error before any smarter fallback
const FALLBACK_SPACING = 1.15;
const DEFAULT_SIZE = 22; // half-points = 11pt — terminal fallback of the chain
// wp:extent (§20.4.2.7) is in EMUs: 914400/inch (open-source cross-check:
// Apache POI Units.java, Apache-2.0 — EMU_PER_INCH=914400, EMU_PER_PIXEL=9525)
// at the same 96-dpi CSS px the rest of this page uses.
const EMU_PX = 9525;

const twip = (t: number): number => (t / 20) * PT_PX;
const imgPx = (emu: number): number => emu / EMU_PX;

/** Line height in px: ascent + |descent| + lineGap from OS/2 (C4-a). */
function lineHeight(r: Run, fonts: QuickView['fonts']): number {
  const px = ((r.size ?? DEFAULT_SIZE) / 2) * PT_PX;
  const m = r.font ? fonts[r.font] : undefined;
  if (!m) return px * FALLBACK_SPACING;
  return ((m[0] - m[1] + m[2]) / 1000) * px;
}

function runFont(r: Run): string {
  const size = r.size ?? DEFAULT_SIZE;
  const px = Math.round((size / 2) * PT_PX * 10) / 10;
  const italic = r.italic ? 'italic ' : '';
  const bold = r.bold ? 'bold ' : '';
  return `${italic}${bold}${px}px ${r.font ?? 'sans-serif'}`;
}

/** Layout evidence the gate asserts on (y advances = line heights). */
interface Stats {
  baselines: number[];
  lhs: number[];
  maxw: number;
  metrics: [number, number, number] | null;
  /** V3: every drawn image — the gate recomputes EMU→px from `emu` and
   * asserts the drawn rect size, flow position and inked-pixel count. */
  media: Array<{ part: string; emu: [number, number]; w: number; h: number; x: number; y: number }>;
}

type Tok =
  | { kind: 'word' | 'space'; r: Run; text: string }
  | { kind: 'tab'; r: Run }
  | { kind: 'break'; r: Run }
  | { kind: 'image'; r: Run; img: ImageRef };

/** Runs → tokens: words/spaces/tabs, hard breaks from `w:br`, then images.
 * The parser flattens intra-run child order, so a run's images render after
 * its text — recorded as a V3 ceiling (ponytail) in DD V01. */
function tokenize(runs: Run[]): Tok[] {
  const toks: Tok[] = [];
  for (const r of runs) {
    r.text.split(/[\n\f\v]/).forEach((seg, i) => {
      if (i > 0) toks.push({ kind: 'break', r });
      seg.split('\t').forEach((piece, j) => {
        if (j > 0) toks.push({ kind: 'tab', r });
        if (piece) {
          for (const word of piece.split(/( +)/)) {
            if (word) toks.push({ kind: / /.test(word) ? 'space' : 'word', r, text: word });
          }
        }
      });
    });
    for (const img of r.images) toks.push({ kind: 'image', r, img });
  }
  return toks;
}

/** Greedy-wrap one paragraph into visual lines (each line = measured items).
 * Line items may carry an image: `w`/`h` are the wp:extent size in px. */
function wrapParagraph(
  p: Paragraph,
  xLeft: number,
  xRight: number,
  ctx: CanvasRenderingContext2D,
): Array<Array<{ r: Run; text: string; w: number; img?: ImageRef; h?: number }>> {
  const measure = (r: Run, text: string): number => {
    ctx.font = runFont(r);
    return ctx.measureText(text).width;
  };
  const lines: Array<Array<{ r: Run; text: string; w: number; img?: ImageRef; h?: number }>> = [[]];
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
        text: '',
        w: imgPx(tok.img.cx_emu),
        h: imgPx(tok.img.cy_emu),
        img: tok.img,
      });
      if (i + 1 < toks.length) breakLine();
    } else if (tok.kind === 'tab') {
      const step = twip(TAB_STOP_TW);
      let adv = step - ((x - xLeft) % step);
      if (x + adv > xRight && x > xLeft) {
        breakLine();
        adv = step; // tab on a full line: one step into the fresh line
      }
      lines[lines.length - 1].push({ r: tok.r, text: '', w: adv });
      x += adv;
    } else {
      const w = measure(tok.r, tok.text);
      if (tok.kind === 'space') {
        if (lines[lines.length - 1].length === 0) continue; // no leading space after a wrap
        if (x + w > xRight) continue; // trailing space never forces a wrap
      } else if (x + w > xRight && lines[lines.length - 1].length > 0) {
        breakLine();
      }
      lines[lines.length - 1].push({ r: tok.r, text: tok.text, w });
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

/** Lay out + draw one paragraph at the current baseline; returns the y advance. */
function drawParagraph(
  ctx: CanvasRenderingContext2D,
  p: Paragraph,
  x0: number,
  w: number,
  y: number,
  fonts: QuickView['fonts'],
  bitmaps: Map<string, ImageBitmap>,
  stats: Stats,
): number {
  const xLeft = x0 + ((p.ind?.left ?? 0) / 20) * PT_PX;
  const lines = wrapParagraph(p, xLeft, x0 + w, ctx);
  let yCur = y;
  for (const line of lines) {
    const imgItem = line.find((it) => it.img);
    if (imgItem?.img && imgItem.h != null) {
      // image line: top at the flow position, the image's own bottom is its
      // baseline; the line advances by the image height. Text lines only in
      // stats.baselines/lhs — the gate's uniform-height asserts stay text-only.
      const x = alignX(p, xLeft, w, imgItem.w);
      if (bitmaps.has(imgItem.img.part)) {
        ctx.drawImage(bitmaps.get(imgItem.img.part)!, x, yCur, imgItem.w, imgItem.h);
      } else {
        // bytes unavailable (fetch failed): reserved box so the flow
        // position is still evidenced
        ctx.strokeStyle = '#bbb';
        ctx.strokeRect(x, yCur, imgItem.w, imgItem.h);
      }
      stats.media.push({
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
    stats.lhs.push(lh);
    stats.baselines.push(yCur);
    stats.maxw = Math.max(stats.maxw, total);
    if (!stats.metrics) {
      const withFont = line.find((it) => it.r.font && fonts[it.r.font]);
      if (withFont) {
        stats.metrics = fonts[withFont.r.font!];
      }
    }
    let x = alignX(p, xLeft, w, total);
    for (const it of line) {
      ctx.font = runFont(it.r);
      ctx.fillStyle = it.r.color ? `#${it.r.color}` : '#000';
      if (it.r.spacing != null) ctx.letterSpacing = `${(it.r.spacing / 20) * PT_PX}px`;
      if (it.text) ctx.fillText(it.text, x, yCur);
      if (it.r.spacing != null) ctx.letterSpacing = '0px';
      if (it.r.underline && it.text) ctx.fillRect(x, yCur + 2, it.w, 1);
      x += it.w;
    }
    yCur += lh;
  }
  return yCur - y;
}

/** Blocks starting at baseline y; x0/w is the text column. Returns the bottom y. */
function drawBlocks(
  ctx: CanvasRenderingContext2D,
  blocks: Block[],
  x0: number,
  w: number,
  y: number,
  fonts: QuickView['fonts'],
  bitmaps: Map<string, ImageBitmap>,
  stats: Stats,
): number {
  for (const block of blocks) {
    if ('Paragraph' in block) {
      y += drawParagraph(ctx, block.Paragraph, x0, w, y, fonts, bitmaps, stats);
    } else {
      // table: each cell's blocks at its column offset; the row advances by
      // the tallest cell (tblGrid widths land with the V3 media slice — the
      // resolved model does not carry the grid yet)
      const row = block.Table.rows[0];
      if (!row) continue;
      const colW = w / row.length;
      let rowBottom = y;
      row.forEach((cell, col) => {
        rowBottom = Math.max(rowBottom, drawBlocks(ctx, cell.blocks, x0 + col * colW, colW, y, fonts, bitmaps, stats));
      });
      y = rowBottom + 4;
    }
  }
  return y;
}

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
 * paths in URLs). Failures log and leave the part absent: the draw pass
 * reserves its extent box, the view never refuses to render. */
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
        await invoke('frontend_log', { level: 'warn', message: `viewer media-failed: part=${part} err=${e}` });
      }
    }),
  );
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
    // text column: pgSz when declared (twips → px), clamped to the window —
    // the wrap width the greedy line breaker works against
    const pageW = qv.page?.w ? twip(qv.page.w) : innerWidth;
    const colW = Math.min(pageW, innerWidth) - 2 * MARGIN;
    const bitmaps = new Map<string, ImageBitmap>();
    if (qv.media_base) await loadMedia(qv.doc.body, qv.media_base, bitmaps);
    const stats: Stats = { baselines: [], lhs: [], maxw: 0, metrics: null, media: [] };
    drawBlocks(ctx, qv.doc.body, MARGIN, colW, MARGIN + 16, qv.fonts, bitmaps, stats);
    const ms = performance.now() - t0;
    // render evidence: count non-white pixels — text actually hit the canvas
    const img = ctx.getImageData(0, 0, cv.width, cv.height).data;
    let inked = 0;
    for (let i = 0; i < img.length; i += 4 * 97) {
      if (img[i] < 128) inked++; // red channel of every 97th pixel
    }
    const msg = `viewer first-draw ${ms.toFixed(1)} ms (parse ${qv.parse_ms.toFixed(1)} ms, body blocks ${qv.doc.body.length}, inked ${inked})`;
    await invoke('frontend_log', { level: 'info', message: msg });
    const m = stats.metrics;
    const baselines = stats.baselines.slice(0, 40).map((y) => y.toFixed(1)).join(',');
    const lhs = stats.lhs.slice(0, 40).map((h) => h.toFixed(1)).join(',');
    // ADDITIVE log contract with harness/scripts/smoke-quick-view.sh (gate 12):
    // the gate parses `viewer layout:` — change both in the SAME commit.
    const layout = `viewer layout: lines=${stats.baselines.length} colw=${colW.toFixed(1)} maxw=${stats.maxw.toFixed(1)} metrics=${m ? `${m[0]},${m[1]},${m[2]}` : 'none'} lhs=${lhs} baselines=${baselines}`;
    await invoke('frontend_log', { level: 'info', message: layout });
    // ADDITIVE log contract with harness/scripts/smoke-quick-view.sh (gate 12):
    // the gate parses `viewer media:` / `viewer media-failed:` — change both
    // in the SAME commit. Per drawn image: the rect the gate re-derives from
    // EMU (drawn size + flow position) + the inked-pixel count inside it
    // (real bytes hit the canvas — the T-A26 placeholder is white, which
    // never counts as ink).
    for (const m of stats.media) {
      const d = Math.max(1, Math.round(m.w * dpr));
      const dv = Math.max(1, Math.round(m.h * dpr));
      const px = ctx.getImageData(Math.round(m.x * dpr), Math.round(m.y * dpr), d, dv).data;
      let ink = 0;
      for (let i = 0; i < px.length; i += 4) {
        if (px[i + 3] > 0 && !(px[i] > 250 && px[i + 1] > 250 && px[i + 2] > 250)) ink++;
      }
      await invoke('frontend_log', {
        level: 'info',
        message: `viewer media: part=${m.part} emu=${m.emu[0]}x${m.emu[1]} drawn=${m.w.toFixed(1)}x${m.h.toFixed(1)} at=${m.x.toFixed(1)},${m.y.toFixed(1)} ink=${ink}`,
      });
    }
  } catch (e) {
    ctx.fillText(`quick view failed: ${e}`, MARGIN, MARGIN);
    await invoke('frontend_log', { level: 'error', message: `viewer failed: ${e}` });
  }
}

void main();
