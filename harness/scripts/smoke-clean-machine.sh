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
# targets; #2–#4 are the emitting ends of save.rs (override log / fallback
# WARN / strict error) — change in the SAME commit or the gate breaks
# silently.
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

TIMEOUT="${SMOKE_CLEAN_TIMEOUT:-240}"
LOG="$(mktemp /tmp/nanooffice-smoke-clean-XXXXXX)"
WORK="$(mktemp -d /tmp/nanooffice-smoke-clean-XXXXXX)"
FIXTURE="$WORK/ta13.docx"
MARKER="TA13-$RANDOM-SAVED-BY-NANOOFFICE"
APP_X2T="/Applications/ONLYOFFICE.app/Contents/Resources/converter/x2t"

command -v pnpm >/dev/null 2>&1 || { echo "smoke: pnpm not found"; exit 1; }
if lsof -nP -iTCP:1420 -sTCP:LISTEN >/dev/null 2>&1; then
  echo "smoke: port 1420 đã có listener (pnpm/tauri dev khác đang chạy?) — tắt rồi chạy lại"
  exit 1
fi

if [ "${SMOKE_CLEAN_FORCE_APP_X2T:-0}" = "1" ]; then
  if [ ! -f "$APP_X2T" ]; then
    echo "smoke: negative-proof mode cần app cài sẵn ($APP_X2T không tồn tại)"; exit 1
  fi
  echo "smoke: NEGATIVE PROOF — NANO_OFFICE_X2T trỏ vào app cài sẵn, gate này PHẢI FAIL"
  FORCE_X2T="$APP_X2T"
else
  FORCE_X2T=""
fi

python3 "$ROOT/harness/fixtures/make_docx.py" "$FIXTURE" || { echo "smoke: cannot generate fixture"; exit 1; }
PRE_SIZE=$(stat -f '%z' "$FIXTURE")
PRE_MTIME=$(stat -f '%m' "$FIXTURE")

echo "smoke: fixture=$FIXTURE log=$LOG timeout=${TIMEOUT}s"
echo "smoke: marker=$MARKER"

# Job control: same teardown discipline as smoke-save.sh (gate 10).
set -m
CMD=(env NANO_OFFICE_OPEN="$FIXTURE" NANO_OFFICE_SAVE_PROBE="$MARKER" \
  NANO_OFFICE_NO_APP_FALLBACK=1 NANO_OFFICE_DEVTOOLS=0 RUST_LOG=info)
[ -n "$FORCE_X2T" ] && CMD+=("NANO_OFFICE_X2T=$FORCE_X2T")
"${CMD[@]}" pnpm tauri dev >"$LOG" 2>&1 &
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
  s4=$(grep -cF '/Applications/ONLYOFFICE' "$LOG" 2>/dev/null); s4=${s4:-0}
  s5=$(grep -c 'falling back to the installed ONLYOFFICE' "$LOG" 2>/dev/null); s5=${s5:-0}
  s6=$(grep -c 'clean-machine mode' "$LOG" 2>/dev/null); s6=${s6:-0}
  # app path reference = hard violation — fail fast (negative proof lands here)
  [ "$s4" -gt 0 ] && break
  if [ "$s1" -ge 1 ] && [ "$s2" -ge 1 ] && [ "$s3" -eq 1 ] \
     && [ "$s4" -eq 0 ] && [ "$s5" -eq 0 ] && [ "$s6" -eq 0 ]; then
    echo "smoke: ✓ save round-trip ($PRE_SIZE → $POST_SIZE B, marker ×$s2) · ✓ 0 path /Applications/ONLYOFFICE trong log · ✓ 0 WARN fallback · ✓ strict mode không trip"
    echo "smoke: PASSED — full log kept at $LOG"
    exit 0
  fi
  if ! kill -0 "$APP_PID" 2>/dev/null; then echo "smoke: app process exited before clean-machine evidence appeared"; break; fi
  if [ "$SECONDS" -ge "$deadline" ]; then echo "smoke: timed out after ${TIMEOUT}s"; break; fi
  sleep 2
done

echo "smoke: FAILED — save-log=$s1 marker-grep=$s2 disk-mutated=$s3 app-path-hits=$s4 fallback-warn=$s5 strict-error=$s6"
if [ "$s4" -gt 0 ]; then
  echo "smoke: log tham chiếu app cài sẵn (${s4} lần):"
  grep -nF '/Applications/ONLYOFFICE' "$LOG" | head -5 | sed 's/^/smoke:   /' | cut -c1-160
fi
echo "smoke: log tail (last 40 lines of $LOG):"
tail -40 "$LOG"
exit 1
