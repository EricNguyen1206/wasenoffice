#!/bin/bash
# WasenOffice smoke gate 13 — quick-view UX wiring E2E (T-V5, Track A rung A3):
# boots the app NORMALLY (NO WASEN_OFFICE_QUICK_VIEW) with the start-page probe
# driver WASEN_OFFICE_UX_PROBE=<docx> (pattern WASEN_OFFICE_SAVE_PROBE: fills the
# path input and clicks the REAL Quick view button — no synthetic keystrokes,
# no focus taken, rule 6b) and asserts the full UI chain on the log:
#   `viewer view:`  ← viewer_view command (the UI wiring itself)
#   → `viewer first-draw` < QUICK_VIEW_MAX_MS (same knob as gate 12)
#   → `viewer layout:`   (the render happened in the window the UI opened)
#   and no `viewer failed:` anywhere.
# Coordinate/layout/media asserts stay in gate 12 (direct env boot); this gate
# owns the WIRING. LOG CONTRACT: `viewer view:` ← src-tauri/src/viewer.rs
# viewer_view; the viewer lines ← src/viewer.ts main() — frozen constants
# SMOKE_RE_QV_* in harness/scripts/smoke-lib.sh, change in the SAME commit.
#
# Usage: smoke-quick-view-ui.sh
# Env:   SMOKE_QUICK_VIEW_TIMEOUT=240  total seconds before giving up
#        QUICK_VIEW_MAX_MS=500         first-draw budget (ms)
# Negative proof (asserts must bite — the chain must be EXERCISED, not just
# absent; documented in docs/diagnostics.md T-V5):
#   QUICK_VIEW_UI_NEGATIVE=1 harness/scripts/smoke-quick-view-ui.sh
#   → probes a garbage .docx: the chain runs end-to-end but parse fails →
#     `viewer failed:` bites → exit 1 (fast, no timeout wait).
set -u

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
. "$ROOT/harness/scripts/smoke-lib.sh"

TIMEOUT="${SMOKE_QUICK_VIEW_TIMEOUT:-240}"
MAX_MS="${QUICK_VIEW_MAX_MS:-500}"
FIXTURE=/tmp/wasenoffice-quickview-ui.docx
LOG="$(mktemp /tmp/wasenoffice-smoke-quickview-ui-XXXXXX)"

if [ "${QUICK_VIEW_UI_NEGATIVE:-0}" = "1" ]; then
  printf 'not a docx package' > "$FIXTURE"
  echo "smoke: NEGATIVE proof — garbage .docx, expecting 'viewer failed:' to bite"
else
  python3 "$ROOT/harness/fixtures/make_docx_layout.py" "$FIXTURE" >/dev/null || {
    echo "smoke: cannot generate layout fixture"; exit 1;
  }
fi

smoke_init "$FIXTURE"
echo "smoke: fixture=$FIXTURE log=$LOG timeout=${TIMEOUT}s budget=${MAX_MS}ms (UI probe driver)"

smoke_boot "WASEN_OFFICE_UX_PROBE=$FIXTURE" "WASEN_OFFICE_DEVTOOLS=0" "RUST_LOG=info" "$LOG"

quick_view_ui_signals() {
  if grep -q "$SMOKE_RE_QV_FAILED" "$LOG" 2>/dev/null; then
    SMOKE_ABORT=1
    SMOKE_SUMMARY="viewer error page ($(grep -m1 "$SMOKE_RE_QV_FAILED" "$LOG" | cut -c1-120))"
    return 1
  fi
  grep -q "$SMOKE_RE_QV_VIEW" "$LOG" 2>/dev/null || { SMOKE_SUMMARY="no 'viewer view:' yet (UI chain not fired)"; return 1; }
  grep -q "$SMOKE_RE_QV_FIRST_DRAW" "$LOG" 2>/dev/null || { SMOKE_SUMMARY="view command seen, no first-draw yet"; return 1; }
  grep -q "$SMOKE_RE_QV_LAYOUT" "$LOG" 2>/dev/null || { SMOKE_SUMMARY="first-draw seen, layout line not yet"; return 1; }
  local ms
  ms=$(grep -m1 -Eo "$SMOKE_RE_QV_FIRST_DRAW [0-9.]+ ms" "$LOG" | grep -Eo '[0-9.]+')
  if python3 -c "import sys; sys.exit(0 if float('${ms:-0}') < float('$MAX_MS') else 1)"; then
    SMOKE_SUMMARY="UI chain OK: view → first-draw ${ms} ms (< ${MAX_MS}) + layout"
    return 0
  fi
  SMOKE_ABORT=1
  SMOKE_SUMMARY="first-draw ${ms} ms exceeds budget ${MAX_MS} ms"
  return 1
}

if smoke_poll "$TIMEOUT" quick_view_ui_signals "quick-view UI chain evidence"; then
  echo "smoke: ✓ 'viewer view:' (UI wiring) → ✓ first-draw < ${MAX_MS} ms → ✓ layout (window opened by the UI, no env trigger)"
  grep -m1 "$SMOKE_RE_QV_VIEW" "$LOG" | sed 's/^/smoke:   /' | cut -c1-160
  grep -m1 "$SMOKE_RE_QV_FIRST_DRAW" "$LOG" | sed 's/^/smoke:   /' | cut -c1-160
  grep -m1 "$SMOKE_RE_QV_LAYOUT" "$LOG" | sed 's/^/smoke:   /' | cut -c1-200
  echo "smoke: PASSED — full log kept at $LOG"
  exit 0
fi
smoke_fail_tail "$LOG"
exit 1
