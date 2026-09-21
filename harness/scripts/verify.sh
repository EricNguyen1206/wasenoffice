#!/bin/zsh
# NanoOffice verification harness — run before claiming any phase complete.
set -u
cd "$(dirname "$0")/../.."
fail=0

echo "▸ cargo check (src-tauri)"
(cd src-tauri && cargo check --quiet) || fail=1

echo "▸ pnpm build (src)"
pnpm build --logLevel error || fail=1

echo "▸ upstream submodules clean?"
if [ -n "$(git submodule status | grep '^[+MU]')" ]; then
  echo "  ✗ submodules dirty/different:"; git submodule status; fail=1
else
  echo "  ✓ clean"
fi

echo "▸ no upstream files staged for commit?"
staged=$(git diff --cached --name-only | grep -c '^upstream/' || true)
if [ "$staged" != "0" ]; then echo "  ✗ $staged upstream files staged"; fail=1; else echo "  ✓ none"; fi

if [ $fail -eq 0 ]; then echo "ALL CHECKS PASSED"; else echo "CHECKS FAILED"; fi
exit $fail
