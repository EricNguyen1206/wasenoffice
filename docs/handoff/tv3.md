---
type: handoff
---
# Handoff — T-V3: A3/V3 — media bytes + render ảnh theo wp:extent

> Bạn (agent) đang ở phiên mới. File này LÀ prompt công việc.

## Nhiệm vụ

**T-V3 (Track A, rung A3, slice V3):** viewer hiển thị được ảnh thật. (1) Serve
`word/media/*` bytes cho viewer qua filesrv **route an toàn** (fileId đã có; KHÔNG mở path
traversal — tham khảo route `/fonts/` và `/file/<id>/`; media đi qua workdir/fileId, không
đường đĩa thẳng). (2) Model đã có image relationship + `wp:extent` EMU (T-A21) — render ảnh
đúng vị trí/kích thước EMU→px (spec-ref §17.3.3 / DrawingML; quy ước 914400 EMU/inch —
verify nguồn mở). (3) Layout ảnh + wrap text tối thiểu (ảnh trên dòng riêng đủ — inline
wrap quanh ảnh KHÔNG bắt buộc, ghi ceiling `ponytail:`).

## Bước 0 — đọc trước khi làm

1. `AGENTS.md` (rule 6b boot nền/focused(false)) → `harness/rules/clean-room.md` +
   `mit-purity.md` (spec-ref/oracle-obs; CẤM đọc upstream render code).
2. `docs/design/viewer-a3/DD.md` TOÀN BỘ (bạn = V3; §5.3 số đo V2; §10 OQ đã chốt).
3. `src-tauri/src/viewer.rs` + `src/viewer.ts` (V0–V2) + crate model image fields +
   `protocol/filesrv.rs` (pattern route) + `harness/fixtures/make_docx_layout.py` /
   corpus-a2 (fixture ảnh đã có từ T-A26 — media-fixture bytes ≠ placeholder).
4. `docs/diagnostics.md` 3 entry mới nhất. `docs/TODO.md` — claim T-V3 (🟨 + ngày).

## Chứng minh bắt buộc

- Test: ảnh bytes thật (≠ placeholder) qua viewer — assert kích thước vẽ đúng EMU→px;
  fixture nhiều ảnh + ảnh trong header nếu corpus có.
- Gate 12 vẫn exit 0 (số đo không thoái hóa quá budget); thêm assert ảnh vào gate nếu rẻ.
- parity-a2.sh + cargo test --workspace + verify.sh exit 0. Route mới: thử traversal
  (`../`) phải 403/404 (test).
- E2E: quick-view file có ảnh qua app — log/inked evidence ảnh pixel thật vẽ (không chỉ
  text ink).

## Ràng buộc

- Ponytail ULTRA (đọc skill; ponytail-review trước commit). Không đụng luồng sdkjs/
  adapter/log contract gates 9/10/11; boot nền rule 6b.
- Commits scoped `a3(viewer): T-V3 …` + docs; TODO ✅ + ▶ = T-V4; diagnostics 1 entry
  (re-emit header cũ); xoá handoff khi đóng. Tiếng Việt; lệnh + exit code + System-1.
