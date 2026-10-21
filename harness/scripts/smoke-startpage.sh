#!/bin/bash
# WasenOffice smoke gate 17 — startpage Welcome wiring E2E (A01 T-A38/T-A39):
# boots the app NORMALLY (no startup doc, no quick-view/nano trigger) with a
# seeded recents store (WASEN_OFFICE_RECENTS_FILE) and the start-page probe
# driver WASEN_OFFICE_STARTPAGE_PROBE (pattern WASEN_OFFICE_UX_PROBE: clicks a
# REAL Welcome control — the genuine handler chain runs; rule 6b), asserting
# the full UI chain on the log.
#
# Modes:
#   (default)  probe `recent:2` — seeded store (1 missing + 1 real fixture):
#     `startpage ready (welcome): recents=2 missing=1 lang=`  ← renderRecents
#     → `open_document: <fixture>`                            ← the command fired
#     → `editor booting (word):`                              ← the engine mounted
#   STARTPAGE_CREATE=1  probe `create:docx` — Create-new (T-A39):
#     `startpage ready (welcome): recents=0 missing=0`        ← empty store
#     → `create_document: docx → …/Untitled 1.docx`           ← template copied
#     → `editor booting (word): Untitled 1.docx`              ← engine mounted
#   STARTPAGE_SAVEAS=1   full Q8 seam — create → SAVE_PROBE types+asc_Save →
#     the untitled intercept routes the FIRST save through the stub panel
#     (WASEN_OFFICE_SAVEAS_STUB — NSSavePanel is not scriptable):
#     `save: untitled document saved as <stub>` + `save delivered: <stub>`
#     + ON DISK: stub file carries the marker, the untitled copy is GONE,
#     the recents store now lists the stub path.
#   and no `[web] ERROR:` anywhere in any mode.
# Coordinate/render asserts stay in gates 9/12; this gate owns the WELCOME
# wiring (recents store → row click → open; template → register → mount).
# LOG CONTRACT: ready/not-found lines ← src/main.ts
# renderRecents/activateRecent, create_document ← src-tauri/src/untitled.rs —
# frozen constants SMOKE_RE_SP_* in harness/scripts/smoke-lib.sh, change in
# the SAME commit. The native Open/Save dialogs are NOT automatable
# (NSOpenPanel/NSSavePanel): their handlers are the same chains these probes
# exercise; panel interaction stays a manual ceiling (TEST A01 TC-A01-05/08).
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

# 360s: a cold Rust relink after edits eats ~4 min (measured 20.10 — the app
# booted at t=240s+0.2 and the evidence line missed the deadline by a hair;
# same rationale as gate 15's 360 default).
TIMEOUT="${SMOKE_STARTPAGE_TIMEOUT:-360}"
FIXTURE=/tmp/wasenoffice-startpage.docx
# BSD mktemp: X's MUST trail the template (bắt được 17.10: template đuôi
# .json → lần 2 fail "File exists" → var rỗng → gate lặng lẽ đọc/ghi store
# THẬT của user). Guard cứng: mktemp fail = exit, không bao giờ fallback.
RECENTS_JSON="$(mktemp /tmp/wasenoffice-recents-gate-XXXXXX)" || { echo "smoke: mktemp recents failed"; exit 1; }
LOG="$(mktemp /tmp/wasenoffice-smoke-startpage-XXXXXX)" || { echo "smoke: mktemp log failed"; exit 1; }

if [ "${STARTPAGE_NEGATIVE:-0}" = "1" ]; then
  echo "smoke: NEGATIVE proof — missing recents row, expecting 'file not found' to bite"
  PROBE_ROW="recent:1"
  MODE="neg"
elif [ "${STARTPAGE_CREATE:-0}" = "1" ]; then
  echo "smoke: CREATE — probe clicks the real Document button (template → register → engine)"
  PROBE_ROW="create:docx"
  MODE="create"
  UNTITLED_DIR="$(mktemp -d /tmp/wasenoffice-untitled-gate-XXXXXX)" || { echo "smoke: mktemp untitled failed"; exit 1; }
elif [ "${STARTPAGE_SAVEAS:-0}" = "1" ]; then
  echo "smoke: SAVE-AS — create → probe types+asc_Save → first save rerouted through the stub panel"
  PROBE_ROW="create:docx"
  MODE="saveas"
  UNTITLED_DIR="$(mktemp -d /tmp/wasenoffice-untitled-gate-XXXXXX)" || { echo "smoke: mktemp untitled failed"; exit 1; }
  TARGET="/tmp/wasenoffice-saveas-target-$$.docx"
  MARKER="T-SAVEAS-$RANDOM-SAVED-BY-WASENOFFICE"
  echo "smoke: stub target=$TARGET marker=$MARKER"
