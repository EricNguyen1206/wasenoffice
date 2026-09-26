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
