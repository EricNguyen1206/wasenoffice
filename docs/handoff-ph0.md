# Handoff — bắt đầu PH-0 (dành cho session mới)

> Bạn (agent) đang ở phiên mới. File này LÀ prompt công việc: đọc nó + các file nó trỏ tới
> theo thứ tự, rồi triển khai PH-0. Đi tuần tự, không nhảy cóc.

## Nhiệm vụ

**PH-0 — checkpoint go/no-go của NanoOffice**: load documenteditor vào WebView Tauri và render
1 DOCX thật. Nếu render fail/chập chờn trong WKWebView (canvas perf, font fallback, shortcuts),
toàn bộ lộ trình phải được xem lại trước khi đi tiếp — đây là nhiệm vụ quan trọng nhất của
dự án lúc này.

## Bước 0 — đọc trước khi làm bất cứ gì (theo thứ tự)

1. `AGENTS.md` — contract của dự án.
2. `docs/vision.md` — định vị + 4 quyết định (MIT purity, budget lightweight, AI 3 lớp, platform).
3. `docs/conventions.md` — quy ước, chế độ đơn nhiệm, workflow skills, checklist.
4. `docs/reference.map.md` — bản đồ bridge ~130 method (observed) + bằng chứng CEF-independence.
5. `harness/rules/mit-purity.md` + `harness/skills/bridge-shim/SKILL.md` + `harness/skills/open-docx/SKILL.md`.
6. `docs/diagnostics.md` — 5 entry mới nhất (số liệu nền, các bẫy đã gặp).
7. `src/shim/desktopEditor.ts` (KNOWN_METHODS ~60 method) + `src-tauri/src/bridge/mod.rs` (dispatch UNIMPL).

## Trạng thái repo lúc handoff (22.09.2026)

- Branch `master`, working tree **SẠCH** — toàn bộ nền móng (license MIT, rules MIT-purity,
  skills workflow, gates verify.sh, System-1 stack, handoff này) đã commit trong 5 commit
  `b60df56..79c4fb0`. → **Đầu phiên vào việc PH-0 luôn**, không cần commit dọn dẹp;
  commit mới theo `ph0(scope): mô tả`, tách docs/harness khỏi code về sau.
- Services AI: ollama :11434 UP; llama-server/semif-engine/system1-router DOWN sau reboot —
  `harness/system1/start.sh` bật cả stack khi cần (không bắt buộc cho PH-0).
- `onlyoffice-study/` KHÔNG còn ở vị trí cũ sau khi repo chuyển về
  `/Users/ericnguyen/DATA/Backend/rust/nanooffice` — cần định vị lại hoặc clone mới (bước 1).

## Kế hoạch PH-0 (5 bước, tuần tự — chế độ đơn nhiệm)

1. **Clone `ONLYOFFICE/desktop-sdk`** vào thư mục study NGOÀI repo (không bao giờ vào repo) —
   nguồn chân lý native surface của bridge, ưu tiên 1 theo diagnostics 21.09.
2. **Quyết định cách phục vụ engine** (điểm quyết định đầu tiên; đề xuất phương án b):
   - a. Build từ source qua `build_tools` — chưa clone, nặng, nhiều giờ.
   - b. **(đề xuất cho spike)** Trích artifact engine đã build từ ONLYOFFICE.app cài sẵn
     (`/Applications/ONLYOFFICE.app/Contents/Resources/`) vào `engines/` — thư mục này đã
     git-ignored, artifact AGPL KHÔNG bao giờ vào git (mit-purity điều 4); quyết định phân phối
     thật là câu hỏi mở PH-4 trong `vision.md`.
3. **Implement custom protocol `nanooffice-file://`** (Range support, fileId→path map trong
   Rust state) — `src-tauri/src/protocol/mod.rs` hiện chỉ là doc-comment.
4. **Load documenteditor vào WebView + mở 1 DOCX thật** (dev bằng `pnpm tauri dev`, devUrl
   :1420): log TẤT CẢ bridge method engine gọi → đối chiếu KNOWN_METHODS → implement first wave
   thật (IsLocalFile, CreateEditorApi, LoadFontBase, GetImageBase, LocalFileSave(D)…) theo đúng
   skill bridge-shim: tên method GIỮ NGUYÊN văn bản upstream, marker `// UNIMPL` chỉ gỡ khi chạy
   thật, cập nhật `reference.map.md` STUB→PARTIAL/DONE cho từng method.
5. **Đo baseline** disk / RAM idle+1 doc / cold-start của NanoOffice vs ONLYOFFICE cài sẵn
   (~1.07 GB RAM / 7 processes, đo 21.09) → ghi `docs/diagnostics.md`, dùng chốt số budget
   trong `vision.md` Quyết định 2.

## Ràng buộc (không thương lượng)

- Repo: `/Users/ericnguyen/DATA/Backend/rust/nanooffice` — luôn dùng đường dẫn tuyệt đối này.
- `upstream/**` chỉ đọc; submodules pinned sdkjs@72b0421c0b / web-apps@9c0ca538c3 (v9.4.0) —
  KHÔNG update submodule trong PH-0. KHÔNG copy code/text upstream vào code mình (MIT purity) —
  cần logic thì re-implement từ spec (ECMA-376/ISO 29500) hoặc behavior quan sát được.
- Mọi nhận định về upstream phải có path làm bằng chứng → ghi `docs/reference.map.md`.
- Nếu spawn subagent → prompt PHẢI nhúng/trỏ `harness/rules/mit-purity.md`.
- Trước khi tuyên bố xong: `./harness/scripts/verify.sh` exit 0 (lần chạy đầu sau reboot lâu do
  clippy rebuild) + code-review trên diff; báo cáo kèm lệnh kiểm chứng + exit code.
- Commit: `ph0(scope): mô tả`; KHÔNG trộn submodule update vào commit code.
- Giao tiếp với user bằng tiếng Việt.

## Định nghĩa hoàn thành PH-0

- DOCX render + scroll/edit cơ bản ổn trong WKWebView, không crash canvas/font.
- Bảng bridge methods đã gọi: engine gọi bao nhiêu / DONE bao nhiêu, còn lại UNIMPL nào.
- Baseline numbers ghi vào `docs/diagnostics.md`.
- Verdict **go/no-go** rõ ràng kèm bằng chứng. Nếu NO-GO: dừng, ghi diagnostics, đợi user
  quyết — KHÔNG tự ý đổi kiến trúc.
