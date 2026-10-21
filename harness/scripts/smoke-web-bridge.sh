#!/bin/bash
# WasenOffice smoke gate 21 — web dev mode HTTP bridge (T-068, DD W01): boots
# the app with WASEN_OFFICE_WEB_BRIDGE=1 (background, rule 6b) and drives the
# command layer exactly like a browser tab would — plain curl, NO Tauri window
# involved on the web side:
#   POST /invoke/recents_list                     → 200 {"ok":true,…}
#   POST /invoke/viewer_view {path} (X-Wasen-Window: web-gate)
#                                                 → {"webOpen":{"url":"/viewer.html?win=quickview-N"}}
#   POST /invoke/viewer_open        (X-Wasen-Window: quickview-N)
#                                                 → result carries mediaBase + parseMs:
#     the label hand-off resolved WITHOUT any native window — the bridge's one idea
#   POST /invoke/open_document                    → ok:false "engine editor is Tauri-only" (ceiling)
#   POST /invoke/plugin%3Aapp%7Cversion           → the app version (getVersion in web mode)
#   unknown command → ok:false · foreign Origin → 403   (both bite in the same boot)
# LOG CONTRACT: `loopback file server on 127.0.0.1:<port>` + `web bridge: listening`
# ← src-tauri/src/protocol/filesrv.rs spawn — frozen SMOKE_RE_FILESRV /
# SMOKE_RE_WEB_BRIDGE in harness/scripts/smoke-lib.sh, change in the SAME commit.
#
# Usage: smoke-web-bridge.sh
# Env:   SMOKE_WEB_BRIDGE_TIMEOUT=240   seconds to wait for the server lines
# Negative proofs (the asserts must bite — documented in docs/diagnostics.md T-068):
#   WEB_BRIDGE_NEGATIVE=disabled harness/scripts/smoke-web-bridge.sh
#     → boots WITHOUT the flag (the release default): the route does not exist,
#       the first POST gets 404 → exit 1
#   WEB_BRIDGE_NEGATIVE=origin harness/scripts/smoke-web-bridge.sh
#     → every POST carries Origin: http://evil.test → 403 → exit 1
set -u

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
. "$ROOT/harness/scripts/smoke-lib.sh"

TIMEOUT="${SMOKE_WEB_BRIDGE_TIMEOUT:-240}"
NEGATIVE="${WEB_BRIDGE_NEGATIVE:-}"
FIXTURE=/tmp/wasenoffice-web-bridge.docx
LOG="$(mktemp /tmp/wasenoffice-smoke-web-bridge-XXXXXX)"
BODY_FILE="$(mktemp /tmp/wasenoffice-smoke-web-bridge-body-XXXXXX)"
ORIGIN="http://127.0.0.1:1420"
[ "$NEGATIVE" = "origin" ] && ORIGIN="http://evil.test"

rm -f "$FIXTURE"
smoke_init "$FIXTURE"
echo "smoke: fixture=$FIXTURE log=$LOG timeout=${TIMEOUT}s mode=${NEGATIVE:-positive} origin=$ORIGIN"

if [ "$NEGATIVE" = "disabled" ]; then
  echo "smoke: NEGATIVE proof — booting WITHOUT WASEN_OFFICE_WEB_BRIDGE: /invoke must not exist"
  smoke_boot "WASEN_OFFICE_DEVTOOLS=0" "RUST_LOG=info" "$LOG"
else
  smoke_boot "WASEN_OFFICE_WEB_BRIDGE=1" "WASEN_OFFICE_DEVTOOLS=0" "RUST_LOG=info" "$LOG"
fi

PORT=""
server_up() {
  PORT=$(grep -m1 -Eo "${SMOKE_RE_FILESRV}[0-9]+" "$LOG" 2>/dev/null | grep -Eo '[0-9]+$')
  [ -n "$PORT" ] || { SMOKE_SUMMARY="loopback file server not up yet"; return 1; }
  if [ "$NEGATIVE" != "disabled" ]; then
    grep -q "$SMOKE_RE_WEB_BRIDGE" "$LOG" 2>/dev/null || { SMOKE_SUMMARY="file server on :$PORT, no 'web bridge: listening' yet"; return 1; }
  fi
  SMOKE_SUMMARY="loopback file server on :$PORT"
  return 0
}
if ! smoke_poll "$TIMEOUT" server_up "loopback server"; then
  smoke_fail_tail "$LOG"
  exit 1
