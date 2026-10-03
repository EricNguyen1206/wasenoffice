/**
 * T-V7 layout-core coordinate tests — run by gate 14
 * (harness/scripts/test-layout.sh: tsc-compile → plain node, no DOM, no test
 * framework, no deps — the assert helpers below are the whole harness). The
 * fake measurer is EXACT (10px per char), so hit boundaries are
 * hand-computable: mid-line → the right run/offset; line edges → the exact
 * start/end offsets; y bands → the documented midpoint rule.
 */
import {
  hitTest,
  layoutBlocks,
  lineHeight,
  twip,
  type Block,
  type Hit,
  type LineModel,
  type Measurer,
  type Run,
} from './layout.js';

let checked = 0;
let failed = 0;
const fail = (what: string, detail: string): void => {
  console.error(`FAIL ${what}\n     ${detail}`);
  failed++;
};
const eq = (a: unknown, b: unknown, what: string): void => {
  const da = JSON.stringify(a);
  const db = JSON.stringify(b);
  if (da === db) checked++;
  else fail(what, `got ${da}, want ${db}`);
};
const ok = (cond: boolean, what: string): void => {
  if (cond) checked++;
  else fail(what, 'condition false');
};

const measure: Measurer = (_r: Run, text: string): number => 10 * text.length;

const mk = (text: string, extra: Partial<Run> = {}): Run => ({
  text,
  font: null,
  size: null,
  bold: false,
  italic: false,
  underline: null,
  color: null,
  spacing: null,
  images: [],
  ...extra,
});

const para = (...runs: Run[]): Block[] => [
  { Paragraph: { align: null, ind: null, runs } },
];
// one column wide enough for the unwrapped tests
const WIDE = { x0: 0, w: 1000, y: 0, fonts: {}, measure };

const hit = (m: LineModel, x: number, y: number): Hit =>
  hitTest(m, measure, x, y) as Hit; // tests below always have ≥1 line

// ── single run, single line: caret offsets are exact ──────────────────────
{
  const m = layoutBlocks({ ...WIDE, blocks: para(mk('Hello world')) });
  eq(m.lines.length, 1, 'one line');
  eq(m.lines[0].total, 110, 'line width = 11 chars × 10px');
  eq(hit(m, 45, 0), { para: 0, line: 0, run: 0, offset: 4 }, 'mid-line: caret before the 5th char');
  eq(hit(m, 55, 0), { para: 0, line: 0, run: 0, offset: 5 }, 'on the space: caret before it');
  eq(hit(m, 65, 0), { para: 0, line: 0, run: 0, offset: 6 }, 'into "world": caret before "w"');
  eq(hit(m, 0, 0), { para: 0, line: 0, run: 0, offset: 0 }, 'left edge = start');
  eq(hit(m, -5, 0), { para: 0, line: 0, run: 0, offset: 0 }, 'before the line clamps to start');
  eq(hit(m, 110, 0), { para: 0, line: 0, run: 0, offset: 11 }, 'right edge = end');
  eq(hit(m, 1000, 0), { para: 0, line: 0, run: 0, offset: 11 }, 'beyond the line clamps to end');
}

// ── two runs on one line: run resolution ──────────────────────────────────
{
  const m = layoutBlocks({ ...WIDE, blocks: para(mk('Hello '), mk('world')) });
  eq(m.lines.length, 1, 'still one line');
  eq(hit(m, 70, 0), { para: 0, line: 0, run: 1, offset: 1 }, 'x at the w|o boundary = caret after "w"');
  eq(hit(m, 75, 0), { para: 0, line: 0, run: 1, offset: 1 }, 'x inside "o" advance = caret BEFORE it (no half-char centering — documented rule)');
  eq(hit(m, 80, 0), { para: 0, line: 0, run: 1, offset: 2 }, 'x at the o|r boundary = caret after "o"');
  eq(hit(m, 30, 0), { para: 0, line: 0, run: 0, offset: 3 }, 'click in run 0');
  eq(hit(m, 105, 0), { para: 0, line: 0, run: 1, offset: 4 }, 'between "worl" and "d" = offset 4 in run 1');
}

