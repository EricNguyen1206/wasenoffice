#!/bin/bash
# smoke-lib.sh — thư viện dùng CHUNG của 3 behavioral smoke gate (T-053, review #2 C3).
#
# CÁCH DÙNG: SOURCE, không execute trực tiếp —
#   . "$ROOT/harness/scripts/smoke-lib.sh"
# rồi gọi theo vòng đời: smoke_init FIXTURE → smoke_boot ENV=… … LOG →
# smoke_poll TIMEOUT PREDICATE [WAITING-FOR] → (thất bại) smoke_fail_tail LOG.
#
# Gate consumer: harness/scripts/smoke-render.sh (gate 9) · smoke-save.sh (gate 10)
# · smoke-clean-machine.sh (gate 11). Trước T-053 cả ba copy boilerplate
# boot/teardown/poll/save-evidence gần nguyên văn và đã lệch style
# (env-inline vs mảng CMD) — giờ 1 nhà ở đây.
#
# LOG CONTRACT — FROZEN (giữ nguyên byte từ gate tiền-T-053):
# mọi chuỗi/pattern log đang grep nằm ở khối hằng dưới đây, KHÔNG inline ở
# consumer. Emitting end giữ pointer comment tên gate script (script đó là
# consumer của lib này — "change in the SAME commit" vẫn áp dụng 2 chiều:
# sửa ở đây = sửa emitting end trong cùng commit, như discipline T-045/T-011).
#   gate 9  · asc_onDocumentContentReady + viewer=WxH/canvas probe + GET /fonts/
#            ← src/shim/delivery.js (font-oracle/canvas probes + contentReady hook)
#   gate 10 · save-end delivered to engine | save delivered:
#            ← src/shim/delivery.js consumeSaveEnd + src-tauri/src/bridge/save.rs
#            (grep cặp — backend OR frontend line, ai tới trước)
#   gate 11 · /Applications/ONLYOFFICE + fallback WARN + strict error
#            ← src-tauri/src/sidecar/mod.rs x2t_path (override info log là dòng
#            mang path app — negative proof bám vào đó)
#   gate 12 · viewer first-draw + viewer layout (T-V2 — ADDITIVE viewer log)
#            ← src/viewer.ts main() (invoke 'viewer_open' ở src-tauri/src/viewer.rs)
#            T-V4 thêm `viewer evidence:` (ADDITIVE — consumer là
#            harness/scripts/parity-view.sh, gate 12 không grep dòng này)
set -u

SMOKE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

# ── Hằng LOG CONTRACT (đóng băng — cross-ref khối comment trên) ──────────────
# gate 9 (render, TC-D01-06)
SMOKE_RE_CONTENT_READY='asc_onDocumentContentReady'
SMOKE_RE_VIEWER='viewer=[0-9]+x[1-9][0-9]*|canvas probe.*[0-9]+x[1-9][0-9]*'
SMOKE_RE_FONT_FETCH='GET /fonts/'
# gate 10/11 (save round-trip) — cặp grep + dòng probe hiển thị
SMOKE_RE_SAVE_DELIVERED='save-end delivered to engine|save delivered:'
SMOKE_RE_SAVE_PROBE='save probe: asc_Save() called'
# gate 11 (clean machine) — fixed-string app path + 2 dòng sidecar
SMOKE_APP_PATH='/Applications/ONLYOFFICE'
SMOKE_RE_FALLBACK_WARN='falling back to the installed ONLYOFFICE'
SMOKE_RE_STRICT_ERROR='clean-machine mode'
# gate 12 (quick-view metrics, T-V2; media T-V3) — dòng ADDITIVE của viewer
SMOKE_RE_QV_FIRST_DRAW='viewer first-draw'
SMOKE_RE_QV_LAYOUT='viewer layout:'
SMOKE_RE_QV_MEDIA='viewer media:'
SMOKE_RE_QV_FAILED='viewer failed:'

# ── Trạng thái dùng chung (consumer đọc, không ghi trực tiếp khi không cần) ──
SMOKE_APP_PID=""     # PID process-group của app (smoke_boot đặt)
SMOKE_LOG=""         # log path (smoke_boot đặt)
SMOKE_SUMMARY=""     # chuỗi tín hiệu cuối (PREDICATE đặt mỗi vòng poll)
SMOKE_ABORT=0        # PREDICATE đặt =1 để dừng poll ngay (vi phạm cứng)
SMOKE_APP_HITS=0     # số dòng log chứa SMOKE_APP_PATH (fail_tail in refs nếu >0)

# smoke_init FIXTURE — precondition chung + sinh fixture nếu thiếu + snapshot
# pre-stat (FIXTURE phải là path FRESH/mutable-được — save gate mutates by design).
# Exit thẳng (không cần trap — chưa boot gì) với đúng thông điệp lỗi cũ.
smoke_init() {
  local fixture="$1"
  SMOKE_FIXTURE="$fixture"
  command -v pnpm >/dev/null 2>&1 || { echo "smoke: pnpm not found"; exit 1; }
  if lsof -nP -iTCP:1420 -sTCP:LISTEN >/dev/null 2>&1; then
    echo "smoke: port 1420 đã có listener (pnpm/tauri dev khác đang chạy?) — tắt rồi chạy lại"
    exit 1
  fi
  if [ ! -f "$fixture" ]; then
    local ext="${fixture##*.}"
    local gen="$SMOKE_ROOT/harness/fixtures/make_${ext}.py"
    [ -f "$gen" ] || { echo "smoke: no fixture generator for .${ext}"; exit 1; }
    python3 "$gen" "$fixture" || { echo "smoke: cannot generate fixture"; exit 1; }
  fi
  SMOKE_PRE_SIZE=$(stat -f '%z' "$fixture")
  SMOKE_PRE_MTIME=$(stat -f '%m' "$fixture")
}

