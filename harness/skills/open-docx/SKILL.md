# Skill: Open / Save DOCX (đại diện cho XLSX/PPTX/PDF)

> Mục đích: chuẩn hóa luồng "mở file từ đĩa → engine render → lưu về đĩa" trong WasenOffice.
> Dùng như checklist cho cả 4 định dạng; khác biệt từng format ghi ở cuối.

## Luồng chuẩn (target kiến trúc)

```
[User chọn file] (tauri-plugin-dialog)
      ↓ absolute path (Rust)
[Rust: đăng ký session] — cấp fileId, ghi entry vào recents
      ↓ custom protocol: wasenoffice-file://<fileId>/...
[WebView: engine fetch file] — sdkjs mở qua url đó, không biết đường đĩa thật
      ↓ editing trong memory (model JS)
[Save] engine yêu cầu save → shim gom dữ liệu (blob/binary) → invoke Rust
      ↓
[Rust: ghi file] — atomic write (temp + rename), giữ extension đúng
```

## Các bước implement

1. **Custom protocol** `wasenoffice-file://`: serve file từ Rust với đúng MIME; hỗ trợ Range request
   (engine có thể fetch từng phần cho file lớn).
2. **Open flow**: dialog → path → Rust tạo fileId → trả engine một URL protocol trên →
   `DocsAPI.DocEditor` init với document url đó.
3. **Save flow**: bắt callback save của engine qua shim (xem bridge-shim: `LocalFileSave`,
   `LocalFileGetImageUrl`...) → invoke Rust ghi đĩa.
4. **Convert không phải OOXML** (DOC/PPT/XLS/RTF...): gọi x2t sidecar trước khi đưa vào engine —
   engine chỉ ăn OOXML + PDF. Xem skill `x2t-sidecar`.

## Verify

- Mở file mẫu từng format từ `~/Documents` (tạo fixture nếu chưa có).
- Sửa 1 đoạn → save → mở lại bằng bản ONLYOFFICE gốc so sánh không mất format.
- Kiểm tra file đĩa: quyền, không file rác temp sót lại.

## Khác biệt theo format

| Format | Engine module | Đặc thù |
|---|---|---|
| DOCX | `upstream/sdkjs/word` | chuẩn chính của engine — flow trơn nhất |
| XLSX | `upstream/sdkjs/cell` | cẩn thận formula cache + shared strings khi ghi đĩa |
| PPTX | `upstream/sdkjs/slide` | media nhúng lớn → protocol phải support Range |
| PDF | `upstream/sdkjs/pdf` | edit object-level; save có thể cần x2t trợ lực (xem `x2t-sidecar`) |