// ── wrap + y bands (nearest baseline, midpoints) ──────────────────────────
{
  const m = layoutBlocks({ ...WIDE, w: 120, blocks: para(mk('aaaa bbbb cccc')) });
  eq(m.lines.length, 2, 'the column forces one wrap');
  eq(m.lines[0].total, 100, 'line 1 width');
  const lh = lineHeight(mk(''), {}); // fallback spacing constant
  eq(m.lines[0].baseline, 0, 'first baseline at y');
  ok(Math.abs(m.lines[1].baseline - lh) < 1e-9, 'second baseline one line-height down');
  eq(hit(m, 20, lh * 0.4), { para: 0, line: 0, run: 0, offset: 2 }, 'y above the midpoint → line 0');
  eq(hit(m, 20, lh * 0.6), { para: 0, line: 1, run: 0, offset: 12 }, 'y below the midpoint → line 1 (offset inside the RUN text: "aaaa bbbb " = 10 + 2)');
  eq(hit(m, 20, lh / 2), { para: 0, line: 0, run: 0, offset: 2 }, 'exact midpoint ties to the EARLIER line');
  eq(hit(m, -3, m.lines[1].baseline), { para: 0, line: 1, run: 0, offset: 10 }, 'mép trái dòng 2 = charStart của "cccc" trong run text');
  eq(hit(m, 41, m.lines[1].baseline), { para: 0, line: 1, run: 0, offset: 14 }, 'mép phải dòng 2 = end (10 + 4)');
  eq(hit(m, 15, m.lines[1].baseline), { para: 0, line: 1, run: 0, offset: 11 }, 'click giữa dòng 2 → đúng offset run');
  eq(m.maxw, 100, 'maxw = widest text line');
}

// ── tab jumps: one control char, width splits before/after ────────────────
{
  const m = layoutBlocks({ ...WIDE, blocks: para(mk('a\tb')) });
  eq(m.lines.length, 1, 'one line');
  const items = m.lines[0].items;
  eq(items.length, 3, 'word + tab + word');
  eq(items[1].w, twip(720) - 10, 'tab advances to the next 720-twip stop');
  eq(hit(m, 20, 0), { para: 0, line: 0, run: 0, offset: 1 }, 'tab left half = caret before \t');
  eq(hit(m, 40, 0), { para: 0, line: 0, run: 0, offset: 2 }, 'tab right half = caret after \t');
}

// ── paragraph index across blocks ─────────────────────────────────────────
{
  const m = layoutBlocks({ ...WIDE, blocks: [...para(mk('xy')), ...para(mk('zw'))] });
  eq(m.lines.length, 2, 'two paragraphs, two lines');
  eq(m.lines[1].para, 1, 'line 1 belongs to block 1');
  eq(hit(m, 15, m.lines[1].baseline), { para: 1, line: 1, run: 0, offset: 1 }, 'hit resolves the paragraph');
}

// ── image line: media surfaces outside `lines`, flow position kept ────────
{
  const img = { rel_id: 'r1', part: 'word/media/p.png', cx_emu: 95250, cy_emu: 47625 };
  const m = layoutBlocks({ ...WIDE, blocks: para(mk('x', { images: [img] })) });
  eq(m.lines.length, 1, 'image takes its OWN line — only the text line is in lines');
  eq(m.media.length, 1, 'media entry emitted');
  eq([m.media[0].w, m.media[0].h], [10, 5], 'wp:extent EMU → px');
  eq(hit(m, 5, m.media[0].y + 1), { para: 0, line: 0, run: 0, offset: 0 }, 'hit in the image band resolves to the nearest text line (mid-char → before it)');
  ok(m.bottom >= m.media[0].y + m.media[0].h, 'bottom covers the media rect');
}

// ── hard break: the EMPTY middle line resolves to the paragraph start ─────
{
  const m = layoutBlocks({ ...WIDE, blocks: para(mk('a\n\nb')) });
  eq(m.lines.length, 3, 'two w:br → text, empty, text');
  const empty = m.lines[1];
  eq(empty.items.length, 0, 'middle line has no items');
  eq(hit(m, 0, empty.baseline), { para: 0, line: 1, run: 0, offset: 0 }, 'empty line → caret placeholder at paragraph start (A4 refines)');
}

console.log(`layout.test: ${checked} checks passed${failed ? `, ${failed} FAILED` : ''}`);
if (failed) throw new Error(`${failed} layout-test checks failed`);
