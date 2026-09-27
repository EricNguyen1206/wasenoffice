# Detail Design (DD) — D01 · Cửa sổ soạn thảo tài liệu (editor)

## 0. Metadata

| Trường | Giá trị |
|---|---|
| Mã màn hình | `D01` (nhóm D — các màn DOCX) |
| Tên màn hình | `editor` |
| Mã cha | — (màn chính) |
| Trạng thái | in-development |
| Phase | PH-0 (checkpoint go/no-go) |
| Owner | Eric Nguyen |
| Cập nhật gần nhất | 2026-09-27 |
| TEST tương ứng | [`TEST.md`](TEST.md) |

## 1. Tổng quan

Cửa sổ chính của NanoOffice: một WKWebView (Tauri) hosting `web-apps` **documenteditor** +
`sdkjs` engine — UI người dùng thấy là UI upstream nguyên bản. Shell Rust đứng sau cung cấp
3 dịch vụ: (1) filesrv loopback phân phát engine + fonts + document, (2) bridge hai chiều
`window.AscDesktopEditor` (JS→Native) và `on_native_message` (Native→JS), (3) convert x2t
(docx ↔ Editor.bin). Sứ mệnh của màn này với dự án: chứng minh engine AGPL chạy trong WKWebView
không cần CEF (PH-0) và là nền của lớp AI (mỗi bridge method = một tool — QĐ3).

## 2. Bối cảnh & phạm vi

- **Goals:** boot engine đầy đủ trong WKWebView; mở 1 DOCX local render đầy đủ (đích PH-0);
  giữ tên bridge method NGUYÊN VĂN upstream để rebase nhẹ.
- **Non-goals:** save/convert UI (PH-1) · print/clipboard/tabs (PH-2) · crypto/signature/cloud (PH-3,
  cắt được) · sửa bất kỳ file nào trong `upstream/`/`engines/`.
- **Quyết định liên quan:** QĐ1 (engine AGPL giữ nguyên, mọi thay đổi hành vi ở lớp shim/wrapper),
  QĐ3 (bridge chính là agent API — không tạo kênh AI song song), QĐ2 (budget nhẹ — xem §7).

## 3. Luồng người dùng & máy trạng thái

Entry: mở app có sẵn file đối số (PH-0) — sau này từ A01 (startpage)/recents. Exit: đóng cửa sổ.

| Trạng thái | Điều kiện vào | Chuyển tiếp | Dấu hiệu quan sát được |
|---|---|---|---|
| INIT | app mở, wrapper page load | → ENGINE_BOOT | toolbar chưa hiện |
| ENGINE_BOOT | LoadJS chain bắt đầu | → READY_NO_DOC | `sdk-all.js`, `fonts.js`, `ChartStyles.js` eval xong (completeLoad) |
| READY_NO_DOC | engine UI hiện (ribbon/toolbar/canvas hệ thống) | → OPENING khi `LocalStartOpen` | hàng trăm asset HTTP 200 |
| OPENING | shell convert + push Editor.bin | → RENDERING | `SetDocumentName` bắn; `DesktopOfflineAppDocumentEndLoad` chạy không exception |
| RENDERING | parse + styles preload + font pipeline | → READY | styles preload sâu (`II(styleName)` ×56); font fetch `/fonts/…` |
| **READY (đích PH-0)** | render xong | — | `asc_onDocumentContentReady` bắn + `id_viewer` height > 0 |
| ERROR | `asc_onError` / RuntimeError | — | hiện pipeline dừng **im lặng** thay vì báo lỗi (xem §10) |

**Hiện trạng (26.09):** đứng ở RENDERING — parse + styles chạy sâu, không exception, nhưng font
pipeline của FULL build (`AscFonts.Wg` — glyph manager) không khởi động; `id_viewer` 0 height.

## 4. Bố cục & hành vi trực quan

Toàn bộ viewport = editor iframe (boot page wrapper ở main frame). UI layout, ribbon, canvas
do upstream render — PH-0 **không** thay đổi gì UI upstream (rule read-only). Trạng thái
loading do engine tự hiển thị. Phím tắt: kế thừa engine; PH-2 mới xử lý conflict với macOS.

## 5. Thiết kế kỹ thuật

- **Entry code:** `src-tauri/src/main.rs` · `src-tauri/src/window/` · `src-tauri/src/protocol/filesrv.rs`
  · `src-tauri/src/bridge/mod.rs` · `src-tauri/src/sidecar/mod.rs` (x2t, PH-1) · `src/main.ts`
  · `src/shim/desktopEditor.js`.
- **Luồng boot:**
  1. Wrapper page load trong WKWebView; shim inject qua `initialization_script_for_all_frames`
     (main frame → boot page iframe → editor iframe; relay postMessage hai chiều).
  2. Engine statics phân phát qua loopback `http://127.0.0.1:14321` (filesrv: `/editors/…`,
     `/fonts/app|sys/…`, `/file/<id>/…`; Range 206; ACAO `*`; OPTIONS preflight).
  3. `sdk-all-min.js` (bootstrap) gọi shim **`LoadJS` contract**: return 1 sync / 2 async,
     `local_load_add/remove`, `completeLoad` bắn sau eval — nạp `sdk-all.js`, `fonts.js`, `ChartStyles.js`.
  4. `window.Module.locateFile` (map theo basename) + filesrv `resolve_engine_payload` redirect
     → `fonts.wasm` init đúng vị trí (xem bẫy §6).
