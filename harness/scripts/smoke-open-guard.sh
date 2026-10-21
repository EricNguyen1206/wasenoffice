#!/bin/bash
# WasenOffice smoke gate 19 — open guard + size-streaming delivery (T-067).
#
# Ba mảnh bằng chứng trên app thật (boot thật, KHÔNG mock), mỗi mảnh một boot:
#   arm 1 (REFUSAL)   : file sparse 300 MB (mkfile -n, > default cap 250 MB)
#                       qua WASEN_OFFICE_OPEN → log phải có
#                       `open guard: file too large to open` (emit
#                       src-tauri/src/config.rs ensure_openable_size_lookup,
#                       lỗi bubble open_document → main.ts `[web] ERROR`)
#                       + dòng `open_document:` đi trước + process vẫn sống.
#                       Refuse xảy ra TRƯỚC registry/recents — không cạn RAM.
#   arm 2 (COUNTERFACTUAL): cùng file sparse, kill switch
#                       WASEN_OFFICE_MAX_OPEN_MB=0 (unlimited) → `open guard:`
#                       KHÔNG được xuất hiện sau khi `open_document:` đã chạy
#                       → chứng minh refusal đến từ cap, không phải lỗi khác.
#                       Engine nhận file rác sẽ tự báo lỗi riêng (đó là
#                       hành vi engine, không phải của guard này).
#   arm 3 (GREEN PATH): fixture docx nhỏ thật, cap mặc định → renderer ready
#                       (cặp tín hiệu gate 9) — chứng minh đường delivery mới
#                       (fetch /delivery/<id>/<name> + binary_content://) mở
#                       file nhỏ bình thường, không thoái hóa.
#
# LOG CONTRACT: SMOKE_RE_OPEN_GUARD nằm trong harness/scripts/smoke-lib.sh
# (khối gate 19) — đổi chuỗi ở config hoặc ở đây trong CÙNG commit.
#
# Usage: smoke-open-guard.sh
# Env:   SMOKE_OPEN_TIMEOUT=90                mỗi arm
#        SMOKE_OPEN_SPARSE_MB=300             kích thước file sparse của arm 1/2
#        SMOKE_OPEN_CF_WAIT=24                giây chờ arm 2 trước khi khẳng định vắng
set -u

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
. "$ROOT/harness/scripts/smoke-lib.sh"

TIMEOUT="${SMOKE_OPEN_TIMEOUT:-90}"
SPARSE_MB="${SMOKE_OPEN_SPARSE_MB:-300}"
CF_WAIT="${SMOKE_OPEN_CF_WAIT:-24}"
SPARSE_BASE="$(mktemp /tmp/wasenoffice-open-guard-XXXXXX)"
SPARSE="$SPARSE_BASE.docx"
LOG1="$(mktemp /tmp/wasenoffice-open-guard-refusal-XXXXXX)"
LOG2="$(mktemp /tmp/wasenoffice-open-guard-unlimited-XXXXXX)"
LOG3="$(mktemp /tmp/wasenoffice-open-guard-green-XXXXXX)"

# smoke_init thấy .docx thiếu → tự sinh fixture docx thật (nhỏ) — rồi dd seek
# phình logical size: guard đọc metadata size, không cần nội dung (arm 2 chỉ
# khẳng định GUARD im — file sau dd không còn hợp lệ cho zip reader, đó là
# hành vi engine, không thuộc gate này).
smoke_init "$SPARSE" || exit 1
dd of="$SPARSE" if=/dev/zero bs=1 count=1 seek="$((SPARSE_MB * 1024 * 1024))" 2>/dev/null \
  || { echo "smoke: cannot create sparse ${SPARSE_MB}MB file"; exit 1; }

FIXTURE="$(mktemp /tmp/wasenoffice-open-guard-fixture-XXXXXX).docx"
python3 "$SMOKE_ROOT/harness/fixtures/make_docx.py" "$FIXTURE" >/dev/null \
  || { echo "smoke: cannot generate small fixture"; exit 1; }

kill_app() {
  [ -n "${SMOKE_APP_PID:-}" ] || return 0
  disown "$SMOKE_APP_PID" 2>/dev/null
  kill -- "-$SMOKE_APP_PID" >/dev/null 2>&1
  sleep 2
  kill -9 -- "-$SMOKE_APP_PID" >/dev/null 2>&1
  SMOKE_APP_PID=""
}
# không dùng trap EXIT mặc định của lib để tự nhịp kill giữa các arm…
# vẫn để trap phòng case thoát đột ngột (nó kill pid mới nhất — an toàn).
trap kill_app EXIT

