#!/bin/bash
# NanoOffice smoke gate — behavioral verification of the SAVE round-trip
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
# LOG CONTRACT (gate 10): the two lines above are grep targets of this script;
# the emitting ends carry pointer comments — change in the SAME commit or the
# gate breaks silently (same discipline as the frozen gate-9 contract in
# src/shim/delivery.js).
#
# This is verify.sh gate 10. Marker: env NANO_OFFICE_SAVE_PROBE IS the probe
# text (src-tauri/src/lib.rs injects it verbatim as
# __NANO_OFFICE_SAVE_PROBE_TEXT__), so the gate randomizes it per run — runs
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

TIMEOUT="${SMOKE_SAVE_TIMEOUT:-240}"
LOG="$(mktemp /tmp/nanooffice-smoke-save-XXXXXX)"
WORK="$(mktemp -d /tmp/nanooffice-smoke-save-XXXXXX)"
FIXTURE="$WORK/t011.docx"
MARKER="T011-$RANDOM-SAVED-BY-NANOOFFICE"
PROBE_TEXT="$MARKER"
if [ "${SMOKE_SAVE_NO_PROBE:-0}" = "1" ]; then PROBE_TEXT=""; fi

command -v pnpm >/dev/null 2>&1 || { echo "smoke: pnpm not found"; exit 1; }
if lsof -nP -iTCP:1420 -sTCP:LISTEN >/dev/null 2>&1; then
  echo "smoke: port 1420 đã có listener (pnpm/tauri dev khác đang chạy?) — tắt rồi chạy lại"
  exit 1
fi

python3 "$ROOT/harness/fixtures/make_docx.py" "$FIXTURE" || { echo "smoke: cannot generate fixture"; exit 1; }
PRE_SIZE=$(stat -f '%z' "$FIXTURE")
PRE_MTIME=$(stat -f '%m' "$FIXTURE")

echo "smoke: fixture=$FIXTURE log=$LOG timeout=${TIMEOUT}s"
echo "smoke: marker=$MARKER"
if [ -z "$PROBE_TEXT" ]; then echo "smoke: COUNTERFACTUAL — probe disabled, this run MUST FAIL"; fi

# Job control: the background job gets its own process group, so the EXIT trap
# can tear down the whole tree (pnpm → tauri-cli → cargo → app) without
# touching unrelated processes. Same teardown as smoke-render.sh (gate 9).
set -m
NANO_OFFICE_OPEN="$FIXTURE" NANO_OFFICE_SAVE_PROBE="$PROBE_TEXT" NANO_OFFICE_DEVTOOLS=0 RUST_LOG=info \
  pnpm tauri dev >"$LOG" 2>&1 &
APP_PID=$!
cleanup() { disown "$APP_PID" 2>/dev/null; kill -- "-$APP_PID" >/dev/null 2>&1; sleep 1; kill -9 -- "-$APP_PID" >/dev/null 2>&1; }
trap cleanup EXIT

deadline=$((SECONDS + TIMEOUT))
while :; do
  s1=$(grep -Ec 'save-end delivered to engine|save delivered:' "$LOG" 2>/dev/null); s1=${s1:-0}
  s2=$(unzip -p "$FIXTURE" word/document.xml 2>/dev/null | grep -cF -- "$MARKER"); s2=${s2:-0}
  POST_SIZE=$(stat -f '%z' "$FIXTURE" 2>/dev/null); POST_SIZE=${POST_SIZE:-0}
  POST_MTIME=$(stat -f '%m' "$FIXTURE" 2>/dev/null); POST_MTIME=${POST_MTIME:-0}
  if [ "$POST_SIZE" != "$PRE_SIZE" ] && [ "$POST_MTIME" != "$PRE_MTIME" ]; then s3=1; else s3=0; fi
  if [ "$s1" -ge 1 ] && [ "$s2" -ge 1 ] && [ "$s3" -eq 1 ]; then
    echo "smoke: ✓ save round-trip log · ✓ file mutated on disk ($PRE_SIZE → $POST_SIZE B, mtime $PRE_MTIME → $POST_MTIME) · ✓ marker in word/document.xml ×$s2"
    grep -m1 -E 'save-end delivered to engine|save delivered:' "$LOG" | sed 's/^/smoke:   /' | cut -c1-140
    grep -m1 'save probe: asc_Save() called' "$LOG" | sed 's/^/smoke:   /' | cut -c1-140
    echo "smoke: PASSED — full log kept at $LOG (mutated fixture kept at $FIXTURE)"
    exit 0
  fi
  if ! kill -0 "$APP_PID" 2>/dev/null; then echo "smoke: app process exited before save evidence appeared"; break; fi
  if [ "$SECONDS" -ge "$deadline" ]; then echo "smoke: timed out after ${TIMEOUT}s"; break; fi
  sleep 2
done

echo "smoke: FAILED — save-log=$s1 marker-grep=$s2 disk-mutated=$s3"
echo "smoke: log tail (last 40 lines of $LOG):"
tail -40 "$LOG"
exit 1
