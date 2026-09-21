# Skill: Bridge Shim

> Mục đích: thêm / kiểm tra method trên bridge `window.AscDesktopEditor` một cách nhất quán,
> giữ hợp đồng JS↔Rust không bị trôi.

## Bối cảnh kiến trúc

- Engine JS gọi native qua: `window.AscDesktopEditor.<Method>(...)` (~130 method, lower bound,
  xem `docs/reference.map.md` § bridge).
- Native gọi JS qua: router `window.on_native_message(cmd, param)` (13 cmd đã biết) +
  `RendererProcessVariable` (object env: theme, lang, rtl, printer…).
- Trong Tauri: shim JS inject lúc init WebView → chuyển tiếp qua `window.__TAURI__.invoke/event`
  → Rust handler ở `src-tauri/src/bridge/`.

## Quy trình thêm method mới

1. Tra call-site trong `upstream/` (chỉ đọc): xác nhận **signature** (tham số, kiểu trả về, sync/async)
   và **tần suất gọi** (method nào editor cần ngay khi mở file — ưu tiên cao).
2. Thêm entry vào `src-tauri/src/bridge/mod.rs`:
   - Trait/enum command với tên GIỐNG HỆT method JS (case-sensitive).
   - Chưa implement thật → body trả `Err(Unsupported)` + marker `// UNIMPL: <lý do>`.
3. Thêm stub phía JS ở shim (file shim của `src/`, KHÔNG sửa `upstream/`):
   method tồn tại, chuyển tiếp invoke, giữ đúng async semantics engine mong đợi.
4. Ghi mapping vào `docs/reference.map.md`: method → file Rust → trạng thái
   (STUB | PARTIAL | DONE) → call-sites chính.
5. Verify: script harness gọi method qua WebView console, đối chiếu hành vi với
   bản ONLYOFFICE thật (đã cài trên máy) khi có nghi ngờ.

## Định nghĩa xong

- `DONE` = engine gọi method này trong flow thật (mở/save file) mà không lỗi + hành vi khớp bản gốc.
- Gỡ marker `// UNIMPL` khi và chỉ khi DONE.

## Điểm thường gãy (tra trước khiDebug)

- Sync-vs-async: một số call-site engine kỳ vọng trả về NGAY (sync) — invoke của Tauri là async;
  cần shim pattern phù hợp (xem call-site cụ thể trước khi chọn pattern).
- Callback một lần vs nhiều lần (progress download, print pages) — dùng event channel riêng, đừng nhét vào return.
- File path: engine làm việc qua URL/blob — Rust trả path tuyệt đối qua custom protocol, không nhét bytes base64 vào JS string lớn.
