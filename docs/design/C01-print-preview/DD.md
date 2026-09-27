# Detail Design (DD) — C01 · Xem trước khi in (print-preview)

| Trường | Giá trị |
|---|---|
| Mã màn hình | `C01` (nhóm C — màn dùng chung nhiều màn hình) |
| Tên màn hình | `print-preview` |
| Mã cha | — (màn common, không thuộc một màn chính nào) |
| Trạng thái | planned |
| Phase | PH-2 (quy hoạch — chưa có QĐ chi tiết) |
| Cập nhật gần nhất | 2026-09-27 |

Màn hình xem trước bản in trước khi gửi printer. Xếp **cụm C (common)** theo quy tắc registry:
dùng chung cho các editor D/E/P (mọi định dạng đều in) — không là màn con của một màn nào.
Thiết kế chi tiết **chưa viết** — sẽ khởi tạo (copy `_TEMPLATE-DD.md`) khi bắt đầu việc print
của PH-2. Nhóm bridge liên quan: `Print`, `Print_Start/Page/End`, `IsFilePrinting`
(trạng thái STUB — `docs/reference.map.md` § Bridge JS→Native, nhóm Print).
