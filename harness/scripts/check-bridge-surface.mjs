#!/usr/bin/env node
// check-bridge-surface.mjs — gate đối chiếu bridge surface 3+1 nơi (T-044,
// review #2 C4; hạ từ ý tưởng "registry sinh code" còn check-script 1 phiên).
//
// Nguồn đối chiếu:
//   (a) src/shim/desktopEditor.js — KNOWN_METHODS (const-block) + LOCAL_TRUTHS
//       keys (object-block, key ở đầu dòng thụt 4 space — bền với body đa dòng)
//   (b) src-tauri/src/bridge/mod.rs — match arms của dispatch (dòng bắt đầu
//       bằng `"Method"` hoặc `| "Method"` cho arm multi-line, trước `_ =>`)
//   (c) docs/reference.map.md — bảng method: section "trạng thái PH-0" cột
//       Method (col 1) cho chiều doc→code; TOÀN FILE backticked tokens
//       (normalize: bỏ suffix `(...)`, expand `A_B/C` → A_B + A_C) cho
//       chiều code→doc (method implemented phải được document)
//   (d) src/shim/globals.d.ts — heuristic sync-truth: method LOCAL_TRUTHS mà
//       nằm trên dòng khai báo Promise → flag (sync method không được
//       khai báo Promise-only)
//
// Zero-dependency (node builtin). Mismatch → exit 1 + list.
import { readFileSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..');
const SHIM = readFileSync(join(ROOT, 'src/shim/desktopEditor.js'), 'utf8');
const RUST = readFileSync(join(ROOT, 'src-tauri/src/bridge/mod.rs'), 'utf8');
const MAP = readFileSync(join(ROOT, 'docs/reference.map.md'), 'utf8');
const DTS = readFileSync(join(ROOT, 'src/shim/globals.d.ts'), 'utf8');

// execCommand nằm ngoài KNOWN_METHODS một cách CÓ Ý ĐÍCH (desktopEditor.js:
// "outside KNOWN_METHODS in upstream terms but part of the same contract")
// — allowlist ở đây thay vì lặng lẽ bỏ qua.
const KNOWN_EXCEPTIONS = new Set(['execCommand']);

// ── (a) shim: KNOWN_METHODS + LOCAL_TRUTHS ──────────────────────────────────
function blockAfter(marker, endRe) {
  const start = SHIM.indexOf(marker);
  if (start === -1) throw new Error(`không tìm thấy ${marker} trong desktopEditor.js`);
  const end = SHIM.slice(start).search(endRe);
  if (end === -1) throw new Error(`không tìm thấy kết thúc block ${marker}`);
  return SHIM.slice(start, start + end);
}

// strip line comments TRƯỚC khi tách chuỗi: comment chứa apostrophe
// (vd "ack'd") làm lệch cặp quote của regex và nuốt method names.
const stripLineComments = (s) => s.replace(/\/\/[^\n]*/g, '');

const knownBlock = stripLineComments(blockAfter('var KNOWN_METHODS = [', /\n {2}\];/));
const KNOWN = new Set([...knownBlock.matchAll(/'([^']+)'/g)].map((m) => m[1]));

const truthsBlock = blockAfter('var LOCAL_TRUTHS = {', /\n {2}\};/);
const TRUTHS = new Set(
  [...truthsBlock.matchAll(/^ {4}([A-Za-z_$][A-Za-z0-9_$]*): function/gm)].map((m) => m[1])
);

// ── (b) Rust dispatch match arms ────────────────────────────────────────────
const DISPATCH = new Set();
{
  const lines = RUST.split('\n');
  const start = lines.findIndex((l) => l.includes('match method {'));
  if (start === -1) throw new Error('không tìm thấy `match method` trong bridge/mod.rs');
  for (const line of lines.slice(start + 1)) {
    const t = line.trim();
    if (/^_\s*=>/.test(t)) break; // fallback arm — hết danh sách method
    // rustfmt: arm/method names luôn bắt đầu dòng bằng `"X"` hoặc `| "X"`
    // (multi-line arm); body arm thụt sâu hơn và không bắt đầu bằng quote/pipe.
    // Chỉ quét TRƯỚC `=>` — sau đó là body (chứa string literal như
    // env!("CARGO_PKG_VERSION") không phải method).
    if (t.startsWith('"') || t.startsWith('| ')) {
      const seg = line.includes('=>') ? line.slice(0, line.indexOf('=>')) : line;
      for (const m of seg.matchAll(/"([A-Za-z_][A-Za-z0-9_]*)"/g)) DISPATCH.add(m[1]);
    }
  }
}

