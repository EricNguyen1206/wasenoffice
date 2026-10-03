#!/bin/bash
# NanoOffice smoke gate 12 — quick-view metrics (T-V2, Track A rung A3):
# boots the app with NANO_OFFICE_QUICK_VIEW (viewer slice — NO engine, no x2t,
# no shim) against the layout fixture and asserts, on the log:
#   1. `viewer first-draw N ms` with N < 500          (rung-A3 open budget; DD V01 §5)
#   2. `viewer layout:` coordinates behave: every baseline delta equals the
#      line's logged line-height, the line-height equals the OS/2 formula
#      (ascent+|descent|+lineGap)/1000 × px-size recomputed from the LOGGED
#      metrics, lines are uniform (fixture is single font/size), the wrap
#      engaged (lines ≥ 12 > 4 paragraphs) and no line overflows the column
#   3. RSS of the app process + its WebKit XPC helpers (PH-0 method: helpers
#      recognized by PID adjacency) < QUICK_VIEW_MAX_MB
#      — FORMULA = whole WebKit tree, chốt ở DD V01 §10 (OQ-V01-2). The 300 MB
#      default is the DEBUG-build tripwire; the rung target < 150 MB is a
#      RELEASE-build number, measured in DD §5.3 (re-check at PH-4).
# LOG CONTRACT: `viewer first-draw` / `viewer layout:` / `viewer failed:` are
# the emitting ends of src/viewer.ts main() (frozen constants SMOKE_RE_QV_*
# in harness/scripts/smoke-lib.sh) — change in the SAME commit.
# Rule 6b: boot via smoke_boot (background process group, window focused(false)).
#
# Usage: smoke-quick-view.sh
# Env:   SMOKE_QUICK_VIEW_TIMEOUT=240  total seconds before giving up
#        QUICK_VIEW_MAX_MS=500         first-draw budget (ms)
#        QUICK_VIEW_MAX_MB=300         RSS tree budget (MB, debug tripwire)
# Negative proof (asserts must bite — thresholds are gate-internal, so the
# negative run shrinks them; documented in docs/diagnostics.md T-V2):
#   QUICK_VIEW_MAX_MS=1 QUICK_VIEW_MAX_MB=1 harness/scripts/smoke-quick-view.sh
#   → exit 1 with "first-draw ... exceeds budget" (no product code touched).
set -u

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
. "$ROOT/harness/scripts/smoke-lib.sh"

TIMEOUT="${SMOKE_QUICK_VIEW_TIMEOUT:-240}"
MAX_MS="${QUICK_VIEW_MAX_MS:-500}"
MAX_MB="${QUICK_VIEW_MAX_MB:-300}"
FIXTURE=/tmp/nanooffice-quickview-layout.docx
LOG="$(mktemp /tmp/nanooffice-smoke-quickview-XXXXXX)"

python3 "$ROOT/harness/fixtures/make_docx_layout.py" "$FIXTURE" >/dev/null || {
  echo "smoke: cannot generate layout fixture"; exit 1;
}

smoke_init "$FIXTURE"
echo "smoke: fixture=$FIXTURE log=$LOG timeout=${TIMEOUT}s budgets=${MAX_MS}ms/${MAX_MB}MB"

smoke_boot "NANO_OFFICE_QUICK_VIEW=$FIXTURE" "NANO_OFFICE_DEVTOOLS=0" "RUST_LOG=info" "$LOG"