echo "smoke: gate 19 — guard refusal/counterfactual/green (sparse=${SPARSE_MB}MB, log base=$(dirname "$LOG1"))"

# ── arm 1: REFUSAL trên cap mặc định (250 MB) ────────────────────────────────
arm_refusal() {
  grep -q "$SMOKE_RE_OPEN_DOC" "$SMOKE_LOG" 2>/dev/null || return 1
  guard_hits=$(grep -c "$SMOKE_RE_OPEN_GUARD" "$SMOKE_LOG" 2>/dev/null); guard_hits=${guard_hits:-0}
  [ "$guard_hits" -ge 1 ] || return 1
  kill -0 "$SMOKE_APP_PID" 2>/dev/null || return 1
  return 0
}
echo "smoke: arm 1 — refusal (default cap 250 MB)"
smoke_boot "WASEN_OFFICE_OPEN=$SPARSE" "WASEN_OFFICE_DEVTOOLS=0" "RUST_LOG=info" "WASEN_OFFICE_PROBE=0" "$LOG1"
if ! smoke_poll "$TIMEOUT" arm_refusal "open guard refusal"; then
  smoke_fail_tail "$SMOKE_LOG"
  exit 1
fi
echo "smoke: ✓ guard từ chối file ${SPARSE_MB}MB trên cap mặc định — app vẫn sống"
kill_app

# ── arm 2: COUNTERFACTUAL — cap=0 không được bắn guard ───────────────────────
echo "smoke: arm 2 — counterfactual cap=0 (guard phải vắng)"
sleep 3   # nhả port 1420 trước arm kế (gate boot lại cùng dev server)
smoke_boot "WASEN_OFFICE_OPEN=$SPARSE" "WASEN_OFFICE_MAX_OPEN_MB=0" \
  "WASEN_OFFICE_DEVTOOLS=0" "RUST_LOG=info" "WASEN_OFFICE_PROBE=0" "$LOG2"
sleep "$CF_WAIT"
cf_open=$(grep -c "$SMOKE_RE_OPEN_DOC" "$LOG2" 2>/dev/null); cf_open=${cf_open:-0}
cf_guard=$(grep -c "$SMOKE_RE_OPEN_GUARD" "$LOG2" 2>/dev/null); cf_guard=${cf_guard:-0}
cf_alive=$(kill -0 "${SMOKE_APP_PID:-0}" 2>/dev/null && echo 1 || echo 0)
if [ "$cf_open" -lt 1 ] || [ "$cf_guard" -ne 0 ] || [ "$cf_alive" != 1 ]; then
  SMOKE_SUMMARY="cf-open=$cf_open cf-guard=$cf_guard cf-alive=$cf_alive"
  smoke_fail_tail "$LOG2"
  exit 1
fi
echo "smoke: ✓ cap=0 mở cùng file — 0 dòng guard (refusal đến từ cap, không phải lỗi khác)"
kill_app

# ── arm 3: GREEN PATH — file nhỏ qua delivery mới, renderer ready ────────────
arm_green() {
  smoke_s1=$(grep -c "$SMOKE_RE_CONTENT_READY" "$SMOKE_LOG" 2>/dev/null); smoke_s1=${smoke_s1:-0}
  [ "$smoke_s1" -ge 1 ]
}
echo "smoke: arm 3 — green path qua /delivery (fixture nhỏ)"
sleep 3
smoke_boot "WASEN_OFFICE_OPEN=$FIXTURE" "WASEN_OFFICE_DEVTOOLS=0" "RUST_LOG=info" "WASEN_OFFICE_PROBE=0" "$LOG3"
if ! smoke_poll "$TIMEOUT" arm_green "renderer ready"; then
  smoke_fail_tail "$SMOKE_LOG"
  exit 1
fi
echo "smoke: ✓ content ready trên fixture nhỏ — đường fetch/stream không thoái hóa"
kill_app

echo "smoke: PASSED — guard refusal (arm 1) · counterfactual cap=0 (arm 2) · green path (arm 3)"
echo "smoke: logs: $LOG1 $LOG2 $LOG3"
exit 0
