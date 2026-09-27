# docs/vision.md — tầm nhìn & quyết định sản phẩm

Chốt trong phiên brainstorm 21.09.2026. Mọi quyết định scope/kiến trúc về sau đối chiếu về đây.
Sửa quyết định = sửa file này + ghi entry mới vào `docs/diagnostics.md`.

## Định vị

- **Open source ngay từ đầu** (quyết định của user, 21.09.2026).
- **Stack shell: GIỮ Rust + Tauri** (chốt 26.09.2026) — re-validate Bun/Go/Rust theo 6 tiêu chí,
  Rust thắng áp đảo (binary 5.0 MB vs 7.9/61 MB; mobile stable duy nhất; MIT không phải yếu tố
  ngôn ngữ). Chi tiết: `docs/techstack-analysis.md` (research, không commit). Câu hỏi ĐÓNG —
  không mở lại nếu không có bằng chứng mới.
- Sản phẩm: office suite desktop **"nhẹ hơn và có AI"** — 3 tiêu chí: AI Native · ultra lightweight · multi platform.
- Mũi nhọn khác biệt: nhẹ + AI local-first + dữ liệu không rời máy — điểm MS 365 / Google Docs không đánh được.

## Quyết định 1 — License: mô hình 2 lớp, MIT cho code tự viết (CHỐT)

| Lớp | License | Ghi chú |
|---|---|---|
| Code tự viết (`src-tauri/`, `src/`, `harness/`, `docs/`) | **MIT** — `LICENSE` ở root | áp dụng từ commit public đầu tiên |
| Engine (`upstream/sdkjs`, `upstream/web-apps`) | AGPL-3.0 (+ điều khoản §7: giữ logo) | submodule chỉ đọc, giữ nguyên provenance |
| Converter (`core`/x2t C++) | AGPL-3.0 | sidecar binary riêng, KHÔNG link tĩnh vào shell |

**Sự thật pháp lý neo quyết định này:** license gắn vào **code gốc**, không gắn vào build.
Rebuild/repackage engine không biến AGPL thành MIT. Chỉ **clean-room rewrite** (viết lại từ
spec chuẩn mở, không nhìn code AGPL) mới tạo ra code engine MIT được — đó là dự án multi
người-năm, tách riêng khỏi roadmap hiện tại.

### Con đường "full MIT" (ngôi sao phương nam — KHÔNG phải scope hiện tại)

Vẫn giữ nguyên cấm port/rewrite engine sang Rust trong scope hiện tại (AGENTS.md § identity,
`docs/conventions.md`). Nhưng kiến trúc phải giữ cửa này mở mãi:

