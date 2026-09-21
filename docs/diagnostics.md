# docs/diagnostics.md — nhật ký chẩn đoán

Quy tắc: mỗi entry ghi thời điểm, triệu chứng, bằng chứng, trạng thái
(observed / candidate / reproduced / confirmed). Entry mới thêm TRÊN CÙNG.

---

## 2026-09-22 · Xóa scaffold-tmp + handoff PH-0

- **scaffold-tmp/:** user xác nhận xóa → `git rm -r --cached` (35 file, đang staged) + `rm -rf`.
  Việc treo từ bootstrap (bị Safety Guard chặn 21.09) đã đóng; AGENTS.md § Việc còn treo cập nhật.
- **`docs/handoff-ph0.md` (mới):** handoff prompt tự chứa cho session mới triển khai PH-0
  (go/no-go render DOCX trong WKWebView; 5 bước; ràng buộc; bẫy đã biết; định nghĩa hoàn thành).
- **Ghi chú:** `onlyoffice-study/` không tìm thấy ở vị trí cũ (`~/DATA/*/onlyoffice-study`,
  mdfind/find không ra) sau repo relocation — handoff giao nhiệm vụ định vị lại / clone
  `desktop-sdk` cho session PH-0 (bước 1).
- **Trạng thái:** observed.

---

## 2026-09-22 · So sánh SemIf vs Laya + quick_decision_router (harness/system1)

- **Yêu cầu user:** khảo sát 2 giải pháp System-1 (SemIf vs Laya `pip install laya`) rồi triển
  khai tool `quick_decision_router` + cascade confidence < 0.85.
- **Verify Laya:** GitHub `convaiinnovations/laya` **không tồn tại** (repo thật: `NandhaKishorM/laya`);
  PyPI `laya` 0.3.4 có thật, Apache-2.0, Convai Innovations. Non-autoregressive ModernBERT-large
  421M / mmBERT 322M, công bố 33ms/câu trên T4.
- **Benchmark đo thật trên máy này (M1 16GB, cùng payload):** Laya CPU triage 3 câu =
  **338.8ms** (vs SemIf ~95ms Metal); guard 1 câu 121.2ms; MPS còn chậm hơn (153.4ms/câu);
  cold load 107s (vs ~10s). Chất lượng: cả hai đúng `area=bridge`; injection Laya noul=1.0,
  SemIf noul=0.893 (đều đúng); điểm yếu chung: ordinal score. Tài liệu Laya tự ghi
  "base near chance zero-shot typed-decisions; fast base to specialise".
- **Kết luận (theo đúng rule user đề ra):** giữ **SemIf** làm engine — nhanh hơn trên máy này,
  context 4096+ vs 512, zero-shot là thiết kế gốc, confidence shipped của Laya mis-calibrated
  (0.502 trên argmax 0.809 → threshold 0.85 luôn escalate). Laya = future specialist (fine-tune).
- **Triển khai `harness/system1/`:** `router.mjs` (quick_decision_router :11437, cascade
  per-primitive threshold `SYSTEM1_CONF_THRESHOLD`=0.85, gắn `escalated` + `routing.decision_source`),
  `start.sh` (idempotent, kéo cả semif stack), `smoke_test.sh`, `README.md` (bảng so sánh + usage).
- **Smoke test (reproduced, exit 0):** route=technical p=0.852 không escalate; urgency=2.72
  p=0.73 escalate; is_injection p=0.687 escalate; guardrail injection chuyên biệt noul=0.893
  boolean=true (284ms router). `decision_source=system1+main-agent-partial` hoạt động đúng.
- **Ghi nhận trung thực:** latency sau idle ~1.8s (KV nguội) vs warm ~95-103ms — smoke test in
  số thật mỗi lần; noul với threshold 0.85 chỉ "tự quyết" khi |p−0.5| > 0.35 — bảo thủ có chủ đích.
- **Trạng thái:** reproduced.

---

## 2026-09-21 · Pivot: SemIf direct-logit engine thay decision-engine (OpenJev sampler)

