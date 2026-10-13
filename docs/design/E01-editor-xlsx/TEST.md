---
type: test
---
# Test Plan — E01 · editor XLSX

## 0. Metadata

| Trường | Giá trị |
|---|---|
| Mã màn hình | `E01` |
| DD tương ứng | [`DD.md`](DD.md) |
| Phase / điều kiện chạy | PH-1 (máy có x2t từ ONLYOFFICE.app — A1 chưa bundle) |
| Cập nhật gần nhất | 2026-09-29 |

## 1. Phạm vi & ma trận loại test

- **In scope:** mở XLSX → render (behavioral, tự động).
- **Out of scope:** save XLSX (T-049) · UI upstream render chi tiết (visual manual — PH-2).
- Ma trận: `integration` (boot app thật) ✅ · `unit` ⬜ (không có logic riêng đủ lớn) ·
  `E2E manual` ⬜ · `visual` ⬜.

## 2. Môi trường & fixtures

- Fixture: `harness/fixtures/make_xlsx.py` (stdlib, ECMA-376; tiếng Việt có dấu).
- Lệnh gate: `harness/scripts/smoke-render.sh /tmp/wasenoffice-smoke-fixture.xlsx`
  (verify.sh chạy tự động — T-048).

## 3. Test cases

| ID | Ưu tiên | Loại | Truy vết DD | Given / When / Then | Bằng chứng mong đợi | Trạng thái |
|---|---|---|---|---|---|---|
| TC-E01-01 | P0 | integration | DD §3 | Mở fixture XLSX hợp lệ → boot spreadsheeteditor → deliver Editor.bin (XLSY) | `asc_onDocumentContentReady` + canvas >0 + `GET /fonts/` trong log; smoke exit 0 | GREEN (29.09, T-048) |
| TC-E01-02 | P1 | integration | DD §3 | Fixture xlsx hỏng (không phải zip) → x2t fail | `document delivery failed: x2t produced no Editor.bin` + gate exit 1 | GREEN (29.09, negative test chung gate — chạy trên xlsx hỏng) |