// ── (c) reference.map.md tokens ─────────────────────────────────────────────
// `LocalFileSave(param, …)` → LocalFileSave · `Property_Get/SetCryptoMode`
// → Property_GetCryptoMode + Property_SetCryptoMode (suffix chia sau `_`)
// · token không phải identifier (path, `a[0]`, `pluginsData[]`…) bị bỏ.
function expandToken(tok) {
  const names = new Set();
  const add = (n) => {
    if (/^[A-Za-z_][A-Za-z0-9_]*$/.test(n)) names.add(n);
  };
  const t = tok.replace(/\(.*$/s, '').trim();
  if (t.includes('/')) {
    const parts = t.split('/');
    add(parts[0]);
    const cut = parts[0].lastIndexOf('_');
    if (cut !== -1) {
      const base = parts[0].slice(0, cut + 1);
      for (let i = 1; i < parts.length; i++) add(base + parts[i]);
    }
  } else {
    add(t);
  }
  return names;
}

const DOC_TOKENS = new Set();
for (const m of MAP.matchAll(/`([^`\n]+)`/g)) {
  for (const n of expandToken(m[1])) DOC_TOKENS.add(n);
}

// section PH-0: cột Method (field thứ 2 giữa cặp `|`) — chiều doc→code
const PH0_METHODS = new Set();
{
  const secStart = MAP.indexOf('## Bridge JS→Native — trạng thái PH-0');
  const secEnd = MAP.indexOf('\n## ', secStart + 1);
  const section = MAP.slice(secStart, secEnd === -1 ? undefined : secEnd);
  for (const line of section.split('\n')) {
    if (!line.startsWith('|')) continue;
    const col1 = line.split('|')[1] ?? '';
    for (const m of col1.matchAll(/`([^`]+)`/g)) {
      for (const n of expandToken(m[1])) PH0_METHODS.add(n);
    }
  }
}

// ── đối chiếu ───────────────────────────────────────────────────────────────
const errors = [];
const shimSurface = new Set([...KNOWN, ...KNOWN_EXCEPTIONS]);

for (const t of TRUTHS) {
  if (!shimSurface.has(t)) {
    errors.push(`LOCAL_TRUTHS '${t}' không nằm trong KNOWN_METHODS (desktopEditor.js)`);
  }
}
for (const d of DISPATCH) {
  if (!shimSurface.has(d)) {
    errors.push(`dispatch arm '${d}' (bridge/mod.rs) không nằm trong KNOWN_METHODS — shim không bao giờ gửi, arm chết hoặc typo`);
  }
}
for (const impl of new Set([...DISPATCH, ...TRUTHS])) {
  if (!DOC_TOKENS.has(impl)) {
    errors.push(`method implemented '${impl}' (dispatch ∪ LOCAL_TRUTHS) không được document trong reference.map.md`);
  }
}
for (const doc of PH0_METHODS) {
  if (!KNOWN.has(doc) && !DISPATCH.has(doc) && !TRUTHS.has(doc) && !KNOWN_EXCEPTIONS.has(doc)) {
    errors.push(`bảng PH-0 document method '${doc}' nhưng shim không biết (thiếu KNOWN_METHODS entry?)`);
  }
}

// ── (d) globals.d.ts: sync-truth không được khai báo Promise-only ───────────
for (const t of TRUTHS) {
  const re = new RegExp(`^.*\\b${t}\\b.*$`, 'gm');
  for (const line of DTS.match(re) ?? []) {
    if (line.includes('Promise')) {
      errors.push(`globals.d.ts khai báo '${t}' (sync-truth/LOCAL_TRUTHS) trên dòng chứa Promise — trả về sync, không Promise-only`);
    }
  }
}

if (errors.length) {
  console.error(`  ✗ ${errors.length} mismatch bridge surface:`);
  for (const e of errors) console.error(`    - ${e}`);
  process.exit(1);
}
console.log(
  `  ✓ bridge surface khớp: KNOWN=${KNOWN.size} · LOCAL_TRUTHS=${TRUTHS.size} · dispatch=${DISPATCH.size} · PH0-doc=${PH0_METHODS.size} · globals.d.ts sync-ok`
);