fi
echo "smoke: ✓ $SMOKE_SUMMARY$([ "$NEGATIVE" != "disabled" ] && echo ' + web bridge listening')"

fail() {
  echo "smoke: FAILED — $*"
  echo "smoke: log tail (last 30 lines of $LOG):"
  tail -30 "$LOG"
  exit 1
}

STATUS=""
BODY=""
# post CMD LABEL JSON — one browser-shaped invoke; STATUS + BODY for the asserts.
post() {
  STATUS=$(curl -s -o "$BODY_FILE" -w '%{http_code}' -X POST "http://127.0.0.1:$PORT/invoke/$1" \
    -H "Origin: $ORIGIN" -H "X-Wasen-Window: $2" -H 'Content-Type: application/json' --data "$3")
  BODY=$(cat "$BODY_FILE")
}
# expect REGEX WHAT — the body must match (keys are alphabetical: one key per regex).
expect() {
  printf '%s' "$BODY" | grep -Eq -- "$1" || fail "$2 — status=$STATUS body=$(printf '%s' "$BODY" | cut -c1-200)"
}

post recents_list web-gate '{}'
[ "$STATUS" = "200" ] || fail "recents_list → HTTP $STATUS (404 = bridge off, 403 = origin refused) body=$(cut -c1-120 "$BODY_FILE")"
expect '"ok":true' "recents_list did not succeed"
echo "smoke: ✓ recents_list → 200 ok (origin $ORIGIN accepted)"

post viewer_view web-gate "{\"path\":\"$FIXTURE\"}"
expect '"webOpen":\{"url":"/viewer\.html\?win=quickview-[0-9]+"\}' "viewer_view did not return a webOpen label"
LABEL=$(printf '%s' "$BODY" | grep -Eo 'quickview-[0-9]+' | head -1)
grep -q "$SMOKE_RE_QV_VIEW" "$LOG" || fail "'viewer view:' log line missing after viewer_view"
echo "smoke: ✓ viewer_view → webOpen $LABEL (stashed, no native window opened)"

post viewer_open "$LABEL" '{}'
expect '"mediaBase":"http://127\.0\.0\.1:[0-9]+/media/[0-9]+"' "viewer_open($LABEL) did not resolve the stashed document (no mediaBase)"
expect '"parseMs":[0-9.]+' "viewer_open($LABEL) result has no parseMs"
echo "smoke: ✓ viewer_open($LABEL) → document parsed via label hand-off (the web side owns no window)"

post open_document web-gate "{\"path\":\"$FIXTURE\"}"
expect '"ok":false' "open_document must be refused in web mode"
expect 'engine editor is Tauri-only' "open_document refusal message changed"
echo "smoke: ✓ open_document → refused (engine = Tauri-only ceiling, message intact)"

post 'plugin%3Aapp%7Cversion' web-gate '{}'
expect '"result":"[0-9]+\.[0-9]+\.[0-9]+"' "plugin:app|version did not return a version"
echo "smoke: ✓ plugin:app|version → $(printf '%s' "$BODY" | grep -Eo '[0-9]+\.[0-9]+\.[0-9]+' | head -1)"

post no_such_command web-gate '{}'
expect '"ok":false' "unknown command must be refused"
expect 'unknown command' "unknown-command error text changed"
echo "smoke: ✓ unknown command → ok:false"

STATUS=$(curl -s -o /dev/null -w '%{http_code}' -X POST "http://127.0.0.1:$PORT/invoke/recents_list" \
  -H 'Origin: http://evil.test' -H 'Content-Type: application/json' --data '{}')
[ "$STATUS" = "403" ] || fail "foreign origin got HTTP $STATUS, expected 403"
echo "smoke: ✓ foreign origin → 403"

rm -f "$BODY_FILE"
echo "smoke: PASSED — web bridge on 127.0.0.1:$PORT; full log kept at $LOG"
exit 0
