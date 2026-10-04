---
type: decision
---
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

### Con đường "full MIT" → đã được nâng lên lộ trình chính, xem Quyết định 9 (Track A)

Nguyên tắc giữ nguyên: **cấm port/translate code AGPL** (license gắn vào code gốc — dịch
ngôn ngữ vẫn là derivative). Thay thế engine chỉ theo clean-room: từ spec chuẩn mở + quan sát
black-box (oracle), theo đúng `harness/rules/clean-room.md`. Trật tự staged giữ nguyên
x2t trước → viewer → editor → UI (chi tiết rung A1–A6 ở QĐ9).

## Quyết định 2 — Ultra lightweight: budget đo được (định hướng — chốt số sau PH-0 baseline)

| Chỉ số | Target đề xuất | Ghi chú |
|---|---|---|
| Installer shell | < 30 MB | chưa tính engine payload |
| Tổng payload có engine | ≤ 100 MB (**soft target** — user 30.09: chấp nhận phình, ưu tiên tự chủ, tối ưu sau) | số liệu thực ghi thẳng diagnostics; tối ưu size = việc T-A12, không chặn rung Track A; chốt lại số khi PH-4 |
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

## Quyết định 8 — Chuẩn tài liệu: OKF v0.2 dạng thin (CHỐT)

Chốt 27.09.2026 (user, sau điều tra chuẩn OKF — Open Knowledge Format, Google Cloud công bố
12.06.2026, hiện v0.2; phân tích so sánh trong report tạm 27.09).

- Áp **dạng thin**: mỗi file `docs/**/*.md` mang 1 dòng frontmatter `type` từ vocabulary cố định
  (bảng: `conventions.md` § OKF-thin) + gate `harness/scripts/check-docs.mjs` trong `verify.sh`
  (frontmatter · link nội bộ · coverage index). Phần triết lý OKF còn lại repo đã có sẵn
  (index/log/path-là-danh-tính/cross-link/provenance) — compatible-by-construction.
- KHÔNG áp full: không đổi tên `index.md`/`log.md`, không metadata đầy đủ, không attestation/
  `sources`/`generated` — over-engineer ở quy mô solo (metadata đôi, vocabulary nghèo hơn, chuẩn
  v0.x còn rung — 2 breaking changes trong 3 tháng).
- **Trigger nâng cấp full OKF:** repo public + có external agent đọc docs theo OKF, hoặc chuẩn
  đạt v1.0 ổn định. Lúc đó chỉ bọc thêm YAML — cây/luật giữ nguyên.

## Quyết định 9 — Tự chủ mã nguồn 100%: Track A thay engine theo thang clean-room (CHỐT)

Chốt 29.09.2026 (user phê D1–D3). Mục tiêu sản phẩm nâng cấp: **cạnh tranh được với
ONLYOFFICE / MS Office** — tự chủ hoàn toàn mã nguồn qua thay thế từng module phía sau
các contract đã reverse (bridge ~130 method, params-XML x2t, Editor.bin, font contract),
pattern "strangler fig". Sản phẩm xuất xưởng được ở **mọi rung**.

**Thang tự chủ (đơn nhiệm vẫn ≤1 🟨 toàn file TODO):**

