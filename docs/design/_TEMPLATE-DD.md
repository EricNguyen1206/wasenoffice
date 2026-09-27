---
type: template
---
# Detail Design (DD) — <Tên màn hình>

<!-- Copy file này vào docs/design/[Mã]-[tên]/DD.md và điền; SAU KHI COPY đổi `type: template`
     thành `type: design`. Không xoá section — mục nào chưa dùng thì ghi "N/A (<lý do ngắn>)".
     Đọc quy tắc tại docs/design/README.md (registry + cấp mã) trước khi viết. -->

## 0. Metadata

| Trường | Giá trị |
|---|---|
| Mã màn hình | `<Chữ><2 số>[-n]` — vd `D01`, `D01-1` (khớp thư mục + dòng registry) |
| Tên màn hình | `<tên-kebab>` (đứng cạnh mã trong tên thư mục) |
| Mã cha | `<mã>` nếu là màn con — `—` nếu màn chính |
| Trạng thái | planned / draft / in-development / stable / deprecated |
| Phase | PH-x |
| Owner | `<người phụ trách>` |
| Cập nhật gần nhất | YYYY-MM-DD |
| TEST tương ứng | `TEST.md` (cùng thư mục — sau khi copy) |

## 1. Tổng quan

1–2 đoạn: màn hình này là gì, phục vụ hành vi chính nào của người dùng, vị trí trong app.

## 2. Bối cảnh & phạm vi

- **Goals:** …
- **Non-goals:** … (việc rõ ràng KHÔNG làm ở màn này — thuộc phase/màn khác)
- **Quyết định liên quan:** `docs/vision.md` QĐ… (điều số + một câu vì sao liên quan)

## 3. Luồng người dùng & máy trạng thái

- **Entry point(s):** từ đâu vào màn. **Exit:** ra đâu, điều kiện gì.
- Bảng trạng thái:

| Trạng thái | Điều kiện vào | Chuyển tiếp | Điều kiện/dấu hiệu ra |
|---|---|---|---|

## 4. Bố cục & hành vi trực quan

- Đặt tên từng vùng layout (kèm ảnh chụp/sơ đồ khi có UI thật).
- Hành vi resize, phím tắt riêng của màn, luật hiển thị loading/empty/error cho từng vùng.

## 5. Thiết kế kỹ thuật

- **Entry code:** đường dẫn file chính của màn.
- **Luồng dữ liệu:** các bước tuần tự (hoặc sequence ngắn) — input từ đâu, qua lớp nào, ra đâu.
- **Interface tiêu thụ:** bridge methods / custom protocol / events — **link** `docs/reference.map.md`
  (không copy bảng); nêu rõ nhóm method màn này cần và trạng thái DONE/PARTIAL/STUB.
- **Quản lý trạng thái:** state sống ở đâu (Rust hay JS), ai đồng bộ, khi nào invalid.
- **Điểm chạm upstream:** path bằng chứng + ranh giới được phép (chỉ đọc; hành vi đổi ở lớp shim/wrapper).

## 6. Xử lý lỗi & ca biên

| Tình huống | Hành vi mong đợi | Bằng chứng / bẫy đã gặp (link `diagnostics.md`) |
|---|---|---|

## 7. Hiệu năng & ràng buộc

Budget áp dụng (link `docs/vision.md` QĐ2 khi có số), số đo hiện có, yêu cầu riêng của màn.

## 8. Trợ năng & đa ngôn ngữ

Thao tác được bằng bàn phím không? i18n strings nằm đâu? Yêu cầu tương phản/kích thước nếu có.

## 9. Phương án đã loại (Alternatives)

| Phương án | Vì sao loại | Bằng chứng |
|---|---|---|

## 10. Câu hỏi mở

- [ ] … (mỗi câu gán phase hoặc link entry `diagnostics.md` tương ứng)

## 11. Changelog

| Ngày | Thay đổi | Commit |
|---|---|---|
