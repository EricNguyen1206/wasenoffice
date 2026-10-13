#!/bin/bash
# WasenOffice gate 14 — layout-core unit tests (T-V7): the PURE layout module
# (src/viewer/layout.ts) compiled with the repo's tsc and run under plain
# node — no DOM, no canvas, no test framework. The measurer is a fixed
# 10px/char fake, so the hit-test assertions are exact hand-computed
# coordinates: mid-line → right run/offset, line edges → exact boundary
# offsets, y bands → the documented midpoint rule.
# This gate owns the SEMANTICS of layout+hitTest; gate 12 owns the E2E
# (real canvas measurer, real fixture, round-trip assert).
set -u
cd "$(dirname "$0")/../.."

OUT="$(mktemp -d /tmp/wasenoffice-layout-test-XXXXXX)"
trap 'rm -rf "$OUT"' EXIT

# NODENEXT: the test imports './layout.js' — tsc resolves it to the .ts
# source and emits plain JS node runs without a loader. --ignoreConfig: TS 6
# refuses file lists when a tsconfig.json exists (we want ONLY these flags).
npx tsc src/viewer/layout.ts src/viewer/layout.test.ts \
  --outDir "$OUT" --module nodenext --moduleResolution nodenext \
  --target es2022 --strict --noUnusedLocals --skipLibCheck --ignoreConfig || {
  echo "gate14: FAILED — tsc errors above"; exit 1;
}
node "$OUT/layout.test.js" || { echo "gate14: FAILED — assertions above"; exit 1; }
echo "gate14: PASSED — layout core (layoutBlocks + hitTest) coordinate tests green"