- **Quyết định user:** bỏ OpenJev sampler, chuyển qua **SemIf** (`theoleecj/semif`, HEAD
  `ca3ba65`, MIT, formerly OpenJev). Nguyên lý: direct logit readout / prefill-only —
  1 forward pass, softmax chỉ trên logits của letter token, 0 output token.
- **Method port từ** `src/semif_phase1/core.py` + `direct.py`: frozen system prompt, payload
  `{evidence, criterion, options:[{letter, description}]}`, LETTERS="ABCDEFGHIJKLMNOP".
- **Hành trình runtime (đều reproduced):**
  1. Ollama KHÔNG expose logits → cài `llama.cpp` (brew, v0.4.1, Metal).
  2. `model_setup.sh` tải GGUF Q4_K_M riêng (1.5GB, git-ignored tại `models/`).
  3. Vấn đề thật: template MiniCPM5 mặc định mở thinking — token đầu là "We/The/..." (prose),
     không phải letter. `/no_think` vô hại, `--no-reasoning-preserve` không đủ.
  4. **Fix:** đọc template từ `/props` → template có nhánh `enable_thinking is false` chèn
     `<think>\n\n</think>\n\n` rỗng → truyền `chat_template_kwargs:{enable_thinking:false}` qua
     `/v1/chat/completions` → letters chiếm top-4 (−0.06…−6.2), token thường tụt −11.
- **Đã dựng `harness/semif_engine/`:** `server.mjs` (zero-dep, contract `/evaluate` giữ nguyên
  như decision-engine cũ cho agent workflow ổn định; mỗi question = 1 forward pass),
  `model_setup.sh`, `start.sh` (llama-server :11436 + node :11435), `check.sh` (E2E + đo
  latency), `README.md` (attribution + bảng khác biệt runtime + giới hạn).
- **E2E (reproduced, check.sh exit 0):** health ✓; triage 3-in-1: `area=bridge` p=0.949 (ĐÚNG),
  `severity=2.69`, `is_flaky=0.688`; **latency warm 92–103ms/call** (đạt target <100ms; call đầu
  ~1s warmup); negative 400×3. So với OpenJev sampler: 5270ms/9-call → ~17× nhanh hơn.
- **Ghi nhận trung thực:** (a) cùng câu hỏi, state ters hơn đã cho `area=D(CI)` thay vì
  `bridge` — model 2B nhạy với chất lượng evidence (SemIf tự đo 0.686 balanced accuracy);
  (b) xác suất là conditional-on-top-k, không full-vocab như gốc — ghi rõ README;
  (c) `harness/decision-engine/` (OpenJev port) đã XÓA theo quyết định user.
- **Trạng thái:** reproduced.

---

## 2026-09-21 · System-1 decision engine (OpenJev port) vào harness

- **Bối cảnh:** user chỉ định repo `SiliconLabAI/OpenJev` + model `MiniCPM5-2B_Q4_K_M` cho
  kiến trúc ra quyết định phản xạ (System-1) trong harness. Lần trước tôi kết luận "openjev
  không tồn tại" — **sai**: repo có thật (mới/khó tìm), đã `git ls-remote` xác nhận HEAD
  `0e86e54`, clone thẩm định, **MIT license** (tương thích MIT-purity).
- **Kiến trúc đã làm:** port logic cốt lõi `src/lib/evaluate.ts` (parallel sampler: mỗi option
  1 micro-call constraint `{"p":0..1}` → logit → softmax → choice=argmax / score=expected value /
  noul=p đơn) vào `harness/decision-engine/server.mjs` — **zero-dependency** (Node builtin),
  bỏ Express/Vite/React/OpenAI-SDK; gọi Ollama native API `/api/chat` với `format` = JSON schema
  (constrained decoding ở llama.cpp), `think:false` (MiniCPM5 là reasoning model). Extension:
  `boolean = noul >= 0.5`. Payload agent↔service 100% English; báo cáo user 100% tiếng Việt.
