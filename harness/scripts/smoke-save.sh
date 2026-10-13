#!/bin/bash
# WasenOffice smoke gate — behavioral verification of the SAVE round-trip
# (TC-D01-11, the automation of the manual proof TC-D01-10 — docs/diagnostics.md
# 28.09 T-010): open fixture → shim probe edits via asc_AddText + calls
# asc_Save() (NO keystrokes — the T-003 lesson) → x2t converts Editor.bin +
# changes0.json back to the ORIGINAL file → the gate asserts ON DISK.
#
# Boots the real app (pnpm tauri dev) with a FRESH fixture COPY (the save
# mutates the file by design — never a repo path, never a reused path) and
# waits, on the ACCUMULATING stdout log, for the save-delivered line, then
# asserts on disk:
#   1. save round-trip log: `save-end delivered to engine` (src/shim/delivery.js
#      consumeSaveEnd) or `save delivered:` (Rust side — emitted after the
#      rename over the original, src-tauri/src/bridge/save.rs)
#   2. fixture mtime + size changed since boot
#   3. marker in word/document.xml: `unzip -p <fixture> word/document.xml |
#      grep -c <marker>` ≥ 1 (direct grep suffices — proven 28.09 T-010)
# LOG CONTRACT (gate 10): the lines above are grep targets of this script;
# the emitting ends carry pointer comments — change in the SAME commit or the
# gate breaks silently (same discipline as the frozen gate-9 contract).
# Since T-053 the patterns + boilerplate live in harness/scripts/smoke-lib.sh.
#
# This is verify.sh gate 10. Marker: env WASEN_OFFICE_SAVE_PROBE IS the probe
# text (src-tauri/src/lib.rs injects it verbatim as
# __WASEN_OFFICE_SAVE_PROBE_TEXT__), so the gate randomizes it per run — runs
# stay independent without touching any code.
#
# Usage: smoke-save.sh
# Env:   SMOKE_SAVE_TIMEOUT=240  total seconds before giving up (cold build
#        needs more; warm runs settle in ~40–70s: boot + probe + x2t convert)
#        SMOKE_SAVE_NO_PROBE=1   counterfactual run — probe disabled, the gate
#        MUST FAIL (negative test: proves the gate actually detects saves)
set -u

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
. "$ROOT/harness/scripts/smoke-lib.sh"

TIMEOUT="${SMOKE_SAVE_TIMEOUT:-240}"
LOG="$(mktemp /tmp/wasenoffice-smoke-save-XXXXXX)"
WORK="$(mktemp -d /tmp/wasenoffice-smoke-save-XXXXXX)"
FIXTURE="$WORK/t011.docx"
MARKER="T011-$RANDOM-SAVED-BY-WASENOFFICE"
PROBE_TEXT="$MARKER"
if [ "${SMOKE_SAVE_NO_PROBE:-0}" = "1" ]; then PROBE_TEXT=""; fi

smoke_init "$FIXTURE"
echo "smoke: fixture=$FIXTURE log=$LOG timeout=${TIMEOUT}s"
echo "smoke: marker=$MARKER"
if [ -z "$PROBE_TEXT" ]; then echo "smoke: COUNTERFACTUAL — probe disabled, this run MUST FAIL"; fi

smoke_boot "WASEN_OFFICE_OPEN=$FIXTURE" "WASEN_OFFICE_SAVE_PROBE=$PROBE_TEXT" \
  "WASEN_OFFICE_DEVTOOLS=0" "RUST_LOG=info" "$LOG"

save_signals() {
  smoke_save_evidence "$FIXTURE" "$MARKER" word/document.xml
  SMOKE_SUMMARY="save-log=$SMOKE_S1 marker-grep=$SMOKE_S2 disk-mutated=$SMOKE_S3"
  [ "$SMOKE_S1" -ge 1 ] && [ "$SMOKE_S2" -ge 1 ] && [ "$SMOKE_S3" -eq 1 ]
}

if smoke_poll "$TIMEOUT" save_signals "save evidence"; then
  echo "smoke: ✓ save round-trip log · ✓ file mutated on disk ($SMOKE_PRE_SIZE → $SMOKE_POST_SIZE B, mtime $SMOKE_PRE_MTIME → $SMOKE_POST_MTIME) · ✓ marker in word/document.xml ×$SMOKE_S2"
  grep -m1 -E "$SMOKE_RE_SAVE_DELIVERED" "$LOG" | sed 's/^/smoke:   /' | cut -c1-140
  grep -m1 "$SMOKE_RE_SAVE_PROBE" "$LOG" | sed 's/^/smoke:   /' | cut -c1-140
  echo "smoke: PASSED — full log kept at $LOG (mutated fixture kept at $FIXTURE)"
  exit 0
fi
smoke_fail_tail "$LOG"
exit 1
