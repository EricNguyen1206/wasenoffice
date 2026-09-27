---
type: test
---
# Test Plan — A01 · Trang bắt đầu (startpage)

## 0. Metadata

| Trường | Giá trị |
|---|---|
| Mã màn hình | `A01` |
| DD tương ứng | [`DD.md`](DD.md) |
| Phase / điều kiện chạy | PH-2 · chạy khi `src/startpage/` đã triển khai |
| Cập nhật gần nhất | 2026-09-26 |

## 1. Phạm vi & ma trận loại test

- **In scope:** hiển thị recents/recovers, tạo mới 3 loại, mở file, empty state, i18n.
- **Out of scope:** nội dung bên trong editor (màn `editor`), cloud/templates marketplace (PH-3).
- Ma trận: ✅ `E2E manual` (khi có code) · ⬜ `unit` (store recents phía Rust, khi triển khai) ·
  ⬜ `integration` (bridge Recents group) · ⬜ `visual`.

## 2. Môi trường & fixtures

App dev `pnpm tauri dev`; tạo recents mẫu bằng cách mở vài fixture
(`harness/fixtures/make_docx.py`) rồi đóng.

## 3. Test cases

| ID | Ưu tiên | Loại | Truy vết DD | Given / When / Then | Bằng chứng mong đợi | Trạng thái |
|---|---|---|---|---|---|---|
| TC-A01-01 | P0 | E2E manual | §3 FIRST_RUN | Given store recents rỗng / When mở app / Then 3 nút tạo mới + câu hướng dẫn, không danh sách | ảnh chụp; không phần tử recents | N/A (chưa code) |
| TC-A01-02 | P0 | E2E manual | §3 OPENING | When nhấn "Tài liệu mới" / Then mở cửa sổ editor với doc trắng DOCX | cửa sổ editor mở; fileId mới đăng ký | N/A |
| TC-A01-03 | P0 | E2E manual | §3 OPENING | When nhấn dòng recent / Then mở đúng file đó | editor mở + `SetDocumentName` đúng tên | N/A |
| TC-A01-04 | P1 | E2E manual | §4 recents menu | When xoá 1 recent / Then biến mất + `onupdaterecents` bắn | danh sách cập nhật không reload | N/A |
| TC-A01-05 | P1 | E2E manual | §6 file mất | Given recent trỏ file đã xoá / When nhấn / Then báo không tìm thấy + gợi ý remove | thông báo hiển thị; không crash | N/A |
| TC-A01-06 | P1 | E2E manual | §8 i18n | When đổi ngôn ngữ / Then toàn bộ label đổi | ảnh chụp 2 ngôn ngữ | N/A |
| TC-A01-07 | P0 | E2E manual | §7 perf | When cold start không file đối số / Then startpage tương tác được < 1 s | timestamp từ launch tới first paint log | N/A |
| TC-A01-08 | P2 | E2E manual | §8 a11y | When điều hướng thuần bàn phím / Then tới mọi action | thứ tự focus hợp lý, Esc thoát menu | N/A |

## 4. Chính sách hồi quy

Sửa `src/startpage/` hoặc nhóm bridge Recents → chạy lại toàn bộ P0 (TC-A01-01, 02, 03, 07).

## 5. Tiêu chí sign-off

- [ ] Mọi P0 GREEN
- [ ] Không network call nào trong phiên startpage (kiểm tra log filesrv/network)
- [ ] Store recents hoạt động qua restart app

## 6. Changelog

| Ngày | Thay đổi | Commit |
|---|---|---|
| 2026-09-26 | Test plan draft (viết trước theo DD — case N/A đến khi có code) | — |
| 2026-09-27 | Mã hóa màn hình: TC-SP-nn → TC-A01-nn | — |
