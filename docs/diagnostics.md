# docs/diagnostics.md — nhật ký chẩn đoán

Quy tắc: mỗi entry ghi thời điểm, triệu chứng, bằng chứng, trạng thái
(observed / candidate / reproduced / confirmed). Entry mới thêm TRÊN CÙNG.

---

## 2026-09-21 · Bootstrap harness + Tauri scaffold

- **Done (verified):**
  - Repo `nanooffice` init: AGENTS.md + harness (3 rules, 3 skills, verify.sh) + docs.
  - Submodules: `upstream/sdkjs` @ 72b0421c0b (128 MB), `upstream/web-apps` @ 9c0ca538c3 (798 MB) — khớp nhánh v9.4.0.
  - Tauri 2 scaffold (vanilla-ts): `cargo check` ✓ · `pnpm build` ✓ · `./harness/scripts/verify.sh` → ALL CHECKS PASSED.
  - Bridge shim skeleton: `src/shim/desktopEditor.ts` (~60 method known + auto-forward unknown) ↔
    `src-tauri/src/bridge/mod.rs` (dispatch trả `UNIMPL:<method>`), command `asc_desktop_editor`.
- **Việc bị chặn:** xóa `scaffold-tmp/` (thư mục trùng từ create-tauri-app) bị Safety Guard từ chối
  (APPROVAL_SERVICE_UNAVAILABLE_DENIED) trong turn bootstrap → giữ lại, cần user xác nhận xóa.
- **Kế hoạch PH-0 tiếp theo:** clone `desktop-sdk` vào onlyoffice-study (surface native của bridge);
  load `upstream/web-apps/apps/documenteditor` vào WebView Tauri; log mọi bridge method engine gọi khi mở 1 DOCX.

---

## 2026-09-21 · Khởi tạo dự án

- **Sự kiện:** Tạo harness + repo NanoOffice (Tauri scaffold). 
- **Số liệu nền (observed, đo trên máy này):**
  - ONLYOFFICE v9.4.0 arm64: ~1.07 GB RAM idle / 7 processes.
  - WPS Office: ~605 MB idle / 2 processes.
  - Engine JS: sdkjs ~1,36M dòng, web-apps ~780k dòng; core C++ 3,44M dòng.
  - Bridge: ~130 method lower-bound (chi tiết `docs/reference.map.md`).
- **Chưa biết (cần kiểm chứng PH-0):**
  - [ ] sdkjs render ổn định trên WKWebView (canvas perf, font fallback, shortcuts) — candidate: có, vì ONLYOFFICE web chạy Safari.
  - [ ] Surface phía native của bridge (cần clone `desktop-sdk`).
  - [ ] Mức RAM thực tế của Tauri shell so với CEF — candidate 20–40%.
