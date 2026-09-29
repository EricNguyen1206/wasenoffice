#!/bin/bash
# NanoOffice smoke gate — behavioral verification of the render chain (TC-D01-06).
#
# Boots the real app (pnpm tauri dev) with a generated DOCX fixture and waits,
# on the ACCUMULATING stdout log, for the three GREEN signals of TC-D01-06:
#   1. `asc_onDocumentContentReady` fires            (hook + log line in src/shim/delivery.js,
#                                                    contentReady attachEvent)
#   2. `id_viewer` height > 0                        (`viewer=WxH` in the font-oracle line, or
#                                                    `canvas probe …WxH` — both src/shim/delivery.js,
#                                                    probe-gated, default ON in debug builds)
#   3. font bytes fetched: `GET /fonts/…`            (Rust side: src-tauri/src/protocol/filesrv.rs:60)
# LOG CONTRACT frozen (T-045): the emitting lines and the grep patterns below
# must change in the SAME commit — each side carries a pointer comment (see
# also the "LOG CONTRACT — FROZEN" header of src/shim/delivery.js).
# All three within the timeout → exit 0; anything else → exit 1 with log tail.
#
# This is verify.sh gate 9 — the first behavioral gate. It exists because the
# 27.09 render blocker was invisible to every compile/lint gate (see
# docs/diagnostics.md 27.09 + docs/design/D01-editor/TEST.md TC-D01-06).
#
# Usage: smoke-render.sh [fixture]   (default: DOCX via make_docx.py; for
#        xlsx/pptx/pdf pass the fixture path — the matching make_<ext>.py
#        generates it when the file does not exist. T-048.)
# Env:   SMOKE_TIMEOUT=240  total seconds before giving up (first cold build
#        needs more; warm runs settle in ~30–60s).
set -u

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

FIXTURE="${1:-/tmp/nanooffice-smoke-fixture.docx}"
TIMEOUT="${SMOKE_TIMEOUT:-240}"
LOG="$(mktemp /tmp/nanooffice-smoke-XXXXXX)"

command -v pnpm >/dev/null 2>&1 || { echo "smoke: pnpm not found"; exit 1; }
if lsof -nP -iTCP:1420 -sTCP:LISTEN >/dev/null 2>&1; then
  echo "smoke: port 1420 đã có listener (pnpm/tauri dev khác đang chạy?) — tắt rồi chạy lại"
  exit 1
fi

if [ ! -f "$FIXTURE" ]; then
  EXT="${FIXTURE##*.}"
  GEN="$ROOT/harness/fixtures/make_${EXT}.py"
  [ -f "$GEN" ] || { echo "smoke: no fixture generator for .${EXT}"; exit 1; }
  python3 "$GEN" "$FIXTURE" || { echo "smoke: cannot generate fixture"; exit 1; }
fi

echo "smoke: fixture=$FIXTURE log=$LOG timeout=${TIMEOUT}s"

# Job control: the background job gets its own process group, so the EXIT trap
# can tear down the whole tree (pnpm → tauri-cli → cargo → app) without
# touching unrelated processes.
set -m
NANO_OFFICE_OPEN="$FIXTURE" NANO_OFFICE_DEVTOOLS=0 RUST_LOG=info \
  pnpm tauri dev >"$LOG" 2>&1 &
APP_PID=$!
cleanup() { disown "$APP_PID" 2>/dev/null; kill -- "-$APP_PID" >/dev/null 2>&1; sleep 1; kill -9 -- "-$APP_PID" >/dev/null 2>&1; }
trap cleanup EXIT

deadline=$((SECONDS + TIMEOUT))
while :; do
  s1=$(grep -c 'asc_onDocumentContentReady' "$LOG" 2>/dev/null); s1=${s1:-0}
  s2=$(grep -Ec 'viewer=[0-9]+x[1-9][0-9]*|canvas probe.*[0-9]+x[1-9][0-9]*' "$LOG" 2>/dev/null); s2=${s2:-0}
  s3=$(grep -c 'GET /fonts/' "$LOG" 2>/dev/null); s3=${s3:-0}
  if [ "$s1" -ge 1 ] && [ "$s2" -ge 1 ] && [ "$s3" -ge 1 ]; then
    echo "smoke: ✓ asc_onDocumentContentReady · ✓ id_viewer height>0 · ✓ /fonts/ fetch"
    grep -m1 'asc_onDocumentContentReady' "$LOG" | sed 's/^/smoke:   /' | cut -c1-140
    grep -Em1 'viewer=[0-9]+x[1-9][0-9]*|canvas probe.*[0-9]+x[1-9][0-9]*' "$LOG" | sed 's/^/smoke:   /' | cut -c1-160
    grep -m1 'GET /fonts/' "$LOG" | sed 's/^/smoke:   /' | cut -c1-140
    echo "smoke: PASSED — full log kept at $LOG"
    exit 0
  fi
  if ! kill -0 "$APP_PID" 2>/dev/null; then echo "smoke: app process exited before all signals appeared"; break; fi
  if [ "$SECONDS" -ge "$deadline" ]; then echo "smoke: timed out after ${TIMEOUT}s"; break; fi
  sleep 2
done

echo "smoke: FAILED — signals contentReady=$s1 viewer>0=$s2 fonts=$s3"
echo "smoke: log tail (last 40 lines of $LOG):"
tail -40 "$LOG"
exit 1
