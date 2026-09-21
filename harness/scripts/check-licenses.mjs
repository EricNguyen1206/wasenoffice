#!/usr/bin/env node
// NanoOffice license gate — allowlist-only cho toàn bộ cargo graph + node_modules.
// ThDependency nào có license ngoài allowlist hoặc không khai báo → exit 1.
// Rule liên quan: harness/rules/mit-purity.md (điều 3).
import { execFileSync } from 'node:child_process';
import { readdirSync, readFileSync, existsSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..');

// SPDX allowlist — permissive only. Bổ sung mới phải ghi justification ở đây.
const ALLOWED = new Set([
  'MIT', 'MIT-0', 'Apache-2.0', 'ISC', 'BSD-2-Clause', 'BSD-3-Clause', '0BSD',
  'MPL-2.0', 'CC0-1.0', 'Unlicense', 'Zlib', 'BSL-1.0', 'BlueOak-1.0.0',
  'Unicode-3.0', 'Unicode-DFS-2016', 'CDLA-Permissive-2.0',
]);

// SPDX expression: "A OR B", "A AND B", "A WITH exc" — WITH-exception giữ base id.
// Nhiều crate cũ ghi dual-license bằng "/" (vd "MIT/Apache-2.0") = OR semantics.
function exprOk(expr) {
  const norm = expr
    .replace(/\//g, ' OR ')
    .replace(/\s+WITH\s+\S+/gi, '')
    .replace(/[()]/g, '')
    .trim();
  return norm.split(/\s+OR\s+/i).some((clause) =>
    clause.split(/\s+AND\s+/i).every((id) => ALLOWED.has(id.replace(/\+$/, '').trim()))
  );
}

const violations = [];

// ---- Rust: toàn bộ graph từ cargo metadata ----
try {
  const meta = JSON.parse(execFileSync('cargo', ['metadata', '--format-version', '1'], {
    cwd: join(ROOT, 'src-tauri'), encoding: 'utf8', maxBuffer: 64 * 1024 * 1024,
  }));
  const rootId = meta.resolve?.root ?? meta.packages.find((p) => p.name === 'nanooffice')?.id;
  for (const p of meta.packages) {
    if (p.id === rootId) continue; // code của mình, không tự kiểm license
    if (!p.license || !exprOk(p.license)) {
      violations.push(`cargo: ${p.name}@${p.version} — license="${p.license || '(không khai báo)'}"`);
    }
  }
  console.log(`  cargo graph: ${meta.packages.length - 1} packages kiểm tra`);
} catch (e) {
  console.error('  ✗ không chạy được cargo metadata:', e.message.split('\n')[0]);
  process.exit(1);
}

// ---- Node: mọi package thật trong node_modules (kể cả .pnpm store của pnpm) ----
const nm = join(ROOT, 'node_modules');
let checked = 0;
const seen = new Set();
function checkPkgDir(dir) {
  const pj = join(dir, 'package.json');
  if (!existsSync(pj)) return;
  let pkg;
  try { pkg = JSON.parse(readFileSync(pj, 'utf8')); } catch { return; }
  const key = `${pkg.name}@${pkg.version}`;
  if (!pkg.name || seen.has(key)) return;
  seen.add(key);
  checked++;
  const lic = typeof pkg.license === 'string' ? pkg.license : pkg.license?.type;
  if (!lic || !exprOk(lic)) {
    violations.push(`node: ${key} — license="${lic || '(không khai báo)'}"`);
  }
}
if (existsSync(nm)) {
  for (const name of readdirSync(nm)) {
    if (name.startsWith('.')) continue;
    if (name.startsWith('@')) {
      for (const sub of readdirSync(join(nm, name))) checkPkgDir(join(nm, name, sub));
    } else if (name === '.pnpm') {
      const store = join(nm, '.pnpm');
      for (const entry of readdirSync(store)) {
        const inner = join(store, entry, 'node_modules');
        if (!existsSync(inner)) continue;
        for (const pkgName of readdirSync(inner)) {
          if (pkgName.startsWith('@')) {
            for (const sub of readdirSync(join(inner, pkgName))) checkPkgDir(join(inner, pkgName, sub));
          } else checkPkgDir(join(inner, pkgName));
        }
      }
    } else checkPkgDir(join(nm, name));
  }
  console.log(`  node_modules: ${checked} packages kiểm tra`);
} else {
  console.log('  node_modules: không tồn tại — bỏ qua (chạy pnpm install trước)');
}

if (violations.length) {
  console.error('  ✗ license ngoài allowlist:');
  for (const v of violations) console.error('    ' + v);
  process.exit(1);
}
console.log('  ✓ tất cả license nằm trong allowlist');