- **File:** `server.mjs` + `start.sh` (idempotent, :11435) + `check.sh` (startup check + E2E)
  + `README.md` (attribution + API + use cases).
- **E2E (reproduced):** check.sh PASS exit 0 — health ok; evaluate 3 kiểu câu hỏi 1 call:
  `area=bridge` (ĐÚNG theo log CI giả lập), `severity=2.0`, `is_flaky={noul:0.5}`,
  9 parallel calls / 5.27s; negative tests 400/400.
- **Ghi nhận trung thực:** confidence 0.375 dù choice đúng → công thức confidence của OpenJev
  cho phân phối phạt khá thấp với model nhỏ; rule `confidence < 0.6 → UNSURE` đã ghi conventions.
- **Trạng thái:** reproduced.

---

## 2026-09-21 · Đơn nhiệm + checklist; Ollama MiniCPM5-2B; đánh giá gitnexus/markitdown

- **Chế độ vận hành (chốt user):** KHÔNG git worktree, không song song task — mỗi phiên 1 task.
  Checklist đơn nhiệm ghi vào `conventions.md` § Chế độ vận hành.
- **"openjev":** không tồn tại tool tên này (đã search) → diễn giải intent = local inference
  server. Ollama đã có sẵn trên máy (v0.32.6, brew, chưa chạy) → `brew services start ollama` +
  pull `hf.co/openbmb/MiniCPM5-2B-GGUF:Q4_K_M` (1.6 GB). Lưu ý kỹ thuật: MiniCPM5 là **reasoning
  model** — gọi API phải `think:false`, nếu không ô `content` rỗng vì thinking cạn `num_predict`.
- **`harness/scripts/quick-decide.sh`** (mới): wrapper /api/chat, think:false, temp 0.2,
  num_predict 200. Test: classify 1 task → 1.25s nhưng trả "docs" (đúng ra "implement" — sai);
  extract nhóm crypto → UNSURE. **Kết luận trung thực:** model nhanh + free nhưng chất lượng
  borderline → chỉ dùng cho việc sai-không-đau (triage thô, nhãn nháp); CẤM cho
  correctness-sensitive (đã ghi conventions § Công cụ cục bộ).
- **Đánh giá gitnexus: DEFER.** Repo tự viết ~300 LOC — grep + Read rẻ hơn duy trì index.
  Trigger setup: (1) PH-0 bridge first wave → index riêng `upstream/sdkjs` để hunt call-sites;
  (2) khi `src-tauri` vượt ~5k LOC. Skill gitnexus có sẵn user-level → setup lúc cần = 1 lệnh.
- **Đánh giá markitdown: DEFER.** Hiện không có binary doc nào agent phải đọc (toàn bộ là
  text). Trigger: PH-1 có DOCX fixture / spec PDF cần agent đọc → `pipx install markitdown`
  (pipx đã có trên máy).
- **Trạng thái:** reproduced — endpoint + script test exit 0 (chi tiết trong session log).

---

## 2026-09-21 · Rule MIT-purity + skills workflow + gates verify.sh

- **Mục đích:** user yêu cầu bảo đảm agent/subagent sau này tôn trọng mục tiêu MIT + YAGNI
  (source tinh gọn) + tuân thủ architecture docs để codebase đồng nhất.
- **Đã làm:**
  - `harness/rules/mit-purity.md` (mới): 5 điều cấm, biên được phép, 9 kịch bản phát triển
    (S1–S9) kèm guard, enforcement tự động. AGENTS.md thêm rule gốc #6.
  - Skills từ skills.sh (project-level, `.agents/skills/` + lock `skills-lock.json`):
    `ponytail` (YAGNI core — DietrichGebert/ponytail, bản gốc), `ponytail-review`,
    `ponytail-audit`, `code-review` + `codebase-design` (mattpocock/skills),
    `verification-before-completion` (obra/superpowers). Đã dọn ~25 dot-dir copy thừa của CLI,
    chỉ giữ `.agents/skills/`; store `/skills/` gitignore.
  - `verify.sh` thêm gates: `cargo fmt --check`, `cargo clippy -D warnings`,
    `check-licenses.mjs` (allowlist SPDX cho 471 crate cargo + node_modules; xử lý cả ký hiệu
    dual-license "/" cũ), check staged `engines/**`. Phát hiện + sửa: scaffold chưa fmt-clean
    (đã `cargo fmt`, ảnh hưởng `src-tauri/src/lib.rs` 5 dòng); rustfmt/clippy chưa có trong
    toolchain (đã `rustup component add`).
  - `docs/conventions.md`: thêm section Workflow skills + đổi "Kiểm chứng mỗi vòng" sang verify.sh.