# smoke_boot [ENV=VALUE …] LOG — boot app nền với process group riêng + trap
# teardown EXIT MỘT LẦN (chống double-trap khi lib bị source 2 lần hoặc boot
# lại). Style thống nhất: env binary + mảng (hỗ trợ env có điều kiện của
# gate 11 negative-proof). Toàn bộ chuỗi teardown như trước T-053.
smoke_boot() {
  [ $# -ge 1 ] || { echo "smoke: smoke_boot cần ít nhất đối số LOG"; return 2; }
  local log="${!#}"
  local -a envs=()
  while [ $# -gt 1 ]; do envs+=("$1"); shift; done
  # Job control: background job có process group riêng để trap EXIT giết cả
  # cây (pnpm → tauri-cli → cargo → app) mà không đụng process lạ.
  set -m
  if [ "${#envs[@]}" -gt 0 ]; then
    env "${envs[@]}" pnpm tauri dev >"$log" 2>&1 &
  else
    pnpm tauri dev >"$log" 2>&1 &
  fi
  SMOKE_APP_PID=$!
  SMOKE_LOG="$log"
  if [ -z "${_SMOKE_TRAP_SET:-}" ]; then
    trap smoke_cleanup EXIT
    _SMOKE_TRAP_SET=1
  fi
}

smoke_cleanup() {
  local pid="${SMOKE_APP_PID:-}"
  [ -n "$pid" ] || return 0
  disown "$pid" 2>/dev/null
  kill -- "-$pid" >/dev/null 2>&1
  sleep 1
  kill -9 -- "-$pid" >/dev/null 2>&1
}

# smoke_poll TIMEOUT PREDICATE [WAITING-FOR] — poll mỗi 2s: PREDICATE (tên hàm
# consumer) trả 0 = mọi tín hiệu GREEN (return 0); PREDICATE đặt SMOKE_ABORT=1
# = vi phạm cứng, dừng ngay (return 1); app chết / hết hạn → return 1 kèm
# thông điệp. PREDICATE phải set SMOKE_SUMMARY mỗi vòng để dòng FAILED in số
# liệu của lần poll cuối cùng.
smoke_poll() {
  local timeout_s="$1" predicate="$2" what="${3:-all signals}"
  local deadline=$((SECONDS + timeout_s))
  while :; do
    if "$predicate"; then
      return 0
    fi
    if [ "$SMOKE_ABORT" = "1" ]; then
      return 1
    fi
    if ! kill -0 "$SMOKE_APP_PID" 2>/dev/null; then
      echo "smoke: app process exited before $what appeared"
      return 1
    fi
    if [ "$SECONDS" -ge "$deadline" ]; then
      echo "smoke: timed out after ${timeout_s}s"
      return 1
    fi
    sleep 2
  done
}

# smoke_save_evidence FIXTURE MARKER INNER_XML — bộ ba bằng chứng save của
# gate 10/11, tính vào SMOKE_S1..S3 (+ SMOKE_POST_SIZE/SMOKE_POST_MTIME cho
# dòng PASSED): (1) cặp grep save line trong SMOKE_LOG, (2) marker trong
# inner XML qua unzip -p + grep -cF, (3) size+mtime đổi so với pre-stat của
# smoke_init. Mọi lệnh đo chạy với 2>/dev/null + default 0 — grep -c trả
# exit 1 khi 0 match, không được để exit code làm hỏng vòng poll.
smoke_save_evidence() {
  local fixture="$1" marker="$2" inner_xml="$3"
  SMOKE_S1=$(grep -Ec "$SMOKE_RE_SAVE_DELIVERED" "$SMOKE_LOG" 2>/dev/null); SMOKE_S1=${SMOKE_S1:-0}
  SMOKE_S2=$(unzip -p "$fixture" "$inner_xml" 2>/dev/null | grep -cF -- "$marker"); SMOKE_S2=${SMOKE_S2:-0}
  SMOKE_POST_SIZE=$(stat -f '%z' "$fixture" 2>/dev/null); SMOKE_POST_SIZE=${SMOKE_POST_SIZE:-0}
  SMOKE_POST_MTIME=$(stat -f '%m' "$fixture" 2>/dev/null); SMOKE_POST_MTIME=${SMOKE_POST_MTIME:-0}
  if [ "$SMOKE_POST_SIZE" != "$SMOKE_PRE_SIZE" ] && [ "$SMOKE_POST_MTIME" != "$SMOKE_PRE_MTIME" ]; then
    SMOKE_S3=1
  else
    SMOKE_S3=0
  fi
}

# smoke_fail_tail LOG — output FAIL chuẩn: dòng FAILED với SMOKE_SUMMARY của
# PREDICATE (+ khối tham chiếu app cài sẵn khi gate clean-machine bắt được
# SMOKE_APP_HITS > 0) + 40 dòng log cuối.
smoke_fail_tail() {
  local log="$1"
  echo "smoke: FAILED — ${SMOKE_SUMMARY:-no signals recorded}"
  if [ "$SMOKE_APP_HITS" -gt 0 ]; then
    echo "smoke: log tham chiếu app cài sẵn (${SMOKE_APP_HITS} lần):"
    grep -nF "$SMOKE_APP_PATH" "$log" | head -5 | sed 's/^/smoke:   /' | cut -c1-160
  fi
  echo "smoke: log tail (last 40 lines of $log):"
  tail -40 "$log"
}

# Guard: lib bị execute trực tiếp thay vì source.
if [ "${BASH_SOURCE[0]}" = "$0" ]; then
  echo "smoke-lib.sh là thư viện SOURCE ('. harness/scripts/smoke-lib.sh'), không execute trực tiếp"
  exit 1
fi
