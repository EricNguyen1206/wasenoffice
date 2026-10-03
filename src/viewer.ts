/**
 * V01 quick-view slice (DD docs/design/viewer-a3/DD.md §3.4) — the DRAW side.
 * T-V7: layout lives in the pure module `src/viewer/layout.ts` (wrap pgSz +
 * tab + OS/2 line height + media flow, ported verbatim; parity asset
 * OQ-V01-1 ε=2.0px) — this page only feeds it a canvas-backed measurer,
 * draws the LineModel it returns and logs the gate evidence. The same module
 * answers `hitTest` (the A4 caret foundation); a gate-12 env probe
 * (`NANO_OFFICE_QV_HITTEST`) round-trips every line through it.
 * V3 media: images fetch by fileId from the loopback filesrv
 * (`/media/<id>/<part>`). V5 UX: the canvas is the page — sized to the
 * laid-out content so the body scrolls the whole document; window resizes
 * re-layout + redraw (the canvas used to go stale). No engine shim is
 * loaded on this page (see viewer.html).
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
  type LineModel,
  type Measurer,
  type Run,
} from './viewer/layout';

interface QuickView {
  parse_ms: number;
  doc: { body: Block[] };
  fonts: FontMetrics;
  /** `w:pgSz` of the final section, twips (§17.6.11). */
  page: { w: number | null; h: number | null; orient: string | null } | null;
  /** V3: `<origin>/media/<fileId>` prefix — append the OPC part name to
   * fetch image bytes from the loopback filesrv; null = media disabled. */
  media_base: string | null;
}

const { invoke } = (window as unknown as { __TAURI__: { core: { invoke: <T>(cmd: string, args?: unknown) => Promise<T> } } }).__TAURI__.core;

