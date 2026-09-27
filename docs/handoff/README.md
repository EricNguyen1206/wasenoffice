# docs/handoff/ — quản lý context giữa các phiên agent

> Nhà của handoff: prompt tự chứa cho phiên agent mới (QĐ7). Template: `_TEMPLATE.md`.
> Handoff là **state tạm của một việc đang chạy** — không phải kho tài liệu dài hạn.

## Khi nào VIẾT handoff mới

1. **Mở task vừa/lớn hoặc phase mới** — handoff là prompt triển khai (metadata ghi `T-xxx`).
2. **Phiên phải dừng giữa chừng** — task chưa qua cổng verify xong mà phiên sắp kết thúc.

## Khi nào GHI TIẾP (không viết mới)

- Còn cùng một task → cập nhật trạng thái, việc đã làm / còn lại vào file hiện có.
- Bất biến: **1 task đang mở ≤ 1 handoff** — không sinh file thứ hai cho cùng việc.

## Khi nào XOÁ

- Task/phase **đã đóng** (verify exit 0 + mọi tri thức sinh ra đã về đúng nhà:
  `diagnostics.md` / `reference.map.md` / `vision.md` / TODO ✅) → xoá handoff của task đó
  **trong cùng commit đóng việc** — git history giữ nội dung nếu cần khảo cổ.
- Trước khi xoá: quét file lần cuối — thông tin nào còn giá trị mà chưa có nhà → chuyển về
  nhà đúng (bảng định tuyến `docs/README.md`) rồi mới xoá.
- **Chỉ xoá handoff của task mình đang claim** — không xoá file của session khác (đa-phiên).
- File đang bị doc khác trỏ (CONTRIBUTING, DD-TEST…) → cập nhật pointer đó trước khi xoá.

## Quy ước

- Tên file `<tên-kebab>.md`; metadata: `T-xxx` + phase + ngày.
- Nội dung theo `_TEMPLATE.md` (6 section; done-criteria trước khi code — cổng G2).
- Mọi giả định phải thành chữ trong file — state không được sống duy nhất trong chat.

## File hiện có

| File | Task | Trạng thái |
|---|---|---|
| `ph0.md` | PH-0 spike (T-001 ⏸ chờ user chọn phương án) | đang mở |
