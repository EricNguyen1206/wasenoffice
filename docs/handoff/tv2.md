---
type: handoff
---
# Handoff — T-V2: A3/V2 — layout tối thiểu + gates số liệu (đóng OQ-V01-2)

> Bạn (agent) đang ở phiên mới. File này LÀ prompt công việc.

## Nhiệm vụ

**T-V2 (Track A, rung A3, slice V2):** layout tối thiểu cho viewer + **gates số liệu đầu
tiên trong verify.sh**. (1) Layout: line breaking theo `pgSz` (wrap đơn giản greedy),
tab stop, line-height từ accessor OS/2 (C4-a — allfonts đã có ascent/descent/line_gap);
multi-line render thật trên canvas V0. (2) **Chốt công thức RAM OQ-V01-2** (DD §10): app-only
hay cả tree WebKit XPC — đề xuất bằng số đo, ghi DD. (3) Gate mới verify.sh: quick-view
<500 ms (parse→first-draw) + RAM theo công thức chốt — positive + negative proven, dùng
`smoke-lib.sh`; **boot nền rule 6b** (không cửa sổ flash nếu làm được, tối thiểu không
frontmost). CẢN T-V3 (media).

## Bước 0 — đọc trước khi làm

1. `AGENTS.md` (kể cả rule 6b app boot nền) → `harness/rules/clean-room.md` + `mit-purity.md`.
2. `docs/design/viewer-a3/DD.md` TOÀN BỘ (bạn = V2; §3.3 C4-a; §10 OQ-V01-2 là của bạn).
3. `src-tauri/src/viewer.rs` + `src/viewer.ts` (V0/V1 hiện tại) + crate `resolve.rs` +
   `allfonts.rs` (accessor metrics) + `harness/scripts/smoke-lib.sh`.
4. `docs/diagnostics.md` 3 entry mới nhất (T-V1 số đo; T-A25 V0 method đo RAM).
5. `docs/TODO.md` — claim T-V2 (🟨 + ngày).

## Chứng minh bắt buộc

- Layout test: fixture nhiều dòng wrap theo pgSz + tab + line-height assert được (tọa độ
  y tăng đúng line-height); parity-a2.sh + cargo test --workspace + verify.sh exit 0
  (gates cũ không thoái hóa).
- Gate mới: số đo thật trong log (parse ms, first-draw ms, RAM theo công thức) + positive
  exit 0 + negative proven exit 1; số liệu ghi DD + diagnostics (so V0/V1).

## Ràng buộc

- Ponytail ULTRA (đọc skill; ponytail-review trước commit). Clean-room spec-ref/oracle-obs.
- Không đụng luồng sdkjs/adapter/log contract gates 9/10/11; boot nền rule 6b.
- Commits scoped `a3(viewer): T-V2 …` + docs; TODO ✅ + ▶ = T-V3; diagnostics 1 entry
  (re-emit header cũ); xoá handoff khi đóng. Tiếng Việt; lệnh + exit code + System-1.