// V5 scroll: WebKit caps canvas dimensions at 16384 device px — at dpr 2 that
// is 8192 logical px. Taller documents clip (no pagination yet — post-A3).
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
  const ctx = cv.getContext('2d')!;

  try {
    const qv = await invoke<QuickView>('viewer_open');
    const bitmaps = new Map<string, ImageBitmap>();
    if (qv.media_base) await loadMedia(qv.doc.body, qv.media_base, bitmaps);
    // The measurer is the ONLY canvas touch in the layout pipeline (C4-b).
    const measure: Measurer = (r: Run, text: string): number => {
      ctx.font = runFont(r);
      return ctx.measureText(text).width;
    };
    // V5 scroll: the canvas is the page — its height follows the laid-out
    // content (never smaller than the viewport) so the body scrolls the whole
    // document. Resizing the canvas clears it, so size FIRST, then draw once
    // (T-V7: layout no longer needs a measure pass — it is pure input→model).
    const render = (): { colW: number; model: LineModel; docH: number } => {
      // text column: pgSz when declared (twips → px), clamped to the window —
      // the wrap width the greedy line breaker works against
      const pageW = qv.page?.w ? twip(qv.page.w) : innerWidth;
      const colW = Math.min(pageW, innerWidth) - 2 * MARGIN;
      const model = layoutBlocks({
        blocks: qv.doc.body,
        x0: MARGIN,
        w: colW,
        y: MARGIN + 16,
        fonts: qv.fonts,
        measure,
      });
      const docH = Math.min(Math.max(Math.ceil(model.bottom) + MARGIN, innerHeight), MAX_CANVAS_H);
      cv.width = innerWidth * dpr;
      cv.height = docH * dpr;
      cv.style.width = `${innerWidth}px`;
      cv.style.height = `${docH}px`;
      ctx.scale(dpr, dpr);
      draw(model);
      return { colW, model, docH };
    };
    /** Draw the model. Same per-item sequence as the pre-T-V7 interleaved
     * draw — pixel-identical output (nothing overlaps in flow layout). */
    const draw = (model: LineModel): void => {
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
    };

    const { colW, model, docH } = render();
    const ms = performance.now() - t0;
    // render evidence: count non-white pixels — text actually hit the canvas
    const img = ctx.getImageData(0, 0, cv.width, cv.height).data;
    let inked = 0;
    for (let i = 0; i < img.length; i += 4 * 97) {
      if (img[i] < 128) inked++; // red channel of every 97th pixel
    }
    const msg = `viewer first-draw ${ms.toFixed(1)} ms (parse ${qv.parse_ms.toFixed(1)} ms, body blocks ${qv.doc.body.length}, inked ${inked}, canvas ${docH}px)`;
    await invoke('frontend_log', { level: 'info', message: msg });
    const m = model.metrics;
    const baselines = model.lines.slice(0, 40).map((l) => l.baseline.toFixed(1)).join(',');
    const lhs = model.lines.slice(0, 40).map((l) => l.lh.toFixed(1)).join(',');
    // ADDITIVE log contract with harness/scripts/smoke-quick-view.sh (gate 12):
    // the gate parses `viewer layout:` — change both in the SAME commit.
    const layout = `viewer layout: lines=${model.lines.length} colw=${colW.toFixed(1)} maxw=${model.maxw.toFixed(1)} metrics=${m ? `${m[0]},${m[1]},${m[2]}` : 'none'} lhs=${lhs} baselines=${baselines}`;
    await invoke('frontend_log', { level: 'info', message: layout });
    // ADDITIVE log contract with harness/scripts/parity-view.sh (T-V4):
    // the parity harness parses `viewer evidence:` (one JSON line) — change
    // both in the SAME commit. Same shape as the pre-T-V7 Stats.lines.
    await invoke('frontend_log', {
      level: 'info',
      message: `viewer evidence: ${JSON.stringify({
        colw: colW,
        page_w: qv.page?.w ?? null,
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
    });
    // ADDITIVE log contract with harness/scripts/smoke-quick-view.sh (gate 12):
    // the gate parses `viewer media:` / `viewer media-failed:` — change both
    // in the SAME commit. Per drawn image: the rect the gate re-derives from
    // EMU (drawn size + flow position) + the inked-pixel count inside it
    // (real bytes hit the canvas — the T-A26 placeholder is white, which
    // never counts as ink).
    for (const m of model.media) {
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
    // T-V7 gate-12 probe: round-trip every text line through hitTest (mid →
    // inside the line; left/right edges → its boundary offsets). Env-driven
    // (NANO_OFFICE_QV_HITTEST → init script in lib.rs) — the compute is gate
    // food, not production work.
    if ((window as unknown as { __NANO_OFFICE_QV_HITTEST__?: boolean }).__NANO_OFFICE_QV_HITTEST__) {
      const inLine = (line: LineModel['lines'][number], k: number, h: ReturnType<typeof hitTest>): boolean =>
        !!h &&
        h.line === k &&
        (line.items.length === 0 ||
          line.items.some(
            (it) =>
              it.ri === h.run &&
              h.offset >= it.charStart &&
              h.offset <= it.charStart + (it.tab ? 1 : it.text.length),
          ));
      let ok = 0;
      const bad: string[] = [];
      model.lines.forEach((line, k) => {
        const last = line.items[line.items.length - 1];
        const endX = last ? last.x + last.w : line.x;
        const hits = [
          hitTest(model, measure, line.x - 1, line.baseline),
          hitTest(model, measure, line.x + line.total / 2, line.baseline),
          hitTest(model, measure, endX + 1, line.baseline),
        ];
        if (hits.every((h) => inLine(line, k, h))) ok += 1;
        else bad.push(`line ${k} → ${JSON.stringify(hits)}`);
      });
      await invoke('frontend_log', { level: 'info', message: `viewer hit-test: ${ok}/${model.lines.length} lines ok` });
      if (bad.length) {
        await invoke('frontend_log', { level: 'error', message: `viewer hit-test FAIL: ${bad.join(' | ')}` });
      }
    }
    // T-V7: re-layout on resize (the canvas used to go stale). Layout is
    // cheap and pure; rAF coalesces the per-pixel resize bursts.
    let raf = 0;
    window.addEventListener('resize', () => {
      cancelAnimationFrame(raf);
      raf = requestAnimationFrame(() => render());
    });
  } catch (e) {
    cv.width = innerWidth * dpr;
    cv.height = innerHeight * dpr;
    cv.style.width = `${innerWidth}px`;
    cv.style.height = `${innerHeight}px`;
    ctx.scale(dpr, dpr);
    ctx.fillStyle = '#000';
    ctx.fillText(`quick view failed: ${e}`, MARGIN, MARGIN);
    await invoke('frontend_log', { level: 'error', message: `viewer failed: ${e}` });
  }
}

void main();
