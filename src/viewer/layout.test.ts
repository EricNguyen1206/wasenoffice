/**
 * T-V7 layout-core coordinate tests — run by gate 14
 * (harness/scripts/test-layout.sh: tsc-compile → plain node, no DOM, no test
 * framework, no deps — the assert helpers below are the whole harness). The
 * fake measurer is EXACT (10px per char), so hit boundaries are
 * hand-computable: mid-line → the right run/offset; line edges → the exact
 * start/end offsets; y bands → the documented midpoint rule.
 */
import {
  caretXY,
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
  { Paragraph: { align: null, ind: null, spacing: null, mark: null, runs } },
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
  eq(hit(m, 45, 0), { caret: { para: 0, run: 0, offset: 4, cell: null, cellPara: null }, line: 0, run: 0, offset: 4 }, 'mid-line: caret before the 5th char');
  eq(hit(m, 55, 0), { caret: { para: 0, run: 0, offset: 5, cell: null, cellPara: null }, line: 0, run: 0, offset: 5 }, 'on the space: caret before it');
  eq(hit(m, 65, 0), { caret: { para: 0, run: 0, offset: 6, cell: null, cellPara: null }, line: 0, run: 0, offset: 6 }, 'into "world": caret before "w"');
  eq(hit(m, 0, 0), { caret: { para: 0, run: 0, offset: 0, cell: null, cellPara: null }, line: 0, run: 0, offset: 0 }, 'left edge = start');
  eq(hit(m, -5, 0), { caret: { para: 0, run: 0, offset: 0, cell: null, cellPara: null }, line: 0, run: 0, offset: 0 }, 'before the line clamps to start');
  eq(hit(m, 110, 0), { caret: { para: 0, run: 0, offset: 11, cell: null, cellPara: null }, line: 0, run: 0, offset: 11 }, 'right edge = end');
  eq(hit(m, 1000, 0), { caret: { para: 0, run: 0, offset: 11, cell: null, cellPara: null }, line: 0, run: 0, offset: 11 }, 'beyond the line clamps to end');
}

// ── two runs on one line: run resolution ──────────────────────────────────
{
  const m = layoutBlocks({ ...WIDE, blocks: para(mk('Hello '), mk('world')) });
  eq(m.lines.length, 1, 'still one line');
  eq(hit(m, 70, 0), { caret: { para: 0, run: 1, offset: 1, cell: null, cellPara: null }, line: 0, run: 1, offset: 1 }, 'x at the w|o boundary = caret after "w"');
  eq(hit(m, 75, 0), { caret: { para: 0, run: 1, offset: 1, cell: null, cellPara: null }, line: 0, run: 1, offset: 1 }, 'x inside "o" advance = caret BEFORE it (no half-char centering — documented rule)');
  eq(hit(m, 80, 0), { caret: { para: 0, run: 1, offset: 2, cell: null, cellPara: null }, line: 0, run: 1, offset: 2 }, 'x at the o|r boundary = caret after "o"');
  eq(hit(m, 30, 0), { caret: { para: 0, run: 0, offset: 3, cell: null, cellPara: null }, line: 0, run: 0, offset: 3 }, 'click in run 0');
  eq(hit(m, 105, 0), { caret: { para: 0, run: 1, offset: 4, cell: null, cellPara: null }, line: 0, run: 1, offset: 4 }, 'between "worl" and "d" = offset 4 in run 1');
}

// ── wrap + y bands (nearest baseline, midpoints) ──────────────────────────
{
  const m = layoutBlocks({ ...WIDE, w: 120, blocks: para(mk('aaaa bbbb cccc')) });
  eq(m.lines.length, 2, 'the column forces one wrap');
  eq(m.lines[0].total, 100, 'line 1 width');
  const lh = lineHeight(mk(''), {}); // fallback spacing constant
  eq(m.lines[0].baseline, 0, 'first baseline at y');
  ok(Math.abs(m.lines[1].baseline - lh) < 1e-9, 'second baseline one line-height down');
  eq(hit(m, 20, lh * 0.4), { caret: { para: 0, run: 0, offset: 2, cell: null, cellPara: null }, line: 0, run: 0, offset: 2 }, 'y above the midpoint → line 0');
  eq(hit(m, 20, lh * 0.6), { caret: { para: 0, run: 0, offset: 12, cell: null, cellPara: null }, line: 1, run: 0, offset: 12 }, 'y below the midpoint → line 1 (offset inside the RUN text: "aaaa bbbb " = 10 + 2)');
  eq(hit(m, 20, lh / 2), { caret: { para: 0, run: 0, offset: 2, cell: null, cellPara: null }, line: 0, run: 0, offset: 2 }, 'exact midpoint ties to the EARLIER line');
  eq(hit(m, -3, m.lines[1].baseline), { caret: { para: 0, run: 0, offset: 10, cell: null, cellPara: null }, line: 1, run: 0, offset: 10 }, 'mép trái dòng 2 = charStart của "cccc" trong run text');
  eq(hit(m, 41, m.lines[1].baseline), { caret: { para: 0, run: 0, offset: 14, cell: null, cellPara: null }, line: 1, run: 0, offset: 14 }, 'mép phải dòng 2 = end (10 + 4)');
  eq(hit(m, 15, m.lines[1].baseline), { caret: { para: 0, run: 0, offset: 11, cell: null, cellPara: null }, line: 1, run: 0, offset: 11 }, 'click giữa dòng 2 → đúng offset run');
  eq(m.maxw, 100, 'maxw = widest text line');
}

