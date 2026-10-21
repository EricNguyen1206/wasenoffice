---
type: design
---
# Detail Design (DD) — T-068 · Web dev mode — HTTP invoke bridge

## 0. Metadata

| Trường | Giá trị |
|---|---|
| Mã | `T-068` (META — harness/dev-infra) |
| Loại nhiệm vụ | dev-infra seam frontend↔Rust — không thuộc màn hình nào; DD nhóm mở `web-dev-mode` |
| Trạng thái | active — 20.10.2026 (user chỉ thị: "phát triển nhanh hơn, web view test được full chức năng, tận dụng ZCode browser-use tối đa") |
| Owner | Eric Nguyen |
| Cập nhật gần nhất | 2026-10-20 |
| Khảo sát | hội thoại 20.10 (browser-use probe start/viewer/editor/settings không Tauri — bằng chứng trong diagnostics) |

## 1. Vấn đề & động lực

Frontend là Vite thuần nhưng mọi hành động thật đi qua Tauri `invoke` — browser thường
chỉ render được vỏ (start page hiện 0 dữ liệu, viewer/editor chết ở top-level). ZCode
browser-use (Chromium) **không attach được vào WKWebView** của Tauri, nên E2E UI hiện
chỉ có pattern tự-lái `WASEN_OFFICE_*_PROBE` (env → click nút thật → grep log).

Phát hiện quyết định: frontend **đã nói HTTP** với Rust cho mọi thứ trừ `invoke` +
quản lý cửa sổ — engine/fonts/media/tài liệu đều serve từ loopback
`http://127.0.0.1:14321` (`filesrv.rs`; `get_engine_base`; `mediaBase`). Custom scheme
chỉ còn trong comment. Thiếu đúng một mắt xích: kênh command.

## 2. Kiến trúc — 3 mảnh

1. **Rust bridge** trong filesrv có sẵn (không dep mới): `POST /invoke/<cmd>` —
   body JSON đúng shape Tauri IPC, header `X-Wasen-Window` mang nhãn tab.
   `web_bridge::dispatch` `match` tên command → gọi **chính các fn command hiện có**
   (`State` qua `app.state()`), trả `{"ok":true,"result"}` hoặc HTTP 200
   `{"ok":false,"error"}` (giữ semantics reject của invoke; 4xx chỉ cho lỗi vận chuyển).
2. **Polyfill** `src/web/tauri-polyfill.js`: Vite tiêm inline `head-prepend` chỉ ở
   `apply: 'serve'` — cài `__TAURI_INTERNALS__`/`__TAURI__` khi (và chỉ khi) trang
   không có IPC thật. **0 sửa call-site**: import ESM đọc lazily, 2 chỗ destructure
   top-level (`viewer.ts:38`/`editor.ts:119`) tự sống vì global tồn tại trước module.
   Kết quả có `webOpen{url}` → `window.open() || location.assign` rồi resolve
   `undefined` (đúng hợp đồng fire-and-forget của 3 command mở cửa sổ).
3. **Vite proxy** `/invoke → 127.0.0.1:14321` (`WASEN_OFFICE_BRIDGE_PORT` override):
   same-origin cho trang → không CORS phía browser; phía Rust vẫn gate Origin.

## 3. "Window label = identity param" — seam duy nhất cần tách

Danh tính tài liệu của viewer/editor đi hoàn toàn qua window label
(`SurfaceHandoff.take(window.label())`, `EditorSessions::session_key(label, tab)`).
Browser không có `window.label()` → tách **impl nhận label tường minh**, wrapper
Tauri thành 1 dòng:

- `viewer_open_impl(label, …)` · `wasen_light_{open,edit,save,close_tab}_impl(label, …)`
- `prepare_quick_view` / `prepare_view` / `prepare_reopen` — nửa không-cửa-sổ của
  `viewer_view`/`wasen_light_view`/`wasen_light_reopen` (validate + mint label +
  stash + recents + log); wrapper mở cửa sổ trên nhãn đó, bridge trả
  `webOpen:/viewer.html?win=<label>` — tab mới mở trang, polyfill đọc `?win=` làm
  nhãn của mình → `viewer_open` phân giải **giống hệt** đường Tauri.
- `open_settings` → `webOpen:/settings.html?win=settings` (nhãn cố định).
- Nhãn tab thường: `web-<uuid8>` (sessionStorage, sống qua reload).

## 4. Bảo mật / release-safety

- **Default OFF**: `WASEN_OFFICE_WEB_BRIDGE=1` mới bật; không bật → `/invoke` = 404
  (unit test `invoke_route_is_absent_when_bridge_off` + gate 21 negative `disabled`).
- Bind loopback (sẵn có) · **Origin allowlist** đúng 2 origin Vite, response echo
  origin (không `*`) · body cap 1 MiB · chỉ dispatch tên có trong `match`.
- Polyfill không vào dist (`apply:'serve'`; counterfactual: `grep WASEN_WEB_MODE dist/` = 0).
- Engine commands (`open_document`/`create_document`) **refuse có message** — không
  để iframe engine treo im lặng.

## 5. Ceilings (phiên bản này)

| Trần | Lý do | Biểu hiện |
|---|---|---|
| Engine editor (sdkjs) | cần chiều Rust→JS (save flow `EndSave` eval vào webview — `bridge/save.rs`) + shim tiêm vào frame engine (Tauri làm via init-script) | `open_document`/`create_document` → lỗi tường minh "use Quick view / Nano Light" |
| Native picker | `plugin-dialog` là panel OS | lỗi "use 'or paste a path…'" — ô path là lối tự động hoá |
| Tab đóng không unregister | không có hook Destroyed phía browser | leak registry/session trong phiên dev (chấp nhận) |
| `window.open` bị chặn popup | chính sách IAB/Chromium | fallback `location.assign` — cùng tab |

## 6. Kiểm thử

- **Gate 21** `harness/scripts/smoke-web-bridge.sh` (thường trụ verify.sh): curl-only,
  không cần browser — `recents_list` 200 → `viewer_view` ra nhãn `quickview-N` →
  `viewer_open` theo nhãn ra `mediaBase`+`parseMs` (**hand-off không cần cửa sổ nào**)
  → `open_document` refuse đúng message → `plugin:app|version` → unknown → 403 origin.
  Negative: `disabled` (404) · `origin` (403). Log contract: `SMOKE_RE_FILESRV` +
  `SMOKE_RE_WEB_BRIDGE` (filesrv.rs spawn — cùng commit khi đổi).
- **E2E browser-use (bằng chứng thủ công, diagnostics 20.10)**: start page 0 ERROR +
  version + recents thật; Quick view render (first-draw 42.8 ms); Nano Light gõ +
  **save ra đĩa** (`NL-WEB-MARKER` trong `word/document.xml`, 3.8 ms); Settings mở
  không lỗi; "Open local file" ra message trần.
- Unit: `origin_allowed` ×3 · `arg` IPC-shape · cờ config · filesrv 404/405.

## 7. Phase 2 (task riêng nếu muốn)

(a) engine render best-effort: tiêm shim vào trang boot engine do filesrv serve;
(b) kênh Rust→browser (SSE/long-poll) + trừu tượng hoá "gửi tới window" trong
`bridge/save.rs` → engine save trong browser; (c) native picker qua
`app.dialog().blocking_pick_file()`; (d) `--web` headless không cửa sổ;
(e) unregister khi tab đóng (`navigator.sendBeacon`).
