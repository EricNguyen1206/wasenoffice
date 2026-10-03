---
type: handoff
---
# Handoff — T-V1: A3/V1 — model render-side + resolve về Rust (DD V01 §3.2)

> Bạn (agent) đang ở phiên mới. File này LÀ prompt công việc.

## Nhiệm vụ

**T-V1 (Track A, rung A3, slice V1):** mở model crate `nanooffice-ooxml` theo **bảng
construct render-side C2** trong `docs/design/viewer-a3/DD.md` §3.2 — mỗi mục kèm
spec-ref sẵn (rFonts §17.3.2.26, w:br/w:tab, bold/italic 3-state, rStyle resolve,
color/spacing/ind/tblGrid/pgSz, docDefaults). Resolve chuỗi style về Rust (con thắng cha,
docDefaults đáy) — viewer chỉ nhận model đã resolved. Giữ 2 invariant parity DD ghi.
CẢN T-V2 (layout+gates): không làm layout/gate RAM.

## Bước 0 — đọc trước khi làm

1. `AGENTS.md` → `harness/rules/clean-room.md` + `mit-purity.md` (spec-ref/oracle-obs;
   CẤM đọc upstream render code).
2. `docs/design/viewer-a3/DD.md` TOÀN BỘ (bạn = V1) + `docs/design/converter-a2/DD.md` §5
   (parity style).
3. `src-tauri/crates/nanooffice-ooxml/` (model hiện tại) + `src-tauri/src/viewer.rs`
   (slice V0 — consumer của bạn) + `harness/scripts/parity-a2.sh` + `harness/fixtures/`
   (corpus-a2 NOTES).
4. `docs/diagnostics.md` 3 entry mới nhất. `docs/TODO.md` — claim T-V1 (🟨 + ngày).

## Chứng minh bắt buộc

- Mỗi construct mới: parse + resolved output đúng trên corpus (test assert resolved
  values, không chỉ raw); style-chain resolve có test cycle/override; parity-a2.sh vẫn
  exit 0 (không thoái hóa A2); fuzz mutation → Err không panic.
- Viewer V0 vẫn render GREEN (số đo không thoái hóa) + `cargo test --workspace` +
  `verify.sh` exit 0.

## Ràng buộc

- Ponytail ULTRA (đọc `.agents/skills/ponytail/SKILL.md`; ponytail-review trước commit —
  findings xử lý hoặc ghi lý do giữ). Model chỉ thêm field có consumer (viewer/test).
- Commits scoped `a3(crate): T-V1 …` + docs; TODO ✅ sink + ▶ = T-V2; diagnostics 1 entry
  (re-emit header cũ); xoá handoff khi đóng. Giao tiếp tiếng Việt; lệnh + exit code +
  dòng System-1 trong báo cáo.
