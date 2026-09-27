#!/bin/zsh
# NanoOffice verification harness — run before claiming any phase complete.
set -u
cd "$(dirname "$0")/../.."
fail=0

echo "▸ cargo check (src-tauri)"
(cd src-tauri && cargo check --quiet) || fail=1

echo "▸ cargo fmt --check (src-tauri)"
(cd src-tauri && cargo fmt --check >/dev/null) || fail=1

echo "▸ cargo clippy -D warnings (src-tauri)"
(cd src-tauri && cargo clippy --quiet -- -D warnings) || fail=1

echo "▸ pnpm build (src)"
pnpm build --logLevel error || fail=1

echo "▸ license allowlist (cargo graph + node_modules)"
node harness/scripts/check-licenses.mjs || fail=1

echo "▸ doc OKF-thin (frontmatter type + links + index coverage)"
node harness/scripts/check-docs.mjs || fail=1

echo "▸ smoke render behavioral (TC-D01-06: boot app, render fixture DOCX)"
harness/scripts/smoke-render.sh || fail=1

echo "▸ upstream submodules clean?"
if [ -n "$(git submodule status | grep '^[+MU]')" ]; then
  echo "  ✗ submodules dirty/different:"; git submodule status; fail=1
else
  echo "  ✓ clean"
fi

echo "▸ no upstream/engines files staged for commit?"
staged=$(git diff --cached --name-only | grep -E '^(upstream/|engines/)' | grep -v '^engines/.gitkeep' | wc -l | tr -d ' ')
if [ "$staged" != "0" ]; then echo "  ✗ $staged upstream/engines files staged"; git diff --cached --name-only | grep -E '^(upstream/|engines/)'; fail=1; else echo "  ✓ none"; fi

if [ $fail -eq 0 ]; then echo "ALL CHECKS PASSED"; else echo "CHECKS FAILED"; fi
exit $fail
