---
type: template
---
# Test Plan — <Tên màn hình>

<!-- Copy file này vào docs/design/[Mã]-[tên]/TEST.md và điền; SAU KHI COPY đổi `type: template`
     thành `type: test`. Mọi test case phải truy vết được về mục tương ứng trong DD.md. -->

## 0. Metadata

| Trường | Giá trị |
|---|---|
| Mã màn hình | `<Chữ><2 số>[-n]` — vd `D01` |
| DD tương ứng | `DD.md` (cùng thư mục — sau khi copy) |
| Phase / điều kiện chạy | PH-x |
| Cập nhật gần nhất | YYYY-MM-DD |

## 1. Phạm vi & ma trận loại test

- **In scope:** …
- **Out of scope:** … (thuộc màn/phase khác — ghi rõ thuộc đâu)
- Ma trận áp dụng: `unit` / `integration` / `E2E manual` / `visual` — đánh ✅ đang dùng,
  ⬜ sẽ thêm (kèm phase dự kiến).

## 2. Môi trường & fixtures

Cách dựng môi trường + fixture: đường dẫn script, lệnh chạy, điều kiện đặc biệt
(vd `harness/fixtures/make_docx.py`, CDP oracle app thật, engine artifact).

## 3. Test cases

| ID | Ưu tiên | Loại | Truy vết DD | Given / When / Then | Bằng chứng mong đợi | Trạng thái |
|---|---|---|---|---|---|---|

Quy ước:

- **Ưu tiên:** P0 = blocker sign-off · P1 = nên có · P2 = nice-to-have.
- **ID case:** `TC-<mã màn>-nn` (vd `TC-D01-06`) — duy nhất, không tái dùng.
- **Trạng thái:** GREEN · RED (kèm lý do + link câu hỏi mở DD §10) · BLOCKED · N/A.
- **Bằng chứng:** tự động được → lệnh + exit code; manual → observable cụ thể (tên marker/log/ảnh chụp),
  không viết "chạy OK".

## 4. Chính sách hồi quy

Sửa gì → chạy lại case nào (tối thiểu: toàn bộ P0 của màn này).

## 5. Tiêu chí sign-off

- [ ] Mọi case P0 GREEN (hoặc RED được duyệt kèm lý do + câu hỏi mở)
- [ ] … điều kiện riêng của màn

## 6. Changelog

| Ngày | Thay đổi | Commit |
|---|---|---|