elif [ "${STARTPAGE_SETTINGS:-0}" = "1" ]; then
  echo "smoke: SETTINGS — probe opens the in-page settings view (T-A46: keymap rows + MCP status)"
  PROBE_ROW="settings"
  MODE="settings"
else
  echo "smoke: positive — real recents row #2 must open through the genuine chain"
  PROBE_ROW="recent:2"
  MODE="pos"
fi

python3 "$ROOT/harness/fixtures/make_docx_layout.py" "$FIXTURE" >/dev/null || {
  echo "smoke: cannot generate layout fixture"; exit 1;
}
if [ "$MODE" = "create" ] || [ "$MODE" = "saveas" ]; then
  printf '[]' > "$RECENTS_JSON"
else
  GHOST="/tmp/wasenoffice-startpage-missing-$$.docx"
  NOW_MS=$(python3 -c 'import time; print(int(time.time()*1000))')
  # Store order = newest first (recents.record inserts at front) — row #1 is
  # the ghost (missing), row #2 the real fixture.
  cat > "$RECENTS_JSON" <<EOF
[
  {"path": "$GHOST", "name": "missing.docx", "kind": "docx", "openedAt": $NOW_MS},
  {"path": "$FIXTURE", "name": "wasenoffice-startpage.docx", "kind": "docx", "openedAt": $((NOW_MS - 1000))}
]
EOF
fi

smoke_init "$FIXTURE"
echo "smoke: fixture=$FIXTURE log=$LOG timeout=${TIMEOUT}s recents=$RECENTS_JSON probe=$PROBE_ROW mode=$MODE"

# Mode saveas adds the two seam knobs: the stub panel target + the engine
# SAVE_PROBE (types the marker + asc_Save — same driver as gate 10).
if [ "$MODE" = "saveas" ]; then
  smoke_boot "WASEN_OFFICE_RECENTS_FILE=$RECENTS_JSON" "WASEN_OFFICE_STARTPAGE_PROBE=$PROBE_ROW" "WASEN_OFFICE_UNTITLED_DIR=$UNTITLED_DIR" "WASEN_OFFICE_SAVEAS_STUB=$TARGET" "WASEN_OFFICE_SAVE_PROBE=$MARKER" "WASEN_OFFICE_DEVTOOLS=0" "RUST_LOG=info" "$LOG"
else
  smoke_boot "WASEN_OFFICE_RECENTS_FILE=$RECENTS_JSON" "WASEN_OFFICE_STARTPAGE_PROBE=$PROBE_ROW" "WASEN_OFFICE_UNTITLED_DIR=${UNTITLED_DIR:-}" "WASEN_OFFICE_DEVTOOLS=0" "RUST_LOG=info" "$LOG"
fi

