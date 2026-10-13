#!/bin/bash
# WasenOffice smoke gate — behavioral verification of the render chain (TC-D01-06).
#
# Boots the real app (pnpm tauri dev) with a generated DOCX fixture and waits,
# on the ACCUMULATING stdout log, for the three GREEN signals of TC-D01-06:
#   1. `asc_onDocumentContentReady` fires            (hook + log line in src/shim/delivery.js,
#                                                    contentReady attachEvent)
#   2. `id_viewer` height > 0                        (`viewer=WxH` in the font-oracle line, or
#                                                    `canvas probe …WxH` — both src/shim/delivery.js,
#                                                    probe-gated, default ON in debug builds)
#   3. font bytes fetched: `GET /fonts/`             (Rust side: src-tauri/src/protocol/filesrv.rs:60)
# LOG CONTRACT frozen (T-045): the emitting lines and the grep patterns must
# change in the SAME commit — each side carries a pointer comment (see also
# the "LOG CONTRACT — FROZEN" header of src/shim/delivery.js). Since T-053 the
# patterns live as frozen constants in harness/scripts/smoke-lib.sh (the
# boilerplate home of all three smoke gates).
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
. "$ROOT/harness/scripts/smoke-lib.sh"

FIXTURE="${1:-/tmp/wasenoffice-smoke-fixture.docx}"
TIMEOUT="${SMOKE_TIMEOUT:-240}"
LOG="$(mktemp /tmp/wasenoffice-smoke-XXXXXX)"

smoke_init "$FIXTURE"
echo "smoke: fixture=$FIXTURE log=$LOG timeout=${TIMEOUT}s"

smoke_boot "WASEN_OFFICE_OPEN=$FIXTURE" "WASEN_OFFICE_DEVTOOLS=0" "RUST_LOG=info" "$LOG"

render_signals() {
  local s1 s2 s3
  s1=$(grep -c "$SMOKE_RE_CONTENT_READY" "$LOG" 2>/dev/null); s1=${s1:-0}
  s2=$(grep -Ec "$SMOKE_RE_VIEWER" "$LOG" 2>/dev/null); s2=${s2:-0}
  s3=$(grep -c "$SMOKE_RE_FONT_FETCH" "$LOG" 2>/dev/null); s3=${s3:-0}
  SMOKE_SUMMARY="signals contentReady=$s1 viewer>0=$s2 fonts=$s3"
  [ "$s1" -ge 1 ] && [ "$s2" -ge 1 ] && [ "$s3" -ge 1 ]
}

if smoke_poll "$TIMEOUT" render_signals "all signals"; then
  echo "smoke: ✓ asc_onDocumentContentReady · ✓ id_viewer height>0 · ✓ /fonts/ fetch"
  grep -m1 "$SMOKE_RE_CONTENT_READY" "$LOG" | sed 's/^/smoke:   /' | cut -c1-140
  grep -Em1 "$SMOKE_RE_VIEWER" "$LOG" | sed 's/^/smoke:   /' | cut -c1-160
  grep -m1 "$SMOKE_RE_FONT_FETCH" "$LOG" | sed 's/^/smoke:   /' | cut -c1-140
  echo "smoke: PASSED — full log kept at $LOG"
  exit 0
fi
smoke_fail_tail "$LOG"
exit 1
