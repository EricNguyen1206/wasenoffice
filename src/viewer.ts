/**
 * V01 quick-view slice (DD docs/design/viewer-a3/DD.md §3.4) — the READ-ONLY
 * entry: feed the pure layout module a canvas measurer, draw its LineModel,
 * log the gate evidence, round-trip every line through `hitTest` (gate-12
 * probe). The canvas/media/evidence machinery lives in `src/viewer/render.ts`
 * (shared with the Nano Light editor since T-060 C1); the layout itself is
 * `src/viewer/layout.ts` (parity asset — consumed, never forked). V5 UX: the
 * canvas is the page — sized to the laid-out content so the body scrolls the
 * whole document. No engine shim is loaded on this page (see viewer.html).
 */
import {
  layoutMetricLine,
  loadMedia,
  mediaEvidenceLine,
  resizeRelayout,
  toEvidence,
} from './viewer/render';
import { hitTest, type Block, type FontMetrics, type LineModel } from './viewer/layout';
import { DocumentSurface } from './viewer/surface';

/** T-A28 C5: mirrors the Rust `QuickView` payload (src-tauri/src/viewer.rs)
 * in the ONE camelCase casing — every Rust→JS payload struct is
 * `#[serde(rename_all = "camelCase")]` (lesson T-059). */
interface QuickView {
  name: string;
  parseMs: number;
  doc: { body: Block[] };
  fonts: FontMetrics;
  /** `w:pgSz` of the final section, twips (§17.6.11). */
  page: { w: number | null; h: number | null; orient: string | null } | null;
  /** `w:pgMar` of the final section (T-V6) — text origin + wrap margins. */
  mar: { top: number | null; right: number | null; bottom: number | null; left: number | null } | null;
  /** V3: `<origin>/media/<fileId>` prefix — append the OPC part name to
   * fetch image bytes from the loopback filesrv; null = media disabled. */
  mediaBase: string | null;
}

const { invoke } = (window as unknown as { __TAURI__: { core: { invoke: <T>(cmd: string, args?: unknown) => Promise<T> } } }).__TAURI__.core;

async function main(): Promise<void> {
  const t0 = performance.now();
  let surface: DocumentSurface | null = null;

  try {
    surface = new DocumentSurface('cv');
    const qv = await invoke<QuickView>('viewer_open');
    const nameEl = document.getElementById('doc-name');
    if (nameEl) nameEl.textContent = qv.name;
    const bitmaps = new Map<string, ImageBitmap>();
    if (qv.mediaBase) await loadMedia(qv.doc.body, qv.mediaBase, bitmaps, 'viewer', invoke);

    const render = () =>
      surface!.layoutAndDraw(
        { blocks: qv.doc.body, fonts: qv.fonts, page: qv.page, mar: qv.mar },
        { bitmaps },
      );

    const { colW, model, docH } = render();
    const ms = performance.now() - t0;
    const inked = surface.countInked();
    const msg = `viewer first-draw ${ms.toFixed(1)} ms (parse ${qv.parseMs.toFixed(1)} ms, body blocks ${qv.doc.body.length}, inked ${inked}, canvas ${docH}px)`;
    await invoke('frontend_log', { level: 'info', message: msg });
    // ADDITIVE log contract with harness/scripts/smoke-quick-view.sh (gate 12):
    // the gate parses `viewer layout:` — change both in the SAME commit.
    // Suffix built by the shared layoutMetricLine (review #6 C2 — byte-identical).
    const layout = `viewer layout: ${layoutMetricLine(model, colW)}`;
    await invoke('frontend_log', { level: 'info', message: layout });
    // ADDITIVE log contract with harness/scripts/parity-view.sh (T-V4):
    // the parity harness parses `viewer evidence:` (one JSON line) — change
    // both in the SAME commit. Same shape as the pre-T-V7 Stats.lines.
    await invoke('frontend_log', {
      level: 'info',
      message: `viewer evidence: ${JSON.stringify(toEvidence(model, colW, qv.page))}`,
    });
    // ADDITIVE log contract with harness/scripts/smoke-quick-view.sh (gate 12):
    // the gate parses `viewer media:` / `viewer media-failed:` — change both
    // in the SAME commit. Per drawn image: the rect the gate re-derives from
    // EMU (drawn size + flow position) + the inked-pixel count inside it.
    for (const media of model.media) {
      await invoke('frontend_log', {
        level: 'info',
        message: `viewer media: ${mediaEvidenceLine(media, surface.ctx, surface.dpr)}`,
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
          hitTest(model, surface!.measure, line.x - 1, line.baseline),
          hitTest(model, surface!.measure, line.x + line.total / 2, line.baseline),
          hitTest(model, surface!.measure, endX + 1, line.baseline),
        ];
        if (hits.every((h) => inLine(line, k, h))) ok += 1;
        else bad.push(`line ${k} → ${JSON.stringify(hits)}`);
      });
      await invoke('frontend_log', { level: 'info', message: `viewer hit-test: ${ok}/${model.lines.length} lines ok` });
      if (bad.length) {
        await invoke('frontend_log', { level: 'error', message: `viewer hit-test FAIL: ${bad.join(' | ')}` });
      }
    }
    resizeRelayout(() => render());
  } catch (e) {
    surface?.showFatal(`quick view failed: ${e}`);
    await invoke('frontend_log', { level: 'error', message: `viewer failed: ${e}` });
  }
}

void main();