// ── tab jumps: one control char, width splits before/after ────────────────
{
  const m = layoutBlocks({ ...WIDE, blocks: para(mk('a\tb')) });
  eq(m.lines.length, 1, 'one line');
  const items = m.lines[0].items;
  eq(items.length, 3, 'word + tab + word');
  eq(items[1].w, twip(720) - 10, 'tab advances to the next 720-twip stop');
  eq(hit(m, 20, 0), { caret: { para: 0, run: 0, offset: 1, cell: null, cellPara: null }, line: 0, run: 0, offset: 1 }, 'tab left half = caret before \t');
  eq(hit(m, 40, 0), { caret: { para: 0, run: 0, offset: 2, cell: null, cellPara: null }, line: 0, run: 0, offset: 2 }, 'tab right half = caret after \t');
}

// ── paragraph index across blocks ─────────────────────────────────────────
{
  const m = layoutBlocks({ ...WIDE, blocks: [...para(mk('xy')), ...para(mk('zw'))] });
  eq(m.lines.length, 2, 'two paragraphs, two lines');
  eq(m.lines[1].para, 1, 'line 1 belongs to block 1');
  eq(hit(m, 15, m.lines[1].baseline), { caret: { para: 1, run: 0, offset: 1, cell: null, cellPara: null }, line: 1, run: 0, offset: 1 }, 'hit resolves the paragraph');
}

// ── image line: media surfaces outside `lines`, flow position kept ────────
{
  const img = { rel_id: 'r1', part: 'word/media/p.png', cx_emu: 95250, cy_emu: 47625 };
  const m = layoutBlocks({ ...WIDE, blocks: para(mk('x', { images: [img] })) });
  eq(m.lines.length, 1, 'image takes its OWN line — only the text line is in lines');
  eq(m.media.length, 1, 'media entry emitted');
  eq([m.media[0].w, m.media[0].h], [10, 5], 'wp:extent EMU → px');
  eq(hit(m, 5, m.media[0].y + 1), { caret: { para: 0, run: 0, offset: 0, cell: null, cellPara: null }, line: 0, run: 0, offset: 0 }, 'hit in the image band resolves to the nearest text line (mid-char → before it)');
  ok(m.bottom >= m.media[0].y + m.media[0].h, 'bottom covers the media rect');
}

// ── hard break: the EMPTY middle line resolves to the paragraph start ─────
{
  const m = layoutBlocks({ ...WIDE, blocks: para(mk('a\n\nb')) });
  eq(m.lines.length, 3, 'two w:br → text, empty, text');
  const empty = m.lines[1];
  eq(empty.items.length, 0, 'middle line has no items');
  eq(hit(m, 0, empty.baseline), { caret: { para: 0, run: 0, offset: 0, cell: null, cellPara: null }, line: 1, run: 0, offset: 0 }, 'empty line → caret placeholder at paragraph start (A4 refines)');
}

