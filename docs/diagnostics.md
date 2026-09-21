# docs/diagnostics.md — nhật ký chẩn đoán

Quy tắc: mỗi entry ghi thời điểm, triệu chứng, bằng chứng, trạng thái
(observed / candidate / reproduced / confirmed). Entry mới thêm TRÊN CÙNG.

---

## 2026-09-21 · Bootstrap harness + Tauri scaffold

- **Done (verified):**
  - Repo `nanooffice` init: AGENTS.md + harness (3 rules, 3 skills, verify.sh) + docs (reference.map, conventions, diagnostics).
  - Submodules chỉ-đọc: `upstream/sdkjs` @ 72b0421c0b (128 MB), `upstream/web-apps` @ 9c0ca538c3 (798 MB) — khớp v9.4.0.
  - Tauri 2 scaffold (vanilla-ts) đã rename thành `nanooffice`: `cargo check` ✓ · `pnpm build` ✓ ·
    `./harness/scripts/verify.sh` → **ALL CHECKS PASSED**.
  - Bridge shim skeleton: `src/shim/desktopEditor.ts` (~60 method known + auto-forward method lạ)
    ↔ `src-tauri/src/bridge/mod.rs` (dispatch trả `UNIMPL:<method>`), Tauri command `asc_desktop_editor`.
  - Commits: `f5b6f69` (bootstrap) → `e070cb1` (sync harness + fix TS/Cargo).
- **Việc bị chặn (cần user):**
  - `scaffold-tmp/` — thư mục trùng từ create-tauri-app, còn nằm trong working tree VÀ đã bị track
    trong commit `f5b6f69` (35 file). Việc xóa file + `git rm --cached` bị Safety Guard từ chối
    2 lần trong turn bootstrap (APPROVAL_SERVICE_UNAVAILABLE / APPROVAL_TIMEOUT) — đúng quy trình,
    không retry trong turn.
  - Cần user tự chạy: `rm -rf scaffold-tmp && git rm -r --cached scaffold-tmp && git commit -m "chore: drop scaffold-tmp"`
    hoặc cấp approval cho lần sau.
- **Kế hoạch PH-0 tiếp theo:**
  1. Clone `ONLYOFFICE/desktop-sdk` vào onlyoffice-study (surface native của bridge — ưu tiên 1).
  2. Serve `upstream/web-apps/apps/documenteditor` qua dev server, load vào WebView Tauri.
  3. Mở 1 DOCX thật, log mọi bridge method engine gọi → đối chiếu KNOWN_METHODS, implement first wave.

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
