#!/bin/bash
# NanoOffice smoke gate 11 — "clean ONLYOFFICE" (T-A13, Track A rung A1):
# the save round-trip of gate 10, booted in CLEAN-MACHINE MODE, asserting the
# log carries ZERO references to the installed ONLYOFFICE.app.
#
# Boots the real app (pnpm tauri dev) with NANO_OFFICE_NO_APP_FALLBACK=1 —
# src-tauri/src/sidecar/mod.rs::x2t_path() then resolves the bundled
# converter or fails loudly, never silently falling back to the installed
# app — plus the same auto-probe as gate 10 (open fixture → asc_AddText +
# asc_Save → x2t converts Editor.bin + changes0.json back to the ORIGINAL).
# Asserts ON DISK AND IN THE LOG:
#   1. save delivered (same evidence as gate 10: save log line + marker in
#      word/document.xml + fixture mutated)
#   2. grep -c "/Applications/ONLYOFFICE" over the WHOLE log == 0
#   3. no "falling back to the installed ONLYOFFICE" WARN line
#   4. no "clean-machine mode" error line (strict mode must not trip)
# A run where everything "works" but references the app STILL FAILS — that
# is the point (see the negative proof in docs/diagnostics.md, T-A13:
# SMOKE_CLEAN_FORCE_APP_X2T=1 → exit 1 with #2 > 0).
#
# This is verify.sh gate 11. LOG CONTRACT: line 1 reuses the frozen gate-10
# targets; #2–#4 are the emitting ends of sidecar/mod.rs (override log /
# fallback WARN / strict error) — change in the SAME commit or the gate
# breaks silently. Since T-053 the patterns + boilerplate live in
# harness/scripts/smoke-lib.sh.
#
# Usage: smoke-clean-machine.sh
# Env:   SMOKE_CLEAN_TIMEOUT=240        total seconds before giving up
#        SMOKE_CLEAN_FORCE_APP_X2T=1    negative-proof mode: point
#                                       NANO_OFFICE_X2T at the installed
#                                       app's converter — the gate MUST
#                                       FAIL (caught by assertion #2)
set -u

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
. "$ROOT/harness/scripts/smoke-lib.sh"

TIMEOUT="${SMOKE_CLEAN_TIMEOUT:-240}"
LOG="$(mktemp /tmp/nanooffice-smoke-clean-XXXXXX)"
WORK="$(mktemp -d /tmp/nanooffice-smoke-clean-XXXXXX)"
FIXTURE="$WORK/ta13.docx"
MARKER="TA13-$RANDOM-SAVED-BY-NANOOFFICE"
APP_X2T="/Applications/ONLYOFFICE.app/Contents/Resources/converter/x2t"

if [ "${SMOKE_CLEAN_FORCE_APP_X2T:-0}" = "1" ]; then
  if [ ! -f "$APP_X2T" ]; then
    echo "smoke: negative-proof mode cần app cài sẵn ($APP_X2T không tồn tại)"; exit 1
  fi
  echo "smoke: NEGATIVE PROOF — NANO_OFFICE_X2T trỏ vào app cài sẵn, gate này PHẢI FAIL"
  FORCE_X2T="$APP_X2T"
else
  FORCE_X2T=""
fi

smoke_init "$FIXTURE"
echo "smoke: fixture=$FIXTURE log=$LOG timeout=${TIMEOUT}s"
echo "smoke: marker=$MARKER"

BOOT_ENVS=("NANO_OFFICE_OPEN=$FIXTURE" "NANO_OFFICE_SAVE_PROBE=$MARKER" \
  "NANO_OFFICE_NO_APP_FALLBACK=1" "NANO_OFFICE_DEVTOOLS=0" "RUST_LOG=info")
[ -n "$FORCE_X2T" ] && BOOT_ENVS+=("NANO_OFFICE_X2T=$FORCE_X2T")
smoke_boot "${BOOT_ENVS[@]}" "$LOG"

clean_signals() {
  smoke_save_evidence "$FIXTURE" "$MARKER" word/document.xml
  local s4 s5 s6
  s4=$(grep -cF "$SMOKE_APP_PATH" "$LOG" 2>/dev/null); s4=${s4:-0}
  s5=$(grep -c "$SMOKE_RE_FALLBACK_WARN" "$LOG" 2>/dev/null); s5=${s5:-0}
  s6=$(grep -c "$SMOKE_RE_STRICT_ERROR" "$LOG" 2>/dev/null); s6=${s6:-0}
  SMOKE_SUMMARY="save-log=$SMOKE_S1 marker-grep=$SMOKE_S2 disk-mutated=$SMOKE_S3 app-path-hits=$s4 fallback-warn=$s5 strict-error=$s6"
  # app path reference = hard violation — fail fast (negative proof lands here)
  if [ "$s4" -gt 0 ]; then
    SMOKE_APP_HITS=$s4
    SMOKE_ABORT=1
    return 1
  fi
  [ "$SMOKE_S1" -ge 1 ] && [ "$SMOKE_S2" -ge 1 ] && [ "$SMOKE_S3" -eq 1 ] \
    && [ "$s4" -eq 0 ] && [ "$s5" -eq 0 ] && [ "$s6" -eq 0 ]
}

if smoke_poll "$TIMEOUT" clean_signals "clean-machine evidence"; then
  echo "smoke: ✓ save round-trip ($SMOKE_PRE_SIZE → $SMOKE_POST_SIZE B, marker ×$SMOKE_S2) · ✓ 0 path /Applications/ONLYOFFICE trong log · ✓ 0 WARN fallback · ✓ strict mode không trip"
  echo "smoke: PASSED — full log kept at $LOG"
  exit 0
fi
smoke_fail_tail "$LOG"
exit 1