// ── T-060 C3: table cell identity — `para` collides, `block`+`cell` do not ─
{
  const cellP = (t: string): Block => ({ Paragraph: { align: null, ind: null, spacing: null, mark: null, runs: [mk(t)] } });
  const blocks: Block[] = [
    cellP('ab'),
    { Table: { rows: [[{ blocks: [cellP('cde'), cellP('f')] }]] } },
    cellP('after'),
  ];
  const m = layoutBlocks({ ...WIDE, blocks });
  eq(m.lines.length, 4, '2 top paragraphs + 2 in-cell paragraphs');
  eq(m.lines[1].para, 2, 'first in-cell line carries the pi that ALSO belongs to the block after the table (the collision)');
  eq([m.lines[1].block, m.lines[1].cell], [1, [0, 0]], 'its block = the TABLE index + the cell identity');
  eq(m.lines[2].para, 3, 'second in-cell paragraph: next pi, same table block');
  eq([m.lines[2].block, m.lines[2].cell], [1, [0, 0]], 'still the table block + the cell identity');
  eq([m.lines[3].block, m.lines[3].cell], [2, null], 'paragraph after the table: own block, no cell');
  eq([m.lines[0].block, m.lines[0].cell], [0, null], 'plain top-level paragraph: own block, no cell');
  const hCell = hit(m, 15, m.lines[1].baseline); // mid "cde"
  eq([hCell.caret.para, hCell.caret.cell], [1, [0, 0]], 'hit in the cell → table-block caret + cell identity (the pi collision is not even exposed)');
  const hAfter = hit(m, 30, m.lines[3].baseline); // mid "after"
  eq([hAfter.caret.para, hAfter.caret.cell], [2, null], 'same pi as the cell line, different block — the caret addresses the BLOCK, not the walk index');
  // T-A28 §5.3a: caretXY matches the BODY BLOCK index (the editor's
  // Caret.para) — cell lines require the full identity (review #6 C1)
  eq(
    caretXY(m, measure, { para: 1, run: 0, offset: 1, cell: null, cellPara: null }),
    null,
    'block-only caret at a TABLE block resolves to NOTHING — cell lines carry identity; such carets are refused upstream',
  );
  const cAfter = caretXY(m, measure, { para: 2, run: 0, offset: 2, cell: null, cellPara: null });
  ok(!!cAfter && cAfter.y === m.lines[3].baseline, 'caretXY(block 2) = the after-table line, NOT the cell line sharing pi=2');
  ok(!!cAfter && cAfter.line === 3, 'caretXY returns the line index (additive, editor Up/Down uses it)');
}

// ── T-A30 §5.5: cellPara identity + caretXY full-identity + tie-break ─────
{
  const cellP = (t: string): Block => ({ Paragraph: { align: null, ind: null, spacing: null, mark: null, runs: [mk(t)] } });
  const blocks: Block[] = [
    { Table: { rows: [[{ blocks: [cellP('cde'), cellP('f')] }, { blocks: [cellP('gh')] }]] } },
  ];
  const m = layoutBlocks({ ...WIDE, blocks });
  eq([m.lines[0].block, m.lines[0].cell, m.lines[0].cellPara], [0, [0, 0], 0], 'cell A para 0: table block + cell + cellPara 0');
  eq([m.lines[1].block, m.lines[1].cell, m.lines[1].cellPara], [0, [0, 0], 1], 'cell A para 1: same cell, next cellPara');
  eq([m.lines[2].block, m.lines[2].cell, m.lines[2].cellPara], [0, [0, 1], 0], 'cell B para 0: own cell, cellPara 0');
  ok(m.lines[0].baseline === m.lines[2].baseline, 'cells of one row share the exact baseline (the collision case)');
  eq(hit(m, 15, m.lines[0].baseline), { caret: { para: 0, run: 0, offset: 1, cell: [0, 0], cellPara: 0 }, line: 0, run: 0, offset: 1 }, 'tie-break: x inside cell A span → cell A line');
  eq(hit(m, 510, m.lines[2].baseline), { caret: { para: 0, run: 0, offset: 1, cell: [0, 1], cellPara: 0 }, line: 2, run: 0, offset: 1 }, 'tie-break: x inside cell B span → cell B line (NOT the earlier same-baseline cell A line)');
  const cFull = caretXY(m, measure, { para: 0, run: 0, offset: 0, cell: [0, 0], cellPara: 1 });
  ok(!!cFull && cFull.line === 1 && cFull.y === m.lines[1].baseline, 'caretXY with cell+cellPara resolves the SECOND cell paragraph');
  const cLegacy = caretXY(m, measure, { para: 0, run: 0, offset: 0, cell: null, cellPara: null });
  eq(cLegacy, null, 'block-only caret on a TABLE block matches nothing — identity is REQUIRED (review #6 C1)');
  const cNestedCell = caretXY(m, measure, { para: 0, run: 0, offset: 0, cell: [0, 1], cellPara: null });
  ok(cNestedCell === null, 'nested-caret coordinates (cellPara null) match nothing here — only nested-table lines carry cellPara null');
  const hCell = hit(m, 5, m.lines[1].baseline);
  eq([hCell.caret.cell, hCell.caret.cellPara], [[0, 0], 1], 'hit in cell A para 2 carries the full identity');
}

