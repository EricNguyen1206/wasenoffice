#!/bin/bash
# WasenOffice smoke gate 20 — keymap native apply + reveal (T-A41, DD K02 §7).
#
# Two arms, both boot the REAL app with WASEN_OFFICE_KM_PROBE=1 and a
# WASEN_OFFICE_KEYMAP_FILE store override (gate never touches user data):
#   Arm 1 (positive, strikeout reassigned to ⌘⇧X):
#     - `km-apply: engine=8 hinted=7 missing=selectall`
#       (8 engine chords through asc_applyAscShortcuts; selectall has no
#       toolbar button — by design; strikeout override applied + hinted)
#     - `km-probe: editor=object unlocked=46 btn-ids=7/7 reveal=ok`
#       (window.editor reachable; reveal self-test ok)
#   Arm 2 (fail-soft negative, garbage store):
#     - `km-apply: engine=7` (strikeout default-empty → not applied; store
#       garbage ignored → defaults, app unharmed — DD K02 §7.5)
# LOG CONTRACT frozen: emitting lines in src/shim/desktopEditor.js, patterns
# in harness/scripts/smoke-lib.sh — change in the SAME commit.
set -u

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
. "$ROOT/harness/scripts/smoke-lib.sh"

FIXTURE="/tmp/wasenoffice-smoke-fixture.docx"
TIMEOUT="${SMOKE_TIMEOUT:-240}"
LOG="$(mktemp /tmp/wasenoffice-smoke-km-XXXXXX)"
KMSTORE="$(mktemp /tmp/wasenoffice-km-XXXXXX).json"

smoke_init "$FIXTURE"
echo "smoke: fixture=$FIXTURE store=$KMSTORE timeout=${TIMEOUT}s"

# Arm 1 — positive: strikeout reassigned ⌘⇧X (override shape = Chord JSON).
printf '{"strikeout":{"cmd":true,"ctrl":false,"shift":true,"alt":false,"key":"x"}}' > "$KMSTORE"

smoke_boot "WASEN_OFFICE_OPEN=$FIXTURE" "WASEN_OFFICE_KM_PROBE=1" \
  "WASEN_OFFICE_COMPACT_CHROME=1" \
  "WASEN_OFFICE_KEYMAP_FILE=$KMSTORE" "WASEN_OFFICE_DEVTOOLS=0" "RUST_LOG=info" "$LOG"

km_positive() {
  local a p
  a=$(grep -c "$SMOKE_RE_KM_APPLY" "$LOG" 2>/dev/null); a=${a:-0}
  p=$(grep -c 'reveal=ok' "$LOG" 2>/dev/null); p=${p:-0}
  SMOKE_SUMMARY="km-apply=$a reveal-ok=$p"
  [ "$a" -ge 1 ] && [ "$p" -ge 1 ] &&
    grep -q 'km-apply: engine=8 hinted=7 missing=selectall' "$LOG" &&
    grep -q 'km-probe: editor=object unlocked=46 btn-ids=7/7 reveal=ok' "$LOG"
}

if smoke_poll "$TIMEOUT" km_positive "keymap positive signals"; then
  echo "smoke: ✓ native apply engine=8 (strikeout=⌘⇧X) · ✓ reveal self-test ok"
  grep -m1 "$SMOKE_RE_KM_APPLY" "$LOG" | sed 's/^/smoke:   /' | cut -c1-140
  grep -m1 "$SMOKE_RE_KM_PROBE" "$LOG" | sed 's/^/smoke:   /' | cut -c1-140
  smoke_cleanup
else
  smoke_fail_tail "$LOG"
  exit 1
fi

# Arm 2 — fail-soft: garbage store must fall back to defaults (no crash,
# strikeout stays empty → 7 engine chords, not 8).
printf '{not json at all' > "$KMSTORE"
LOG2="$(mktemp /tmp/wasenoffice-smoke-km2-XXXXXX)"
smoke_boot "WASEN_OFFICE_OPEN=$FIXTURE" "WASEN_OFFICE_KM_PROBE=1" \
  "WASEN_OFFICE_COMPACT_CHROME=1" \
  "WASEN_OFFICE_KEYMAP_FILE=$KMSTORE" "WASEN_OFFICE_DEVTOOLS=0" "RUST_LOG=info" "$LOG2"

km_negative() {
  grep -q 'km-apply: engine=7 ' "$LOG2" 2>/dev/null
}

if smoke_poll "$TIMEOUT" km_negative "keymap fail-soft signal"; then
  echo "smoke: ✓ garbage store → defaults (engine=7, strikeout empty)"
  grep -m1 "$SMOKE_RE_KM_APPLY" "$LOG2" | sed 's/^/smoke:   /' | cut -c1-140
  echo "smoke: PASSED (arm 1 positive · arm 2 fail-soft) — full logs: $LOG $LOG2"
  exit 0
fi
smoke_fail_tail "$LOG2"
exit 1
