#!/bin/bash
# dev-check — fast pre-verify: the EXACT commands the gates run, sourced from
# the one home (gate-cmds.sh). NO app boots (~10–15 s). Run before claiming a
# slice done; verify.sh FULL stays the final judge (smoke gates + app boots).
set -u
cd "$(dirname "$0")/../.."
. harness/scripts/gate-cmds.sh
fail=0
run() {
  echo "▸ $1"
  eval "$2" || fail=1
}
run "cargo check (workspace)" "$GATE_CARGO_CHECK"
run "cargo fmt --check --all" "$GATE_FMT"
run "cargo clippy -D warnings" "$GATE_CLIPPY"
run "pnpm build" "$GATE_BUILD"
run "gate 14 — layout core unit tests" "$GATE_LAYOUT"
if [ "$fail" -ne 0 ]; then
  echo "dev-check: FAILED"
  exit 1
fi
echo "dev-check: ALL FAST CHECKS PASSED (smoke gates + app boots remain in verify.sh)"
