# docs/conventions.md — quy ước NanoOffice

## Kiến trúc mục tiêu (đã chốt theo feasibility study 21.09.2026)

Lộ trình A: **Tauri shell + engine gốc + x2t sidecar.**

```
src-tauri/ (Rust — mã nguồn chính)
  ├─ bridge/        shim window.AscDesktopEditor: command enum + handlers
  ├─ protocol/      custom protocol nanooffice-file:// (Range support)
  ├─ sidecar/       quản lý x2t (spawn, timeout, queue)
  ├─ window/        cửa sổ, tab, state
  └─ main.rs
src/ (frontend Tauri — KHÔNG chứa engine)
  ├─ startpage/     start page thay loginpage upstream (JS riêng của mình)
  └─ shim/          inject bridge JS vào WebView trước khi engine load
engines/            artifact build sdkjs/web-apps + x2t binary (không edit tay)
upstream/           submodule chỉ đọc
```

## Quy ước code

- **Rust**: edition 2021+, `cargo fmt` + `cargo clippy` sạch trước commit; error handling dùng
  `thiserror` cho lib-layer, `anyhow` chỉ ở main; không `unwrap()` trong handler liên quan UI.
- **TypeScript/JS (src/)**: đặt tên shim khớp method bridge 1-1; mỗi shim module export đúng một
  nhóm chức năng (fileOps, print, clipboard...).
- **Tên method bridge**: giữ NGUYÊN VĂN tên upstream (`IsLocalFile`, `LocalFileSave`...) — không
  rename sang convention riêng, giúp đối chiếu call-sites và rebase đỡ đau.
- **Không** hardcode đường đĩa trong engine-facing code; mọi đường file đi qua fileId + custom protocol.

## Git

- Branch chính: `main`. Nhánh làm việc: `feat/<phase>-<mô tả>` (vd `feat/ph0-spike-editor-load`).
- Commit message: `phase(scope): mô tả` (vd `ph0(bridge): stub file-dialog methods`).
- upstream submodule cập nhật là commit riêng, không trộn với commit code.

## Kiểm chứng mỗi vòng

- `cargo check` (src-tauri) sạch.
- `pnpm build` (src) sạch.
- Có thay đổi bridge/protocol → chạy smoke: mở app, load editor, log các method UNIMPL bị gọi.

## Định nghĩa các phase (ngắn gọn)

- PH-0: Tauri trống load documenteditor + render DOCX trong WKWebView (checkpoint go/no-go).
- PH-1: CRUD file end-to-end (open/save/convert) qua x2t sidecar.
- PH-2: print/clipboard/fonts/tabs/file-association.
- PH-3: crypto, signature, cloud (scope-cut được).
- PH-4: đóng gói + updater.