- **Luồng open (normative — chi tiết bằng chứng: `docs/reference.map.md` § Desktop open contract):**
  1. Engine boot với document url `_offline_` → gọi `LocalStartOpen`.
  2. Shell chạy x2t: docx → `Editor.bin` (magic `DOCY`) + `media/`.
  3. Shell push: `LocalFileSetSaved` → `LocalFileRecoverFolder(folder)` → `LocalFileSetSourcePath(path)`
     → `window.DesktopOfflineAppDocumentEndLoad(folder, base64, decodedLen)`.
  4. Engine: `zP.decode` → `f$c("DOCY")` → parse → fonts → render → `asc_onDocumentContentReady`.
- **Interface tiêu thụ:** bảng trạng thái đầy đủ ở `docs/reference.map.md` § Bridge JS→Native
  (DONE 15+ method, `LocalStartOpen` PARTIAL, ~115 STUB log `UNIMPL` khi bị gọi) và § Native→JS
  (router `on_native_message` 13 cmd; **nghĩa vụ app-layer trả lời `editor:config` request** —
  `Desktop.js:141` — nghi vấn blocker, xem §10).
- **Quản lý trạng thái:** Rust giữ map fileId → disk path (đường đĩa không bao giờ ra JS);
  sync-truth table phía shim cho method engine gọi đồng bộ (`IsLocalFile`, `GetEncryptedHeader`…).
- **Điểm chạm upstream:** artifact engine trong `engines/editors/` (AGPL, git-ignored, không sửa);
  mọi nhận định về hành vi upstream phải có path (rule gốc #3).

## 6. Xử lý lỗi & ca biên

| Tình huống | Hành vi mong đợi | Bằng chứng / bẫy đã gặp |
|---|---|---|
| Response body bị nhân đôi NUL | unit test hồi quy chặn | diagnostics 22.09 (bẫy 1) |
| Document URL scheme lạ | engine allowlist `https?\|ftps?\|file\|tessa\|joplin\|smb` — `nanooffice-file://` bị Invalid → mọi thứ cùng origin loopback | `editorscommon.js:2939` (reference.map § Ràng buộc URL) |
| WASM resolve sai thư mục | `locateFile` + redirect của filesrv | RuntimeError Aborted **im lặng** — diagnostics 22.09 (bổ sung) |
| Mixed-content | engine + document cùng origin `127.0.0.1:14321` | custom-scheme page fetch `http://` bị chặn |
| `include_str!` đổi file JS không rebuild | `touch`/build tay khi sửa payload nhúng | diagnostics 22.09 (bẫy 4) |
| Pipeline dừng không báo lỗi | HIỆN TRẠNG PH-0 — không `asc_onError`, không ready | diagnostics 22.09 (bổ sung) — câu hỏi mở §10 |

## 7. Hiệu năng & ràng buộc

Baseline debug (diagnostics 22.09, chưa phải release): shell binary 30 MB (debug) · engines
payload 313 MB (full copy, chưa trim per-app) · RSS idle 133 MB · cold start → start page 1–2 s (dev).
Đối chiếu ONLYOFFICE cài sẵn: 1,1 GB disk, ~1,07 GB RAM / 7 processes. x2t convert 28–116 ms
(fixture 1,9 KB). Budget đích: `docs/vision.md` QĐ2 — chốt số release ở PH-4.

## 8. Trợ năng & đa ngôn ngữ

PH-0 kế thừa nguyên trạng engine (query `lang` của boot page). Chưa có layer trợ năng riêng —
câu hỏi mở mức thấp, xử lý PH-2.

## 9. Phương án đã loại (Alternatives)

| Phương án | Vì sao loại | Bằng chứng |
|---|---|---|
| Shell CEF (như upstream) | sdkjs/web-apps không có dependency CEF; WKWebView boot đầy đủ | reference.map § CEF-independence (grep 0 match + runtime) |
| Native graphics cho render | app thật cũng render bằng canvas trong webview | CDP oracle: `window.native` undefined, canvas 2654×1312 (diagnostics 22.09 bổ sung) |
| `nanooffice-file://` làm document URL | engine đánh Invalid theo allowlist scheme | reference.map § Ràng buộc URL |
| Dựng `DocsCoApi`/docsco cho word | word build không chứa DocsCoApi — socket polling đến từ web-apps (Gs) | diagnostics 22.09 (bổ sung) |

## 10. Câu hỏi mở

- [ ] **P0 blocker:** font pipeline FULL build (`AscFonts.Wg`) không khởi động sau EndLoad —
  3 phương án chờ user chọn: (a) oracle-diff trạng thái `AscFonts.Wg` app thật vs của ta;
  (b) Editor.bin từ app thật (không giải quyết render); (c) reverse `AscFonts.Wg` trong full
  build. Phương án dự phòng 2 đã sửa thành reverse `AscCommon.Gs` (không phải docsco).
  Chi tiết: `docs/diagnostics.md` entry 22.09 (bổ sung).
- [ ] `editor:config` answer từ app-layer có phải đồng phạm của điểm treo? (Desktop.js:141)
- [ ] Dài hạn (PH-1): giữ loopback `127.0.0.1:14321` hay chuyển hẳn `nanooffice-engine://`
  sau khi có sidecar — deviation kiến trúc cần chốt.

## 11. Changelog

| Ngày | Thay đổi | Commit |
|---|---|---|
| 2026-09-26 | DD đầu tiên — chốt open-pipeline + boot-flow từ diagnostics/reference.map thành spec | — |
| 2026-09-27 | Mã hóa màn hình (QĐ6 bổ sung): editor → D01; tham chiếu startpage → A01 | — |
