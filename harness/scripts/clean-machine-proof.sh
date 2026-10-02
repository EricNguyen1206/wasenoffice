#!/bin/bash
# T-A13 integrative proof (Track A rung A1, 02.10.2026) — run the FULL verify
# harness with the installed ONLYOFFICE.app temporarily HIDDEN.
#
# This is the closing evidence for rung A1: every check — including gate 11
# (smoke-clean-machine.sh) — must pass while /Applications/ONLYOFFICE.app does
# not exist. The app is MOVED aside on the same volume (atomic) and ALWAYS
# restored, via an idempotent EXIT/INT/TERM trap:
#   - never rm, never touch anything else in /Applications
#   - refuses to start if ONLYOFFICE is running (osascript quit + 30s wait,
#     NO kill -9 — it is the user's app)
#   - mv failure (SIP/perm) → exit 3 → plan B: strict gate + reference table
#     only, proof limitation recorded in docs/diagnostics.md
# Restore is verified with `ls` before the script exits.
#
# Usage: clean-machine-proof.sh
set -u

APP="/Applications/ONLYOFFICE.app"
HIDDEN="/Applications/ONLYOFFICE.app.ta13-hidden"
LOG="/tmp/verify-ta13-clean-machine.log"
RESTORED=0

restore() {
  [ "$RESTORED" = "1" ] && return 0
  RESTORED=1
  if [ -d "$HIDDEN" ] && [ ! -e "$APP" ]; then
    echo "proof: RESTORE mv '$HIDDEN' → '$APP'"
    if ! mv "$HIDDEN" "$APP"; then
      echo "proof: ✗✗ RESTORE THẤT BẠI — app còn nằm ở '$HIDDEN', mv tay NGAY"
      exit 4
    fi
  fi
  if [ -d "$APP" ]; then
    echo "proof: ✓ app đã khôi phục: $(ls -d "$APP")"
  elif [ -d "$HIDDEN" ]; then
    echo "proof: ✗✗ trạng thái lạ — app ở '$HIDDEN', không có ở '$APP' — mv tay NGAY"
    exit 4
  else
    echo "proof: ✗✗ không thấy app ở CẢ HAI vị trí — kiểm tra /Applications ngay"
    exit 4
  fi
}
trap restore EXIT
trap 'restore; exit 130' INT TERM

# --- preconditions ---------------------------------------------------------
if [ -d "$HIDDEN" ]; then
  echo "proof: ✗ '$HIDDEN' tồn tại từ lần chạy trước — restore tay trước khi chạy lại"
  exit 4
fi
if [ ! -d "$APP" ]; then
  echo "proof: ✗ '$APP' không tồn tại — không có gì để ẩn (máy đã sạch?) — bài test mv-app không áp dụng; dùng phương án B (gate strict + bảng đối chiếu)"
  exit 3
fi
if pgrep -x ONLYOFFICE >/dev/null 2>&1; then
  echo "proof: ONLYOFFICE đang chạy — osascript quit + chờ tối đa 30s"
  osascript -e 'tell application "ONLYOFFICE" to quit' || true
  for _ in {1..30}; do
    pgrep -x ONLYOFFICE >/dev/null 2>&1 || break
    sleep 1
  done
  if pgrep -x ONLYOFFICE >/dev/null 2>&1; then
    echo "proof: ✗ ONLYOFFICE vẫn chạy sau 30s — KHÔNG kill -9 (app của user) — abort"
    exit 4
  fi
fi
echo "proof: ✓ preconditions — app tồn tại, 0 process ONLYOFFICE"

# --- hide (same volume → atomic) --------------------------------------------
echo "proof: HIDE mv '$APP' → '$HIDDEN'"
if ! mv "$APP" "$HIDDEN"; then
  echo "proof: ✗ mv thất bại (SIP/perm?) — phương án B: chỉ gate 11 strict + bảng đối chiếu runtime paths"
  exit 3
fi
if [ ! -d "$HIDDEN" ] || [ -e "$APP" ]; then
  echo "proof: ✗ trạng thái sau mv không như kỳ vọng — restore và abort"
  restore
  exit 4
fi
echo "proof: ✓ app đã ẩn — toàn bộ /Applications không còn ONLYOFFICE.app"

# --- full harness with the app absent ---------------------------------------
echo "proof: chạy ./harness/scripts/verify.sh FULL (log: $LOG) — có thể mất vài phút"
./harness/scripts/verify.sh >"$LOG" 2>&1
VERIFY=$?
tail -3 "$LOG" | sed 's/^/proof:   /'

# --- restore ------------------------------------------------------------------
restore

# --- verdict -------------------------------------------------------------------
if [ "$VERIFY" -eq 0 ]; then
  echo "proof: ✓✓ verify.sh FULL exit 0 với app ẨN — máy không cài ONLYOFFICE chạy trọn harness"
else
  echo "proof: ✗ verify.sh FAIL exit $VERIFY khi app ẩn — log đầy đủ: $LOG"
  tail -40 "$LOG"
fi
exit "$VERIFY"
