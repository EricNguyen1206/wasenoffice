---
type: test
---
# Test Plan — P01 · editor PPTX

## 0. Metadata

| Trường | Giá trị |
|---|---|
| Mã màn hình | `P01` |
| DD tương ứng | [`DD.md`](DD.md) |
| Phase / điều kiện chạy | PH-1 (máy có x2t từ ONLYOFFICE.app — A1 chưa bundle) |
| Cập nhật gần nhất | 2026-09-29 |

## 1. Phạm vi & ma trận loại test

- **In scope:** mở PPTX → render (behavioral, tự động).
- **Out of scope:** save PPTX (T-049) · UI upstream render chi tiết (visual manual — PH-2).
- Ma trận: `integration` ✅ · `unit` ⬜ · `E2E manual` ⬜ · `visual` ⬜.

## 2. Môi trường & fixtures

- Fixture: `harness/fixtures/make_pptx.py` (stdlib, ECMA-376 PresentationML; tiếng Việt
  có dấu; 1 slide + 1 master + 1 layout + 1 theme — bộ part tối giản).
- Lệnh gate: `harness/scripts/smoke-render.sh /tmp/wasenoffice-smoke-fixture.pptx`
  (verify.sh chạy tự động — T-048).

## 3. Test cases

| ID | Ưu tiên | Loại | Truy vết DD | Given / When / Then | Bằng chứng mong đợi | Trạng thái |
|---|---|---|---|---|---|---|
| TC-P01-01 | P0 | integration | DD §3 | Mở fixture PPTX hợp lệ → boot presentationeditor → deliver Editor.bin (PPTY) | `asc_onDocumentContentReady` + canvas >0 + `GET /fonts/` trong log; smoke exit 0 | GREEN (29.09, T-048) |