| Rung | Nội dung | XONG khi |
|---|---|---|
| A0 ✅ | shell + bridge + save chain của mình | gates 9+10 GREEN (28.09) |
| A1 ✅ | Tự chủ phân phối: bundle x2t + tự sinh AllFonts.js + font subset | ✅ **02.10.2026** — gate 11 "sạch ONLYOFFICE" thường trụ trong verify.sh (strict clean-machine: round-trip GREEN + 0 path app trong log; negative proven) + test tích thể mv `/Applications/ONLYOFFICE.app`: verify.sh FULL exit 0 khi app ẨN (`harness/scripts/clean-machine-proof.sh`, diagnostics 02.10). Payload: soft target (113 MB x2t — user 30.09 chấp nhận phình, tối ưu sau) |
| A2 ✅ | Converter Rust thay x2t (DOCX → XLSX → PPTX) | ✅ **03.10.2026** — adapter crate trong production path: open-side pre-normalize qua seam `sidecar::convert` (docx → canonical.docx bằng crate → x2t chuyển ra Editor.bin; kill switch `NANO_OFFICE_NO_RUST_CONVERTER`), parity 15/15 construct (T-A23), gates 9/10/11 GREEN cả ON lẫn OFF (counterfactual); Editor.bin giữ x2t đến A4 (DD K01 §2) · diagnostics 03.10 T-A24 |
| A3 ✅ | Viewer Rust đọc-only ("quick view" không boot sdkjs) | ✅ **03.10.2026** — viewer canvas 2D webview riêng không shim/engine (DD V01), parse+resolve về Rust qua crate A2: parity render vs oracle CDP **6/6 corpus ×2 deterministic + negative cắn, ε=2.0px** (T-V4, OQ-V01-1); mở **<500 ms** GREEN (gate 12, đo 18–89 ms debug); UX dùng được thật: nút Quick view trên start page + scroll (T-V5, OQ-V01-3 toggle) + gate 13 E2E từ UI; **RAM <150 MB = criterion RELEASE** — T-057 (PH-4) chặn đo release, debug tripwire 300 MB (đo 265 MB tree) — ceiling ghi DD V01 §10 · diagnostics 03.10 T-V5 |
| A4 ✅ | Editor core riêng cho DOCX subset hằng ngày ("Nano Light", engine ONLYOFFICE = "chế độ nâng cao"; **default-switch DỜI A5** — OQ-L01-1, user chốt 04.10) | ✅ **04.10.2026** — parity matrix DD L01 §5.1: **9 lớp GIỮ** (text/para/run · bảng cấu trúc · list · ảnh byte-exact · hdr/ftr · sectPr + pgMar · hyperlink · settings/theme/docProps passthrough) qua gate 15 + parity-a2 11/11 invariant; **dòng mất còn lại TOÀN BỘ detect+warn** (tracked/footnotes/fields · tblPr/tcPr T-A32 · con khác sectPr) — hết mất im lặng; 4 slice + closure: selection/arrows T-A28 · IME/clipboard T-A29 (OQ-L01-3) · dblclick/blink/bảng single-cell T-A30 · save-fidelity T-A31 (OQ-L01-2) · tblPr/tcPr warn T-A32; verify FULL exit 0 19 ▸ steps · diagnostics 04.10 T-A32 |
| A5 | UI editor riêng thay web-apps | web-apps rút khỏi phân phối mặc định |
| A6 | sdkjs rút khỏi installer — repo thuần MIT | `upstream/` chỉ còn tham khảo |

**Ba quyết định con (user phê 29.09):**
- **D1 — không hard-cut:** engine AGPL giữ vai trò reference + chế độ nâng cao tới tận A6;
  phân phối trong giai đoạn này vẫn chịu nghĩa vụ AGPL component (QĐ1 + `licensing.md`).
- **D2 — TODO hai track song song** (P = sản phẩm trên engine AGPL, A = tự chủ) — user theo
  dõi tiến trình qua dashboard; track P không bị giết: nó là sản phẩm thật + nuôi kiến thức seam.
- **D3 — clean-room rule ràng buộc** mọi agent: `harness/rules/clean-room.md` (cấm ĐỌC code
  module đang thay — chỉ spec `spec-ref:` + oracle `oracle-obs:`; siết kịch bản S9 mit-purity).

**Định nghĩa "cạnh tranh được" (thẳng thắn):** không đấu feature-parity trực diện với MS
(sdkjs 2.1M dòng / ~15 năm; MS ~35 năm). Đấu ở 3 mũi vision sẵn có: **nhẹ** (≤100 MB vs
1.1–3 GB), **MIT cho phép nhúng** (đối thủ không cho — moat cộng đồng từ A2), **AI-native
local-first** trên bridge-as-agent-API (QĐ3, chạy trên seam — không phụ thuộc rung nào).
Sản phẩm có chỗ đứng từ A2–A3 (~tháng 3–6); A4+ là cuộc chiến dài, cần repo public kéo
cộng đồng — đúng lý do chốt open-source ngày đầu.

## Horizons

| Horizon | Track P — sản phẩm (engine AGPL) | Track A — tự chủ mã nguồn (QĐ9) |
|---|---|---|
| H1 | PH-1 phần còn lại (T-048 multi-format, T-047) + PH-2 trải nghiệm desktop | **A1** (nuốt T-040/T-041) |
| H2 | PH-4 packaging + updater + CI + size/RAM/startup gates | **A2** converter Rust |
| H3 | AI lớp 1 → 2 (QĐ3, trên bridge — không đổi) | **A3** viewer + khởi động A4 |
| H4 | Mobile + ecosystem (MCP, plugins) | **A4** mở rộng + **A5** |
| ★ | — | **A6** thoát sdkjs — giữ điều kiện nguồn lực/cộng đồng |

## Câu hỏi mở

- [ ] **PH-0:** đo baseline size/RAM/startup: app mình vs ONLYOFFICE cài sẵn → ghi `docs/diagnostics.md`, từ đó chốt số Quyết định 2.
- [ ] **PH-4:** bundle engine AGPL trong installer (phải đáp ứng đủ nghĩa vụ AGPL: NOTICE,
  source offer, logo §7) **hay** tải engine lần đầu chạy (repo + installer thuần MIT, update
  engine tách nhịp app release, installer nhẹ hơn). Xu hướng hiện: download-first. Quyết khi làm packaging.
