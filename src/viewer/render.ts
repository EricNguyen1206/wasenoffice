/**
 * Shared canvas surface of the viewer and the Nano Light editor (T-060 C1,
 * gộp C6 — review #5): media loading, the canvas measurer, the model draw,
 * the resize hook and the evidence JSON. Both entries used to carry a
 * near-verbatim copy of this (~37% of editor.ts); the selection slice would
 * have grown a third. The per-entry halves stay in their entries: viewer =
 * read-only evidence + hit-test probe; editor = command layer, caret
 * overlay, save.
 * LOG CONTRACT: `toEvidence` builds the exact object both entries stringify
 * after their own prefix (`viewer evidence:` / `nano light evidence:` —
 * gate 12/15 + parity-view parse it): key order is load-bearing, do not
 * reorder. `loadMedia`'s failure log carries the entry's tag the same way.
 */
import {
  MARGIN,
  PT_PX,
  runFont,
  twip,
  type Block,
  type LineModel,
  type Measurer,
  type Run,
} from './layout';

// V5 scroll ceiling: WebKit caps canvas dimensions at 16384 device px — at
// dpr 2 that is 8192 logical px. Taller documents clip (pagination post-A3).
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
 * paths in URLs). Failures log under the entry's tag and leave the part
 * absent: the draw pass reserves its extent box, the view never refuses. */
export async function loadMedia(
  blocks: Block[],
  base: string,
  bitmaps: Map<string, ImageBitmap>,
  tag: string,
  invoke: (cmd: string, args: unknown) => Promise<unknown>,
): Promise<void> {
  const parts = new Set<string>();
  imageParts(blocks, parts);
  await Promise.all(
    [...parts].map(async (part) => {
      try {
        const resp = await fetch(`${base}/${encodeURI(part)}`);
        if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
        bitmaps.set(part, await createImageBitmap(await resp.blob()));
      } catch (e) {
        await invoke('frontend_log', { level: 'warn', message: `${tag} media-failed: part=${part} err=${e}` });
      }
    }),
  );
}

/** Bind the measurer to the canvas (C4-b: the ONLY canvas touch in the
 * layout pipeline). */
export function canvasMeasurer(ctx: CanvasRenderingContext2D): Measurer {
  return (r: Run, text: string): number => {
    ctx.font = runFont(r);
    return ctx.measureText(text).width;
  };
}

/** Wrap column width: pgSz − pgMar left/right (T-V6 §5.8), clamped to the
 * viewport; missing margins fall back to the 1in default — byte-identical
 * with the pre-V6 behavior for files without pgMar. */
export function columnWidth(
  page: { w: number | null } | null | undefined,
  mar: { left: number | null; right: number | null } | null | undefined,
  vw: number,
): number {
  const pageW = page?.w ? (page.w / 20) * PT_PX : vw;
  const left = mar?.left != null ? twip(mar.left) : MARGIN;
  const right = mar?.right != null ? twip(mar.right) : MARGIN;
  return Math.min(pageW, vw) - left - right;
}

/** Text origin (x0, first-baseline y) from pgMar (T-V6 §5.8) — y keeps the
 * historical +16 half-leading offset; missing margins fall back to 1in. */
export function pageOrigin(
  mar: { top: number | null; left: number | null } | null | undefined,
): { x0: number; y: number } {
  return {
    x0: mar?.left != null ? twip(mar.left) : MARGIN,
    y: (mar?.top != null ? twip(mar.top) : MARGIN) + 16,
  };
}

/** Layout metrics line WITHOUT the entry prefix — review #6 C2: the frozen
 * log contract (`viewer layout:` gate 12 · `nano light layout:` gate 15)
 * keeps ONE edit-point; each entry prepends its own tag. */
export function layoutMetricLine(model: LineModel, colW: number): string {
  const m = model.metrics;
  const baselines = model.lines.slice(0, 40).map((l) => l.baseline.toFixed(1)).join(',');
  const lhs = model.lines.slice(0, 40).map((l) => l.lh.toFixed(1)).join(',');
  return `lines=${model.lines.length} colw=${colW.toFixed(1)} maxw=${model.maxw.toFixed(1)} metrics=${m ? `${m[0]},${m[1]},${m[2]}` : 'none'} lhs=${lhs} baselines=${baselines}`;
}

/** One media evidence line WITHOUT the entry prefix — same frozen contract
 * (`viewer media:` / `nano light media:`). */