startpage_signals() {
  if grep -q "$SMOKE_RE_SP_WEB_ERROR" "$LOG" 2>/dev/null; then
    SMOKE_ABORT=1
    SMOKE_SUMMARY="startpage logged an error ($(grep -m1 "$SMOKE_RE_SP_WEB_ERROR" "$LOG" | cut -c1-140))"
    return 1
  fi
  grep -q "$SMOKE_RE_SP_READY" "$LOG" 2>/dev/null || { SMOKE_SUMMARY="no startpage ready line yet"; return 1; }
  if [ "$MODE" = "create" ]; then
    grep -q 'recents=0 missing=0' "$LOG" 2>/dev/null || { SMOKE_SUMMARY="ready line without recents=0 missing=0"; return 1; }
    grep -q "$SMOKE_RE_SP_CREATE" "$LOG" 2>/dev/null || { SMOKE_SUMMARY="ready seen, create_document not fired yet"; return 1; }
    grep -q "$SMOKE_RE_SP_CREATE_BOOT" "$LOG" 2>/dev/null || { SMOKE_SUMMARY="create seen, Untitled boot line not yet"; return 1; }
    SMOKE_SUMMARY="create-new OK: ready(recents=0) → create_document(Untitled 1.docx) → editor booting(word)"
  elif [ "$MODE" = "saveas" ]; then
    grep -q "$SMOKE_RE_SP_CREATE" "$LOG" 2>/dev/null || { SMOKE_SUMMARY="no create_document yet"; return 1; }
    grep -qF "untitled document saved as $TARGET" "$LOG" 2>/dev/null || { SMOKE_SUMMARY="create seen, untitled save-as not delivered yet"; return 1; }
    grep -qF "save delivered: $TARGET" "$LOG" 2>/dev/null || { SMOKE_SUMMARY="retarget logged, save-delivered line not yet"; return 1; }
    if [ ! -f "$TARGET" ]; then SMOKE_SUMMARY="save delivered logged but stub target missing on disk"; SMOKE_ABORT=1; return 1; fi
    if ! unzip -p "$TARGET" word/document.xml 2>/dev/null | grep -qF "$MARKER"; then SMOKE_SUMMARY="stub target lacks the SAVE_PROBE marker"; SMOKE_ABORT=1; return 1; fi
    if [ -e "$UNTITLED_DIR/Untitled 1.docx" ]; then SMOKE_SUMMARY="untitled copy not removed after save-as"; SMOKE_ABORT=1; return 1; fi
    if ! grep -qF "$TARGET" "$RECENTS_JSON" 2>/dev/null; then SMOKE_SUMMARY="recents store does not list the saved-as path"; SMOKE_ABORT=1; return 1; fi
    SMOKE_SUMMARY="save-as OK: create → first save → stub target (marker on disk) → copy removed → recents updated"
  elif [ "$MODE" = "settings" ]; then
    grep -q "$SMOKE_RE_SP_SETTINGS" "$LOG" 2>/dev/null || { SMOKE_SUMMARY="ready seen, settings view evidence line not yet"; return 1; }
    SMOKE_SUMMARY="settings view OK: probe → sidebar Settings → keymap rows + MCP status"
  else
    grep -q 'recents=2 missing=1' "$LOG" 2>/dev/null || { SMOKE_SUMMARY="ready line without the seeded recents=2 missing=1"; return 1; }
    grep -q "$SMOKE_RE_OPEN_DOC" "$LOG" 2>/dev/null || { SMOKE_SUMMARY="ready seen, open_document not fired yet"; return 1; }
    grep -q "$SMOKE_RE_SP_BOOTING" "$LOG" 2>/dev/null || { SMOKE_SUMMARY="open seen, editor boot line not yet"; return 1; }
    SMOKE_SUMMARY="startpage wiring OK: ready(recents=2 missing=1) → open_document → editor booting"
  fi
  return 0
}

# Positive/Create: poll exits 0 when the full chain shows up. Negative: the UI
# guard logs `[web] ERROR: file not found:` → first predicate rule aborts →
# poll returns 1 → fail_tail + exit 1 = the "no ERROR" assert provably bites.
if smoke_poll "$TIMEOUT" startpage_signals "startpage wiring evidence"; then
  echo "smoke: ✓ ready → ✓ command fired → ✓ engine booting ($MODE)"
  grep -m1 "$SMOKE_RE_SP_READY" "$LOG" | sed 's/^/smoke:   /' | cut -c1-160
  if [ "$MODE" = "create" ]; then
    grep -m1 "$SMOKE_RE_SP_CREATE" "$LOG" | sed 's/^/smoke:   /' | cut -c1-160
    grep -m1 "$SMOKE_RE_SP_CREATE_BOOT" "$LOG" | sed 's/^/smoke:   /' | cut -c1-160
  elif [ "$MODE" = "saveas" ]; then
    grep -m1 -F "untitled document saved as $TARGET" "$LOG" | sed 's/^/smoke:   /' | cut -c1-160
    grep -m1 -F "save delivered: $TARGET" "$LOG" | sed 's/^/smoke:   /' | cut -c1-160
    echo "smoke:   on-disk: stub target carries the marker · untitled copy removed · recents lists $TARGET"
  elif [ "$MODE" = "settings" ]; then
    grep -m1 "$SMOKE_RE_SP_SETTINGS" "$LOG" | sed 's/^/smoke:   /' | cut -c1-160
  else
    grep -m1 "$SMOKE_RE_OPEN_DOC" "$LOG" | sed 's/^/smoke:   /' | cut -c1-160
    grep -m1 "$SMOKE_RE_SP_BOOTING" "$LOG" | sed 's/^/smoke:   /' | cut -c1-160
  fi
  echo "smoke: PASSED — full log kept at $LOG"
  [ -n "${UNTITLED_DIR:-}" ] && rm -rf "$UNTITLED_DIR"
  exit 0
fi
[ -n "${UNTITLED_DIR:-}" ] && rm -rf "$UNTITLED_DIR"
smoke_fail_tail "$LOG"
exit 1