// ── T-060 C3: hitTest ↔ caretXY round-trip (char boundaries are exact
// under the 10px/char fake; mid-char hits floor to the boundary by rule)
{
  const m = layoutBlocks({ ...WIDE, blocks: para(mk('Hello '), mk('world')) });
  for (let k = 0; k <= 11; k++) {
    const x = k * 10;
    const c = caretXY(m, measure, hit(m, x, 0).caret);
    ok(c !== null, `round-trip boundary ${k}: caret resolves`);
    if (c) {
      eq(Math.abs(c.x - x) < 1e-9, true, `round-trip boundary ${k}: caret x ${c.x} ≈ hit x ${x}`);
      eq(c.y, 0, `round-trip boundary ${k}: caret y = the line baseline`);
    }
  }
  // wrapped model: the round-trip holds on the second line too (x within
  // the line span; beyond it both sides clamp to the line end)
  const w = layoutBlocks({ ...WIDE, w: 120, blocks: para(mk('aaaa bbbb cccc')) });
  const y1 = w.lines[1].baseline;
  for (const x of [0, 20, 40]) {
    const c = caretXY(w, measure, hit(w, x, y1).caret);
    ok(!!c && Math.abs(c.x - x) < 1e-9 && c.y === y1, `round-trip wrapped line x=${x} → (${c?.x}, ${c?.y}), want (${x}, ${y1})`);
  }
}

// ── T-V6 §5.8: paragraph spacing (§17.3.1.33) — gap = prev.after + cur.before ─
{
  const P = (before: number | null, after: number | null): Block => ({
    Paragraph: { align: null, ind: null, spacing: { before, after }, mark: null, runs: [mk('word')] },
  });
  const m = layoutBlocks({ ...WIDE, blocks: [P(null, 200), P(100, null)] });
  const lh = lineHeight(mk(''), {});
  ok(
    Math.abs(m.lines[1].baseline - m.lines[0].baseline - lh - twip(300)) < 1e-9,
    'gap = prev.after + cur.before (twip 300 → exactly 20px)',
  );
  const m2 = layoutBlocks({ ...WIDE, y: 0, blocks: [P(150, null)] });
  ok(Math.abs(m2.lines[0].baseline - twip(150)) < 1e-9, 'before applies at the TOP of the document (Word parity)');
  // reset across a table boundary: the after before a table is dropped, the
  // paragraph after the table keeps its own before (DD V01 §5.8 ceiling)
  const cp: Block = { Paragraph: { align: null, ind: null, spacing: null, mark: null, runs: [mk('cell')] } };
  const m3 = layoutBlocks({
    ...WIDE,
    y: 0,
    blocks: [P(null, 500), { Table: { rows: [[{ blocks: [cp] }]] } }, P(150, null)],
  });
  ok(
    Math.abs(m3.lines[2].baseline - (2 * lh + 4 + twip(150))) < 1e-9,
    'after before a TABLE is dropped; the para after the table gets only its own before (+4 row gap)',
  );
}

// ── T-V6 §5.8: numbering mark rides as the FIRST item (not model text) ─────
{
  const m = layoutBlocks({
    ...WIDE,
    blocks: [{ Paragraph: { align: null, ind: null, spacing: null, mark: '1. ', runs: [mk('item')] } }],
  });
  const items = m.lines[0].items;
  eq(items.length, 3, 'mark splits into word+space tokens + the real word');
  eq([items[0].text, items[0].ri, items[0].charStart], ['1.', 0, 0], 'mark token maps to run 0 offset 0 — not editable');
  eq([items[1].text, items[1].charStart], [' ', 2], 'the mark trailing space keeps its charStart inside the mark');
  eq([items[2].text, items[2].ri, items[2].charStart], ['item', 0, 0], 'the real word keeps the MODEL run index 0 (mark does not shift ri)');
  eq(m.lines[0].total, 70, 'line width includes the mark (2+1+4 chars × 10px)');
  eq(hit(m, 5, 0).caret.offset, 0, 'hit inside the mark → offset 0');
  eq(hit(m, 25, 0).caret.offset, 2, 'hit inside the mark trailing space → offset 2 (still the mark)');
  eq(hit(m, 45, 0).caret.offset, 1, 'hit in the word → offset 1');
}

console.log(`layout.test: ${checked} checks passed${failed ? `, ${failed} FAILED` : ''}`);
if (failed) throw new Error(`${failed} layout-test checks failed`);