export function mediaEvidenceLine(
  media: LineModel['media'][number],
  ctx: CanvasRenderingContext2D,
  dpr: number,
): string {
  return `part=${media.part} emu=${media.emu[0]}x${media.emu[1]} drawn=${media.w.toFixed(1)}x${media.h.toFixed(1)} at=${media.x.toFixed(1)},${media.y.toFixed(1)} ink=${mediaInk(ctx, media, dpr)}`;
}

/** Size + clear the canvas for the fatal path and print the failure on it.
 * The drawn text and the log line differ per entry (`quick view failed` vs
 * `viewer failed`) — the caller keeps both frozen strings. */
export function drawFatal(
  cv: HTMLCanvasElement,
  ctx: CanvasRenderingContext2D,
  dpr: number,
  vw: number,
  vh: number,
  drawMsg: string,
): void {
  cv.width = vw * dpr;
  cv.height = vh * dpr;
  cv.style.width = `${vw}px`;
  cv.style.height = `${vh}px`;
  ctx.scale(dpr, dpr);
  ctx.fillStyle = '#000';
  ctx.fillText(drawMsg, MARGIN, MARGIN);
}

/** Canvas height the scroll view needs: the laid-out bottom (+margin), never
 * smaller than the viewport, capped at the WebKit ceiling. */
function docHeight(bottom: number, vh: number): number {
  return Math.min(Math.max(Math.ceil(bottom) + MARGIN, vh), MAX_CANVAS_H);
}

/** Size + clear the canvas for a fresh draw, then paint the model. Same
 * per-item sequence as the pre-T-V7 interleaved draw — pixel-identical
 * output (nothing overlaps in flow layout). `overlay` runs after the paint
 * (the editor's caret rect). Resizing the canvas clears it, so size FIRST,
 * then draw once. Returns the canvas height in CSS px. */
export function drawModel(
  model: LineModel,
  cv: HTMLCanvasElement,
  ctx: CanvasRenderingContext2D,
  dpr: number,
  vw: number,
  vh: number,
  bitmaps: ReadonlyMap<string, ImageBitmap>,
  overlay?: (ctx: CanvasRenderingContext2D) => void,
): number {
  const docH = docHeight(model.bottom, vh);
  cv.width = vw * dpr;
  cv.height = docH * dpr;
  cv.style.width = `${vw}px`;
  cv.style.height = `${docH}px`;
  ctx.scale(dpr, dpr);
  for (const m of model.media) {
    if (bitmaps.has(m.part)) {
      ctx.drawImage(bitmaps.get(m.part)!, m.x, m.y, m.w, m.h);
    } else {
      // bytes unavailable (fetch failed): reserved box so the flow
      // position is still evidenced
      ctx.strokeStyle = '#bbb';
      ctx.strokeRect(m.x, m.y, m.w, m.h);
    }
  }
  for (const line of model.lines) {
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
  overlay?.(ctx);
  return docH;
}

/** Inked (non-white) pixels inside a drawn media rect — real bytes hit the
 * canvas (the T-A26 placeholder is white, which never counts as ink). */
export function mediaInk(ctx: CanvasRenderingContext2D, m: LineModel['media'][number], dpr: number): number {
  const d = Math.max(1, Math.round(m.w * dpr));
  const dv = Math.max(1, Math.round(m.h * dpr));
  const px = ctx.getImageData(Math.round(m.x * dpr), Math.round(m.y * dpr), d, dv).data;
  let ink = 0;
  for (let i = 0; i < px.length; i += 4) {
    if (px[i + 3] > 0 && !(px[i] > 250 && px[i + 1] > 250 && px[i + 2] > 250)) ink++;
  }
  return ink;
}

/** The evidence object both entries stringify after their own prefix —
 * key order is the byte-identical log contract, keep as is. */
export function toEvidence(model: LineModel, colW: number, page: { w: number | null } | null | undefined): {
  colw: number;
  page_w: number | null;
  lines: { pi: number; x: number; y: number; t: string; al: string | null; c?: number }[];
} {
  return {
    colw: colW,
    page_w: page?.w ?? null,
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
  };
}

/** Re-layout on resize (the canvas used to go stale): layout is cheap and
 * pure; rAF coalesces the per-pixel resize bursts. */
export function resizeRelayout(render: () => void): void {
  let raf = 0;
  window.addEventListener('resize', () => {
    cancelAnimationFrame(raf);
    raf = requestAnimationFrame(render);
  });
}
