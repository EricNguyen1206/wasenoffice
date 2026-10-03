---
type: handoff
---
# Handoff — T-V5: wire UX quick-view + ĐÓNG RUNG A3

> Bạn (agent) đang ở phiên mới. File này LÀ prompt công việc.

## Nhiệm vụ

**T-V5 (Track A, rung A3 slice cuối):** (1) UX tối thiểu để quick-view dùng được thật:
theo DD V01 (mục V5/§4 — đọc DD trước khi chốt scope; hướng: chọn file từ UI mở quick-view
thay chỉ env `NANO_OFFICE_QUICK_VIEW`, scroll cơ bản, phím tắt thoát — CHỈ cái DD ghi,
không tự thêm); mọi style mới dùng token-slot Tailwind T-058. (2) **ĐÓNG RUNG A3**:
vision QĐ9 bảng thang dòng A3 ✅ kèm evidence (T-V4 parity đạt + T-V2 gates GREEN + số
đo); RAM <150 MB là criterion RELEASE — T-057 (release stall, PH-4) chặn đo release →
ghi ceiling thẳng như T-V2 đã làm (debug tripwire 300 MB), không fake số. (3) Seed mở
màn A4 theo thang QĐ9 (editor core "Nano Light" — task pointer, không thiết kế).

## Bước 0 — đọc trước khi làm

1. `AGENTS.md` (rule 6b boot nền/focused(false)) → `harness/rules/clean-room.md` +
   `mit-purity.md`.
2. `docs/design/viewer-a3/DD.md` TOÀN BỘ (V5 row + §4 bảng slice + §5 số đo mọi phiên).
3. `src/viewer.html`/`src/viewer.ts` + token slots T-058 + `src-tauri/src/viewer.rs` +
   `harness/scripts/smoke-quick-view.sh` + `smoke-lib.sh`.
4. `docs/diagnostics.md` 3 entry mới nhất (T-058 bẫy font-smoothing scope #app).
5. `docs/TODO.md` — claim T-V5 (🟨 + ngày).

## Chứng minh bắt buộc

- UX flow E2E từ UI (không env): chọn file → quick-view render — bằng chứng log + số đo
  không thoái hóa; gates hiện có FULL exit 0 (thêm assert UX nếu rẻ, negative proven).
- Đóng rung: vision A3 ✅ + AGENTS snapshot + TODO dashboard + ▶ = T-055 review kiến trúc
  #4 (post-A3 — đúng nhịp T-055) rồi task A4 seed; diagnostics 1 entry (re-emit header cũ).
- verify.sh exit 0 lần cuối trên cây commit.

## Ràng buộc

- Ponytail ULTRA (đọc skill; ponytail-review trước commit). Clean-room như các phiên A3.
- Không đụng luồng sdkjs/adapter/log contract gates 9/10/11; boot nền rule 6b; canvas
  render logic chỉ đụng nếu UX đòi (scroll = vị trí vẽ, không phải layout).
- Commits scoped `a3(viewer|ui): T-V5 …` + docs; xoá handoff khi đóng. Tiếng Việt;
  lệnh + exit code + dòng System-1.
