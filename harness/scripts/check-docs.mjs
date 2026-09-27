#!/usr/bin/env node
// check-docs.mjs — gate chuẩn OKF-thin cho docs/ (QĐ8, vision.md).
// Kiểm tra: (1) mọi docs/**/*.md có frontmatter `type` thuộc vocabulary;
// (2) mọi markdown link nội bộ resolve (file/thư mục tồn tại);
// (3) mọi doc có dòng index trong docs/README.md (file hoặc thư mục cha).
// Zero-dependency (node builtin). Exit 1 kèm danh sách vi phạm.
import { readFileSync, readdirSync, existsSync } from 'node:fs';
import { join, dirname, resolve, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..');
const DOCS = join(ROOT, 'docs');

// Vocabulary 1-1 với bảng routing — thêm type mới = sửa đây + conventions.md § OKF-thin.
const TYPES = new Set([
  'decision', 'convention', 'evidence', 'log', 'design', 'test',
  'template', 'queue', 'handoff', 'registry', 'research',
]);

function walkMd(dir, out = []) {
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    if (e.name.startsWith('.') || e.name === 'node_modules') continue;
    const p = join(dir, e.name);
    if (e.isDirectory()) walkMd(p, out);
    else if (e.name.endsWith('.md')) out.push(p);
  }
  return out;
}

const errors = [];
const docFiles = walkMd(DOCS);
const rootMd = ['AGENTS.md', 'CONTRIBUTING.md']
  .map((f) => join(ROOT, f))
  .filter((f) => existsSync(f));

// 1) Frontmatter `type`
for (const f of docFiles) {
  const rel = relative(ROOT, f);
  const text = readFileSync(f, 'utf8');
  const m = text.match(/^---\r?\n([\s\S]*?)\r?\n---\r?\n/);
  if (!m) {
    errors.push(`frontmatter thiếu: ${rel}`);
    continue;
  }
  const t = m[1].match(/^type:\s*(\S+)\s*$/m);
  if (!t) errors.push(`frontmatter thiếu 'type': ${rel}`);
  else if (!TYPES.has(t[1])) errors.push(`type không hợp lệ '${t[1]}': ${rel}`);
}

// 2) Link nội bộ resolve — bỏ qua code fence/inline code, ảnh, http(s), mailto
const stripCode = (s) => s.replace(/```[\s\S]*?```/g, '').replace(/`[^`\n]*`/g, '');
const linkRe = /(!?)\[[^\]]*\]\(([^)\s]+)(?:#[^)]*)?\)/g;
for (const f of [...docFiles, ...rootMd]) {
  const rel = relative(ROOT, f);
  const body = stripCode(readFileSync(f, 'utf8'));
  for (const [, bang, target] of body.matchAll(linkRe)) {
    if (bang === '!') continue;
    if (/^(https?:|mailto:|data:)/i.test(target)) continue;
    const abs = target.startsWith('/') ? join(ROOT, target) : resolve(dirname(f), target);
    if (!existsSync(abs)) errors.push(`link gãy: ${rel} → ${target}`);
  }
}

// 3) Coverage index — mọi doc có dòng trong docs/README.md (file hoặc thư mục cha)
const indexText = readFileSync(join(DOCS, 'README.md'), 'utf8');
for (const f of docFiles) {
  const rel = relative(ROOT, f);
  if (rel === 'docs/README.md') continue; // chính là index
  const dirWithSlash = rel.slice(0, rel.lastIndexOf('/') + 1);
  if (!(indexText.includes(rel) || indexText.includes(dirWithSlash))) {
    errors.push(`chưa có dòng index trong docs/README.md: ${rel}`);
  }
}

if (errors.length) {
  console.error(`  ✗ ${errors.length} vi phạm OKF-thin:`);
  for (const e of errors) console.error(`    - ${e}`);
  process.exit(1);
}
console.log(`  ✓ ${docFiles.length} files: type hợp lệ · 0 link gãy · index coverage đủ`);