quick_view_signals() {
  if grep -q "$SMOKE_RE_QV_FAILED" "$LOG" 2>/dev/null; then
    SMOKE_ABORT=1
    SMOKE_SUMMARY="viewer error page ($(grep -m1 "$SMOKE_RE_QV_FAILED" "$LOG" | cut -c1-120))"
    return 1
  fi
  grep -q "$SMOKE_RE_QV_FIRST_DRAW" "$LOG" 2>/dev/null || { SMOKE_SUMMARY="no first-draw line yet"; return 1; }
  grep -q "$SMOKE_RE_QV_LAYOUT" "$LOG" 2>/dev/null || { SMOKE_SUMMARY="first-draw seen, layout line not yet"; return 1; }
  SMOKE_QV_REPORT=$(python3 - "$LOG" "$MAX_MS" <<'PY' 2>&1
import re
import sys

text = open(sys.argv[1], errors="replace").read()
max_ms = float(sys.argv[2])
SIZE_HP = 22  # the fixture's only size (half-points, from w:docDefaults)
PT_PX = 96 / 72

def need(cond, msg):
    if not cond:
        print(f"QV FAIL {msg}")
        sys.exit(1)

m = re.search(r"viewer first-draw ([\d.]+) ms", text)
need(m, "no first-draw line")
ms = float(m.group(1))
need(ms < max_ms, f"first-draw {ms} ms exceeds budget {max_ms} ms")

m = re.search(
    r"viewer layout: lines=(\d+) colw=([\d.]+) maxw=([\d.]+) "
    r"metrics=([\d.-]+),([\d.-]+),([\d.-]+) lhs=([\d.,]+) baselines=([\d.,]+)",
    text,
)
need(m, "layout line malformed or metrics=none (bundled fonts missing?)")
lines, colw, maxw = int(m.group(1)), float(m.group(2)), float(m.group(3))
asc, desc, gap = float(m.group(4)), float(m.group(5)), float(m.group(6))
lhs = [float(v) for v in m.group(7).split(",")]
ys = [float(v) for v in m.group(8).split(",")]

need(lines >= 12, f"lines={lines} — wrap did not engage (4 paragraphs in fixture)")
need(maxw <= colw + 0.6, f"maxw {maxw} overflows column {colw}")
expected = (asc - desc + gap) / 1000.0 * (SIZE_HP / 2) * PT_PX
need(abs(lhs[0] - expected) <= 0.6,
     f"line-height {lhs[0]} != OS/2 formula {expected:.2f} (metrics {asc},{desc},{gap})")
need(max(lhs) - min(lhs) <= 0.15, f"line heights not uniform: {lhs}")
need(len(lhs) == len(ys) >= 2, "baselines/lhs length mismatch")
for i in range(len(ys) - 1):
    need(abs((ys[i + 1] - ys[i]) - lhs[i]) <= 0.15,
         f"baseline delta {ys[i + 1] - ys[i]:.1f} != line-height {lhs[i]} at line {i}")
print(f"QV OK first-draw={ms}ms lines={lines} lh={expected:.2f}px(col {colw}px maxw {maxw}px)")
PY
)
  local rc=$?
  SMOKE_SUMMARY="$SMOKE_QV_REPORT"
  [ "$rc" -ne 0 ] && SMOKE_ABORT=1 # asserts bit — stop polling (negative proof lands here)
  return "$rc"
}

if smoke_poll "$TIMEOUT" quick_view_signals "quick-view evidence"; then
  sleep 2  # let allocations settle before the RSS sample
  VIEWER_APP_PID=""
  find_app_pid() {
    [ -n "$VIEWER_APP_PID" ] && return 0
    local parent="$1" p
    for p in $(pgrep -P "$parent" 2>/dev/null); do
      case "$(ps -o command= -p "$p" 2>/dev/null)" in
        *target/debug/nanooffice*) VIEWER_APP_PID="$p"; return 0 ;;
      esac
      find_app_pid "$p"
    done
  }
  find_app_pid "$SMOKE_APP_PID"
  if [ -z "$VIEWER_APP_PID" ]; then
    echo "smoke: FAILED — app process not found under pid $SMOKE_APP_PID"
    smoke_fail_tail "$LOG"
    exit 1
  fi
  # PH-0 method (docs/diagnostics.md 27.09): WebKit XPC helpers are spawned by
  # launchd, not the app — recognized by PID adjacency to the app process.
  helpers_kb() {
    local pid cmd rss sum=0
    for pid in $(seq $(($VIEWER_APP_PID - 3)) $(($VIEWER_APP_PID + 64))); do
      cmd=$(ps -o command= -p "$pid" 2>/dev/null) || continue
      case "$cmd" in
        *com.apple.WebKit.WebContent*|*com.apple.WebKit.GPU*|*com.apple.WebKit.Networking*)
          rss=$(ps -o rss= -p "$pid" 2>/dev/null | tr -d ' ')
          sum=$((sum + ${rss:-0})) ;;
      esac
    done
    echo "$sum"
  }
  app_kb=$(ps -o rss= -p "$VIEWER_APP_PID" 2>/dev/null | tr -d ' '); app_kb=${app_kb:-0}
  help_kb=$(helpers_kb "$VIEWER_APP_PID")
  tree_mb=$(( (app_kb + help_kb) / 1024 ))
  SMOKE_SUMMARY="$SMOKE_QV_REPORT tree=${tree_mb}MB (app $((app_kb / 1024)) + helpers $((help_kb / 1024)))"
  if [ "$tree_mb" -gt "$MAX_MB" ]; then
    echo "smoke: FAILED — RSS tree ${tree_mb} MB exceeds budget ${MAX_MB} MB"
    smoke_fail_tail "$LOG"
    exit 1
  fi
  echo "smoke: ✓ first-draw < ${MAX_MS} ms · ✓ layout asserts (delta == line-height == OS/2 formula) · ✓ RSS tree ${tree_mb} MB ≤ ${MAX_MB} MB (app $((app_kb / 1024)) + helpers $((help_kb / 1024)))"
  grep -m1 "$SMOKE_RE_QV_FIRST_DRAW" "$LOG" | sed 's/^/smoke:   /' | cut -c1-160
  grep -m1 "$SMOKE_RE_QV_LAYOUT" "$LOG" | sed 's/^/smoke:   /' | cut -c1-220
  echo "smoke: PASSED — full log kept at $LOG"
  exit 0
fi
smoke_fail_tail "$LOG"
exit 1
