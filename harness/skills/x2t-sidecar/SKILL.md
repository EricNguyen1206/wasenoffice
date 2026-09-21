# Skill: x2t Sidecar

> Mục đích: chạy engine convert của ONLYOFFICE (`core` — binary x2t) như tiến trình con của
> app Tauri, phục vụ: convert định dạng legacy (DOC/PPT/RTF…), export PDF, hỗ trợ save PDF.

## Hiện trạng bằng chứng

- x2t binary nằm sẵn trong app ONLYOFFICE đã cài:
  `/Applications/ONLYOFFICE.app/Contents/Resources/converter/x2t` (~186 MB RSS khi chạy, đo 21.09.2026).
- Cách dùng thật: tham khảo repo `ONLYOFFICE/core` → `X2tConverter/build/` (chưa clone; khi cần,
  clone vào `~/.openclaw-autoclaw/workspace/onlyoffice-study/core` đã có source engine — phần build
  script nằm ở repo build_tools).
- x2t nhận config file (XML/JSON tùy version) khai báo: source path, target path, format,
  password, options. **Kiểm chứng bằng thực nghiệm trước khi tin khẳng định nào về CLI args.**

## Luồng chuẩn trong NanoOffice

```
[Rust] cần convert A.doc → A.docx
  → spawn sidecar: engines/x2t (bundle đi kèm app)
  → truyền config file trong thư mục temp của app
  → chờ exit code + kiểm tra sản phẩm tồn tại + non-empty
  → timeout 60s → kill + báo lỗi sạch (không zombie)
```

## Quy tắc

1. Sidecar là **tiến trình con ngắn hạn** — một việc một lần gọi, không daemon lâu dài (tránh
   memory tích tụ; mỗi instance x2t chiếm ~180–400 MB).
2. Hàng đợi convert: tối đa 1 task đồng thời trên mỗi sidecar; cần song song → spawn thêm instance,
   đừng dùng chung.
3. Đóng gói: chọn flavor x2t nhỏ nhất đủ định dạng cần (trim DjVu/HWP/EPUB nếu không dùng —
   bundle converter hiện 323 MB).
4. Log stderr của sidecar → `docs/diagnostics.md` khi lỗi (x2t nói gì là bằng chứng đầu tiên).

## Verify

- Convert DOC→DOCX và DOCX→PDF với file thật; mở sản phẩm bằng ONLYOFFICE gốc kiểm tra không vỡ layout.
- Kill app giữa chừng → không để lại process x2t mồ côi (`pgrep x2t` sau khi thoát app).
