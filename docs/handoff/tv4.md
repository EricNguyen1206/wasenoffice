---
type: handoff
---
# Handoff — T-V4: A3/V4 — parity render viewer ↔ sdkjs qua oracle CDP

> Bạn (agent) đang ở phiên mới. File này LÀ prompt công việc.

## Nhiệm vụ

**T-V4 (Track A, rung A3, slice V4):** chứng minh viewer render **parity** với engine
sdkjs trên corpus — engine làm ORACLE (CDP `:9222`, kỹ thuật `docs/diagnostics.md` 22.09
bổ sung; boot app thật + `open -a` + quit là hợp lệ, CẤM keystroke). Metric theo DD V01
(§4 V4 + §5): đề xuất — text content exact theo dòng, số dòng, tọa độ baseline trong
tolerance (font metrics khác nhau giữa 2 renderer → tolerance chốt bằng số đo, ghi DD),
ảnh position/size. Parity harness dạng script (pattern parity-a2.sh) + bảng kết quả
corpus; khác biệt nào phải phân loại (bug viewer / ceiling đã ghi / khác biệt
metric/expected của oracle). KHÔNG fix mọi ceiling trong phiên này — parity đo được là
đích; ceiling lớn lộ ra thì seed task.

## Bước 0 — đọc trước khi làm

1. `AGENTS.md` (rule 6b) → `harness/rules/clean-room.md` + `mit-purity.md` (oracle-obs:
   quan sát runtime hợp lệ; CẤM đọc source render sdkjs).
2. `docs/design/viewer-a3/DD.md` TOÀN BỘ (bạn = V4; §5.3/§5.4 số đo + ceilings V2/V3).
3. `src/viewer.ts` (renderer + log evidence hiện có: baselines/lhs/drawn/ink) +
   `src-tauri/src/viewer.rs` + `harness/scripts/smoke-quick-view.sh` (pattern asserts)
   + `harness/scripts/parity-a2.sh` (pattern harness + corpus).
4. `docs/diagnostics.md` 3 entry mới nhất. `docs/TODO.md` — claim T-V4 (🟨 + ngày).

## Chứng minh bắt buộc

- Bảng parity corpus ≥ 5 file (kể cả fixture layout V2 + media V3 + tiếng Việt + full):
  từng file từng metric pass/tolerance/lệch-đã-phân-loại; harness chạy ×2 deterministic
  exit 0; negative (tiêm diff giả) cắn exit 1.
- Gates hiện có không thoái hóa: verify.sh FULL exit 0 + parity-a2.sh exit 0.
- Đóng góp phụ nếu rẻ: extract layout-evidence JSON từ viewer để harness so tự động.

## Ràng buộc

- Ponytail ULTRA (đọc skill; ponytail-review trước commit). Boot nền rule 6b; CẤM
  keystroke osascript; quit app thật sau dùng.
- Không đụng luồng sdkjs/adapter/log contract gates 9/10/11; không mở scope sang V5 UX
  hay T-058 Tailwind.
- Commits scoped `a3(viewer|harness): T-V4 …` + docs; TODO ✅ + ▶ = T-058 (Tailwind —
  đúng trigger trước T-V5); diagnostics 1 entry (re-emit header cũ); xoá handoff khi
  đóng. Tiếng Việt; lệnh + exit code + dòng System-1.