1. Engine chỉ chạm shell qua bridge + `nanooffice-file://` (đã là rule gốc #4) — shell không
   import/link gì từ `upstream/`.
2. Không copy-paste AGPL source vào file MIT (rule `licensing.md`) — cây code sạch, rewrite
   sau này không "nhiễm DNA".
3. Nếu một ngày làm thật, theo thứ tự staged:
   - Bước 1 — thay x2t bằng Rust (parse/serialize DOCX/XLSX/PPTX từ spec ECMA-376/ISO 29500):
     nhỏ nhất, tách biên sạch nhất (đã là process riêng).
   - Bước 2 — render/edit engine (sdkjs ~2,1M dòng): chỉ làm khi có team + nguồn lực riêng.

## Quyết định 2 — Ultra lightweight: budget đo được (định hướng — chốt số sau PH-0 baseline)

| Chỉ số | Target đề xuất | Ghi chú |
|---|---|---|
| Installer shell | < 30 MB | chưa tính engine payload |
| Tổng payload có engine | ≤ 100 MB | hoặc engine tải riêng lần đầu (xem Câu hỏi mở) |
| RAM idle + 1 doc mở | < 250 MB | ONLYOFFICE gốc ~1,07 GB / 7 processes (đo 21.09.2026, `diagnostics.md`) |
| Cold start → gõ được chữ | < 2 s | đo bằng script, đưa vào CI gate từ PH-4 |

Đòn bẩy cắt nhẹ: per-app-type engine build (documenteditor/cell/slide load on demand),
strip loginpage/cloud (PH-3), fonts subset theo locale, WASM engine load khi cần,
x2t spawn on-demand (đã thiết kế trong `sidecar/`).

## Quyết định 3 — AI Native: 3 lớp, bridge chính là agent API (định hướng)

Insight trung tâm: ~130 method `window.AscDesktopEditor` (`reference.map.md`) đã là tool
surface hoàn chỉnh — **mỗi method STUB→DONE là một tool cho AI**. Không bao giờ tạo kênh AI
riêng song song bridge.

1. **Copilot** — command palette + inline generate/edit; drive editor qua config API sdkjs
   (cùng đường với thao tác người dùng).
2. **Document intelligence local-first** — index + RAG + summarize trên máy user; dữ liệu
   đi qua `nanooffice-file://`, index nằm ở Rust.
3. **Agent mode + MCP** — agent multi-step thao tác tài liệu qua bridge; export MCP server
   để mọi AI client ngoài dùng được document ops của NanoOffice.

Model strategy: BYO-key cloud trước (0 byte tăng installer) → local model sau qua sidecar
(llama.cpp/ONNX runtime, tái dùng pattern spawn/timeout/queue của `sidecar/`).

## Quyết định 4 — Multi platform: macOS → Windows → Linux, mobile ở H4 (định hướng)

- Desktop theo thứ tự trên (macOS = máy dev hiện tại; Windows = thị trường; Linux = cộng đồng AGPL).
- Mobile (Tauri v2): engine JS chạy mọi webview (đã verify 0 CEF-dependency — `reference.map.md`),
  bottleneck duy nhất là x2t binary per-platform → mobile convert ở desktop/cloud thay vì build x2t mobile.

## Quyết định 5 — Hai ngôi nhà agent: `harness/` và `.agents/` (CHỐT)

Chốt 26.09.2026 (architecture review định kỳ, report HTML tạm). Hai folder phục vụ hai seam khác nhau,
zero trùng nội dung (đã grep hai chiều) — **không gộp**:

| Ngôi nhà | Bản chất | Ai consumes |
|---|---|---|
| `harness/` | contract first-party tự viết (rules · skills project · verify.sh · system1) | `verify.sh:20` exec `check-licenses.mjs`; AGENTS.md; comment path trong `src-tauri/src/{bridge,sidecar,protocol}/mod.rs`; `licensing.md`, `.gitignore` |
| `.agents/skills/` | skill generic import từ marketplace skills.sh, hash-lock qua `skills-lock.json` | ZCode Skill discovery; `npx skills update -p` (CLI sở hữu dir) |

- Lịch sử: `harness/` từ bootstrap `f5b6f69` (21.09); `.agents/` từ `1830d50` (22.09) — path `.agents/skills/`
  do CLI skills.sh dictate, không phải lựa chọn thiết kế cạnh tranh với `harness/`.
- **Cấm merge `harness/` → `.agents/`** khi không có bằng chứng mới: vỡ verify gate (path chết trong verify.sh),
  ~10 tham chiếu path thành stale, file tay thành foreign entry trong dir tool bên ngoài sở hữu.
- Muốn skill harness tự-trigger được: chỉ bằng pointer-skill mỏng trong `.agents/` (không copy nội dung),
  phải probe CLI trước + qua acceptance YAGNI. Chưa có consumer thực → để backlog.

## Quyết định 6 — Tài liệu public: DD + TEST theo màn hình (CHỐT)

Chốt 26.09.2026 (user). `docs/design/` là source of truth cho thiết kế chi tiết (DD) + kế hoạch
kiểm thử (TEST) của từng màn hình — public official ngay từ đầu, phục vụ contributor ngoài.

- Cấu trúc: `docs/design/[Mã]-[tên]/` gồm `DD.md` + `TEST.md`; registry (danh sách màn + cấp mã)
  ở `docs/design/README.md`; template chuẩn `_TEMPLATE-*.md` (cảm hứng Google Design Doc + RFC
  status lifecycle + test-plan tách riêng).
- **Bổ sung 27.09 — mã hóa màn hình:** mỗi màn có mã ổn định `[Chữ][2 số]` (màn con `-n`), chữ =
  nhóm chức năng: **A** trang chủ + thông tin chung (A01 startpage, A02 about) · **B** tài liệu về
  phần mềm (hướng dẫn) · **C** màn dùng chung nhiều màn (C01 print-preview) · **D/E/P** màn
  DOCX/XLSX/PPTX (D01 editor) · **M** markdown editor · **T** txt editor · **X** cài đặt (X01).
  M/T ngoài roadmap hiện tại — chỉ giữ namespace. Chi tiết + quy tắc cấp mã: `docs/design/README.md`.
- **Ngôn ngữ: tiếng Việt** (chốt user 26.09 — nhất quán toàn bộ docs dự án).
- Luật: đổi hành vi màn hình → cập nhật DD + TEST **cùng commit**; màn mới → đăng ký screenlist
  trước; TEST là điều kiện sign-off (P0 phải GREEN hoặc RED có lý do).
- Khởi tạo 26.09: editor + startpage + 3 stub planned (print-preview, settings, about).
  `CONTRIBUTING.md` ở root từ 26.09. (27.09 mã hóa: D01-editor, A01-startpage, A02-about,
  C01-print-preview, X01-settings.)
- Lý do: "open source ngay từ đầu" (§ Định vị) — contributor cần entry point + source of truth,
  không phải đọc ngược chat/diagnostics để hiểu hệ thống.

## Quyết định 7 — Quản lý công việc: TODO queue theo phase (CHỐT)

Chốt 27.09.2026 (user duyệt format chia theo phase + placement `docs/`).

- `docs/TODO.md` = nhà **duy nhất** của task vận hành: chia section theo phase (+ section
  **META** cho việc harness/docs/skills), thứ tự trong section = priority, dashboard đếm +
  dòng `▶ Đang làm` ở đầu file.
- 1 task = 1 dòng **pointer** (chi tiết ở handoff/DD-TEST/diagnostics); ID `T-n`; trạng thái
  ⬜/🟨/⏸/✅/🚫; toàn file ≤ 1 🟨 (đơn nhiệm); ✅ sink đáy section + archive theo phase
  (`docs/TODO-archive.md` khi đóng phase).
- Agent: claim đầu phiên (🟨 + ngày, không đụng 🟨 của session khác), release + cập nhật
  dashboard cuối phiên — ghi vào AGENTS.md checklist.
- AGENTS.md "Việc còn treo" chỉ còn pointer + điểm nổi bật. Khi public repo: migrate queue
  sang GitHub Issues (nhà của spec doc không đổi).

## Horizons

| Horizon | Phạm vi |
|---|---|
| H1 | PH-0 go/no-go → PH-1 vòng đời file → PH-2 trải nghiệm desktop |
| H2 | PH-4 packaging + updater + CI + size/RAM/startup gates; tự build engine (clone `build_tools`); PH-3 mặc định cắt khỏi roadmap |
| H3 | AI lớp 1 → 2 → 3 |
| H4 | Mobile + ecosystem (MCP server, plugins) |
| ★ | Full-MIT engine rewrite — chỉ khi nguồn lực cho phép, theo staged order ở trên |

## Câu hỏi mở

- [ ] **PH-0:** đo baseline size/RAM/startup: app mình vs ONLYOFFICE cài sẵn → ghi `docs/diagnostics.md`, từ đó chốt số Quyết định 2.
- [ ] **PH-4:** bundle engine AGPL trong installer (phải đáp ứng đủ nghĩa vụ AGPL: NOTICE,
  source offer, logo §7) **hay** tải engine lần đầu chạy (repo + installer thuần MIT, update
  engine tách nhịp app release, installer nhẹ hơn). Xu hướng hiện: download-first. Quyết khi làm packaging.
