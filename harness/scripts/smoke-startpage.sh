#!/bin/bash
# WasenOffice smoke gate 17 — startpage Welcome + recents wiring E2E (A01
# T-A38): boots the app NORMALLY (no startup doc, no quick-view/nano trigger)
# with the recents store seeded via WASEN_OFFICE_RECENTS_FILE (1 missing path
# + 1 real fixture) and the start-page probe driver
# WASEN_OFFICE_STARTPAGE_PROBE=recent:2 (pattern WASEN_OFFICE_UX_PROBE: clicks
# the REAL recents row — the genuine openPath chain runs; rule 6b) and asserts
# the full UI chain on the log:
#   `startpage ready (welcome): recents=2 missing=1 lang=`  ← renderRecents
#   → `open_document: <fixture>`                            ← the command fired
#   → `editor booting (word):`                              ← the engine mounted
#   and no `[web] ERROR:` anywhere.
# Coordinate/render asserts stay in gates 9/12; this gate owns the WELCOME
# wiring (recents store → row click → open). LOG CONTRACT: the ready/not-found
# lines ← src/main.ts renderRecents/activateRecent — frozen constants
# SMOKE_RE_SP_* in harness/scripts/smoke-lib.sh, change in the SAME commit.
# The native Open dialog itself is NOT automatable (NSOpenPanel): its handler
# is the same openPath chain the row probe exercises; panel interaction stays
# a manual ceiling (TEST A01 TC-A01-05).
#
# Usage: smoke-startpage.sh
# Env:   SMOKE_STARTPAGE_TIMEOUT=240  total seconds before giving up
# Negative proof (asserts must bite — the chain must be EXERCISED, not just
# absent; pattern documented in docs/diagnostics.md T-V5):
#   STARTPAGE_NEGATIVE=1 harness/scripts/smoke-startpage.sh
#   → probes the MISSING recents row: the UI guard catches it →
#     `[web] ERROR: file not found:` bites → exit 1 (fast, no timeout wait).
set -u

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
. "$ROOT/harness/scripts/smoke-lib.sh"

TIMEOUT="${SMOKE_STARTPAGE_TIMEOUT:-240}"
FIXTURE=/tmp/wasenoffice-startpage.docx
# BSD mktemp: X's MUST trail the template (bắt được 17.10: template đuôi
# .json → lần 2 fail "File exists" → var rỗng → gate lặng lẽ đọc/ghi store
# THẬT của user). Guard cứng: mktemp fail = exit, không bao giờ fallback.
RECENTS_JSON="$(mktemp /tmp/wasenoffice-recents-gate-XXXXXX)" || { echo "smoke: mktemp recents failed"; exit 1; }
LOG="$(mktemp /tmp/wasenoffice-smoke-startpage-XXXXXX)" || { echo "smoke: mktemp log failed"; exit 1; }

python3 "$ROOT/harness/fixtures/make_docx_layout.py" "$FIXTURE" >/dev/null || {
  echo "smoke: cannot generate layout fixture"; exit 1;
}
GHOST="/tmp/wasenoffice-startpage-missing-$$.docx"
NOW_MS=$(python3 -c 'import time; print(int(time.time()*1000))')
# Store order = newest first (recents.record inserts at front) — row #1 is the
# ghost (missing), row #2 the real fixture.
cat > "$RECENTS_JSON" <<EOF
[
  {"path": "$GHOST", "name": "missing.docx", "kind": "docx", "openedAt": $NOW_MS},
  {"path": "$FIXTURE", "name": "wasenoffice-startpage.docx", "kind": "docx", "openedAt": $((NOW_MS - 1000))}
]
EOF

if [ "${STARTPAGE_NEGATIVE:-0}" = "1" ]; then
  echo "smoke: NEGATIVE proof — missing recents row, expecting 'file not found' to bite"
  PROBE_ROW="recent:1"
else
  echo "smoke: positive — real recents row #2 must open through the genuine chain"
  PROBE_ROW="recent:2"
fi

smoke_init "$FIXTURE"
echo "smoke: fixture=$FIXTURE log=$LOG timeout=${TIMEOUT}s recents=$RECENTS_JSON probe=$PROBE_ROW"

smoke_boot "WASEN_OFFICE_RECENTS_FILE=$RECENTS_JSON" "WASEN_OFFICE_STARTPAGE_PROBE=$PROBE_ROW" "WASEN_OFFICE_DEVTOOLS=0" "RUST_LOG=info" "$LOG"

startpage_signals() {
  if grep -q "$SMOKE_RE_SP_WEB_ERROR" "$LOG" 2>/dev/null; then
    SMOKE_ABORT=1
    SMOKE_SUMMARY="startpage logged an error ($(grep -m1 "$SMOKE_RE_SP_WEB_ERROR" "$LOG" | cut -c1-140))"
    return 1
  fi
  grep -q "$SMOKE_RE_SP_READY" "$LOG" 2>/dev/null || { SMOKE_SUMMARY="no startpage ready line yet"; return 1; }
  grep -q 'recents=2 missing=1' "$LOG" 2>/dev/null || { SMOKE_SUMMARY="ready line without the seeded recents=2 missing=1"; return 1; }
  grep -q "$SMOKE_RE_OPEN_DOC" "$LOG" 2>/dev/null || { SMOKE_SUMMARY="ready seen, open_document not fired yet"; return 1; }
  grep -q "$SMOKE_RE_SP_BOOTING" "$LOG" 2>/dev/null || { SMOKE_SUMMARY="open seen, editor boot line not yet"; return 1; }
  SMOKE_SUMMARY="startpage wiring OK: ready(recents=2 missing=1) → open_document → editor booting"
  return 0
}

# Positive: poll exits 0 when the full chain shows up. Negative: the UI guard
# logs `[web] ERROR: file not found:` → first predicate rule aborts → poll
# returns 1 → fail_tail + exit 1 = the "no ERROR" assert provably bites.
if smoke_poll "$TIMEOUT" startpage_signals "startpage wiring evidence"; then
  echo "smoke: ✓ ready(recents=2 missing=1) → ✓ open_document (recents row click) → ✓ editor booting"
  grep -m1 "$SMOKE_RE_SP_READY" "$LOG" | sed 's/^/smoke:   /' | cut -c1-160
  grep -m1 "$SMOKE_RE_OPEN_DOC" "$LOG" | sed 's/^/smoke:   /' | cut -c1-160
  grep -m1 "$SMOKE_RE_SP_BOOTING" "$LOG" | sed 's/^/smoke:   /' | cut -c1-160
  echo "smoke: PASSED — full log kept at $LOG"
  exit 0
fi
smoke_fail_tail "$LOG"
exit 1
