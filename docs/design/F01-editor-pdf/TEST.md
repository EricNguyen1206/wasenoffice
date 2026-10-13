---
type: test
---
# Test Plan — F01 · editor PDF

## 0. Metadata

| Trường | Giá trị |
|---|---|
| Mã màn hình | `F01` |
| DD tương ứng | [`DD.md`](DD.md) |
| Phase / điều kiện chạy | PH-1 |
| Cập nhật gần nhất | 2026-09-29 |

## 1. Phạm vi & ma trận loại test

- **In scope:** mở PDF → render (behavioral, tự động).
- **Out of scope:** save/export PDF (T-049) · form filling (cắt) · visual chi tiết (PH-2).
- Ma trận: `integration` ✅ · `unit` ⬜ · `E2E manual` ⬜ · `visual` ⬜.

## 2. Môi trường & fixtures

- Fixture: `harness/fixtures/make_pdf.py` (stdlib, ISO 32000 viết tay; 1 trang;
  tiếng Việt ở Title metadata UTF-16BE — lý do ở DD §10/OQ-F01-2).
- Lệnh gate: `harness/scripts/smoke-render.sh /tmp/wasenoffice-smoke-fixture.pdf`
  (verify.sh chạy tự động — T-048).

## 3. Test cases

| ID | Ưu tiên | Loại | Truy vết DD | Given / When / Then | Bằng chứng mong đợi | Trạng thái |
|---|---|---|---|---|---|---|
| TC-F01-01 | P0 | integration | DD §3 | Mở fixture PDF hợp lệ → boot pdfeditor → deliver bytes gốc (không x2t) | `asc_onDocumentContentReady` + canvas >0 + `GET /fonts/` trong log; smoke exit 0 | GREEN (29.09, T-048) |