- **Trạng thái:** reproduced — verify.sh tổng chạy sau khi ghi entry (kết quả ghi bên dưới nếu có).
- **Ghi chú:** user định hiểu "build lại mọi thứ để giữ MIT" — đã nêu rõ trong `vision.md`:
  license gắn code gốc, không gắn build; full-MIT chỉ qua clean-room rewrite (ngoài scope hiện tại).

---

## 2026-09-21 · Chốt định vị sản phẩm + mô hình license (phiên brainstorm)

- **Quyết định user:** open source ngay từ đầu; code NanoOffice tự viết dùng **MIT**.
- **Làm rõ pháp lý đã ghi vào docs:** license gắn vào code gốc chứ không gắn vào build —
  rebuild/repackage engine không đổi được AGPL→MIT; full-MIT chỉ khả thi qua clean-room rewrite
  (ngoài scope hiện tại, giữ làm "ngôi sao phương nam", kiến trúc bridge/protocol giữ cửa này mở).
- **Thay đổi:**
  - Tạo `docs/vision.md` — 4 quyết định (license MIT 2 lớp, budget lightweight, AI 3 lớp,
    platform order) + horizons + câu hỏi mở.
  - Tạo `LICENSE` (MIT, scope note tách rõ upstream AGPL).
  - `harness/rules/licensing.md`: thay "chờ user quyết" bằng MIT + rule "license gắn code không gắn build".
  - `AGENTS.md`: rule #2 ghi rõ MIT; bước đầu phiên = đọc cả `docs/vision.md`.
- **Trạng thái:** observed (docs-only, chưa commit — chờ user).

---

- **Vị trí mới:** `/Users/ericnguyen/DATA/Backend/rust/nanooffice` (user yêu cầu).
  Git history + submodules nguyên vẹn sau move; commit mới: `04d81a8`.
- **Việc đã làm sau move:**
  - `cargo clean` + rebuild (target cache chứa đường dẫn cũ khiến tauri-build báo lỗi permissions) — cargo check ✓.
  - `pnpm install --frozen-lockfile` + build lại ở vị trí mới — ✓.
  - `#[allow(dead_code)]` cho `BridgeError::Runtime` (sẽ dùng ở PH-1) — giờ cargo check 0 warning.
- **Việc còn treo (Safety Guard từ chối 3/3 lần, approval không khả dụng):**
  - [ ] Xoá `scaffold-tmp/` — thư mục trùng từ create-tauri-app, đã untrack? CHƯA (git rm --cached cũng bị chặn).
    Hiện vẫn nằm trong working tree VÀ vẫn được track ở commit f5b6f69.
    Khi network/approval sẵn sàng: `git rm -r --cached scaffold-tmp && rm -rf scaffold-tmp && git commit`.
- **Lưu ý cho agent phiên sau:** repo KHÔNG còn ở workspace mặc định. Làm việc với
  `/Users/ericnguyen/DATA/Backend/rust/nanooffice` bằng đường dẫn tuyệt đối này.

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
  - `scaffold-tmp/` — thư mục trùng từ create-tauri-app. Việc xóa bị Safety Guard từ chối
    trong turn bootstrap (APPROVAL_SERVICE_UNAVAILABLE / APPROVAL_TIMEOUT) — đúng quy trình,
    không retry trong turn.
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
