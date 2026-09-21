# Handoff — phân tích techstack shell: Bun vs Golang vs Rust (dành cho agent khác)

> Bạn là agent phân tích ĐỘC LẬP. Nhiệm vụ: đánh giá Bun / Golang / Rust cho lớp shell của
> NanoOffice và khuyến nghị rõ ràng. Bạn KHÔNG sửa code — đầu ra là MỘT file phân tích.

## Bối cảnh (đọc trước khi làm gì cả, theo thứ tự)

1. `AGENTS.md` — contract dự án.
2. `docs/vision.md` — định vị + 4 quyết định. Lưu ý: kiến trúc hiện chốt là **Tauri (Rust)
   shell + engine JS gốc + x2t sidecar**, budget lightweight là số đo được, mobile ở H4.
3. `docs/conventions.md` — quy ước, đơn nhiệm, checklist, System-1 accounting.
4. `docs/reference.map.md` — bridge ~130 method (không đổi theo ngôn ngữ shell).
5. `docs/diagnostics.md` — các entry mới nhất (số liệu nền: ONLYOFFICE ~1.07GB RAM; stack hiện tại).
6. Code shell hiện tại: `src-tauri/src/**` (~vài trăm LOC: bridge stub, protocol/sidecar/window
   còn doc-comment) + `src/shim/desktopEditor.ts`. Hãy TỰ ĐẾM LOC thật (`git ls-files 'src-tauri/src/*.rs' | xargs wc -l`)
   — con số này quyết định **migration cost gần bằng 0 nếu đổi ngôn ngữ NGAY BÂY GIỜ**, và sẽ
   tăng dần sau PH-0/PH-1. Phân tích này là time-sensitive: quyết định cần chốt trước PH-1.

## Phạm vi phân tích — tách bạch 3 lớp (KHÔNG gộp)

- **Lớp A — Shell/WebView host**: cửa sổ, tab, custom protocol `nanooffice-file://` (Range),
  file dialog, print, updater. Engine JS (sdkjs/web-apps) và x2t (C++) KHÔNG đổi dù chọn gì —
  chúng là AGPL component tách biên. Frontend TS/Vite cũng không đổi.
- **Lớp B — Sidecar/process layer**: spawn x2t (timeout, queue, stderr-as-evidence), sau này
  AI runtime local. Process management + IPC.
- **Lớp C — Dev tooling/runtime**: pnpm/vite scripts, scripts trong harness — Bun có thể thay
  Node ở lớp này BẤT KỔ shell là gì. Đừng lẫn lớp C vào kết luận lớp A.

Ứng viên: **Bun** (runtime JS), **Golang** (+ ecosystem webview shell như Wails — kiểm chứng
thực trạng mobile/production 2026), **Rust** (+ Tauri v2 — đang dùng, pin trong repo).

## Tiêu chí chấm (weight theo vision.md — tự tính điểm, ghi rõ rubric)

1. **MIT-license readiness** (bắt buộc trả lời trực tiếp): runtime/toolchain license của từng
   ứng viên; rủi ro license ecosystem dependency (npm vs crates vs Go modules) + công cụ gate
   có sẵn (repo ĐÃ có gate cargo+node trong `harness/scripts/check-licenses.mjs` — đánh giá
   chi phí dựng tương đương cho Go/Bun); AGPL engine tách biên có bị ảnh hưởng bởi lựa chọn
   shell không?
2. **Ultra lightweight (số đo được, không tin marketing)**: binary/installer size, RAM idle,
   cold start — MỤC TIÊU: shell đóng góp càng ít càng tốt vào trần <250MB RAM / <2s start
   (vision Quyết định 2). Lưu ý bẫy: latency app do engine JS + webview quyết định, shell chỉ
   là overhead — đừng so benchmark tổng quát vô nghĩa, hãy đo đúng "shell overhead".
3. **Multi-platform**: macOS/Windows/Linux + mobile (H4). Trạng thái THẬT 2026 của mobile:
   Tauri v2 iOS/Android, Wails mobile?, Bun trên mobile? — mỗi ý phải có nguồn.
4. **Độ chín của ecosystem webview-shell**: mỗi ứng viên có path production-ready nào để host
   WebView + custom protocol + updater trên 3 OS không? (Tauri/Wails/… tự kiểm chứng, đừng
   theo tài liệu stale — check release notes/repo hoạt động 6 tháng gần nhất.)
5. **Agent-velocity** (dự án do AI agent phát triển, 1 người): vòng lặp compile/lint/test
   — ĐO THẬT trên máy này: `time (cargo check|go build|bun build)` hello-world tương đương;
   chất lượng LSP/clippy-analogs; khả năng agent sửa code an toàn (type/memory safety).
6. **Migration cost nếu đổi khỏi Rust**: đếm LOC thật, liệt kê phần harness dính Rust
   (verify.sh gates, check-licenses.mjs — phần nào phải viết lại).

## Phương pháp bắt buộc (evidence-discipline của dự án)

- Mỗi nhận định gắn nhãn **observed** (đo trên máy này) / **candidate** (nguồn bên ngoài, ghi
  link) / **unknown** (không đủ dữ kiện — nói thẳng). Cấm treating candidate as observed.
- Cho phép spike trong `/tmp` (KHÔNG trong repo): hello-world webview app cho từng stack khả
  thi (ít nhất Rust+Tauri vì toolchain đã có; Go/Bun nếu cài nhanh được), đo binary size + RSS
  idle + cold start, ghi lệnh đo + số. Không cài được thì ghi observed="không đo được, lý do".
- So sánh công bằng: mỗi stack ở cấu hình release tối ưu hợp lý (Rust: lto/strip đã config sẵn
  trong `src-tauri/Cargo.toml`).

## Ràng buộc

- KHÔNG sửa `src-tauri/src/**`, `src/**`, harness — chỉ đọc. File duy nhất được tạo:
  `docs/techstack-analysis.md`.
- `upstream/**` chỉ đọc. Không commit gì — để user quyết.
- Báo cáo cho user bằng tiếng Việt; nội dung file phân tích viết tiếng Anh hoặc tiếng Việt
  nhất quán (đề xuất tiếng Việt cho nhất quán với docs/).
- Nếu dùng quick_decision_router cho triage → cuối báo cáo kèm dòng System-1 accounting
  (docs/conventions.md); không dùng thì `System-1: 0 calls`.
- Không tự ý đổi kiến trúc dù phân tích ra kết quả gì — quyết định cuối là của user.

## Định nghĩa hoàn thành

`docs/techstack-analysis.md` chứa đủ:
1. Decision matrix: 3 ứng viên × 6 tiêu chí, điểm + bằng chứng (observed/candidate + nguồn).
2. Trả lời THẲNG câu hỏi MIT: "ngôn ngữ nào giúp đạt MIT-app tốt nhất?" — gồm phần nói rõ
   điều nào trong MIT goal là thuộc ngôn ngữ, điều nào KHÔNG (đã được bảo đảm ở lớp quy trình).
3. **Khuyến nghị xếp hạng rõ** (1 câu đầu súc): giữ Rust / chuyển Go / chuyển Bun / hybrid
   (vd Bun cho lớp C) + lý do 3 dòng.
4. Migration cost nếu chọn phương án khác Rust (LOC, gate phải viết lại, rủi ro).
5. Phần "điều gì phải đúng để ứng viên thua cuộc tiếp tục thắng" — falsifiability.
6. Verdict khuyến nghị user: CHỐT giữ nguyên kiến trúc hiện tại, hay mở phiến PH-0 theo
   shell mới — nhưng KHÔNG tự thực hiện việc đổi.
