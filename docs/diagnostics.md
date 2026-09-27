---
type: log
---
# docs/diagnostics.md — nhật ký chẩn đoán

Quy tắc: mỗi entry ghi thời điểm, triệu chứng, bằng chứng, trạng thái
(observed / candidate / reproduced / confirmed). Entry mới thêm TRÊN CÙNG.

---

## 2026-09-27 · T-003: PH-0 verdict — GO WITH CAVEAT + baseline

- **Verdict: GO WITH CAVEAT** — render + open pipeline GREEN; scroll/edit qua bàn phím tự động
  (criteria b) KHÔNG test được một cách an toàn: GUI đang bị user chiếm dùng live (foreground
  liên tục đổi app giữa các lần chụp) và `osascript click at` bị macOS từ
  (`-25211 … not allowed assistive access`). Gửi tiếp phím tự động = nguy cơ inject text vào app
  đang mở của user → DỪNG, fallback theo pre-decided rule của T-003. Caveat: user cần ~30 giây
  xác nhận thủ công (mở app, PageDown/PageUp, gõ chữ, Cmd+Z — chữ hiện, không crash) để chốt (b).
- **Criteria walk:**
  - **(a) render GREEN — đạt:** verify.sh gate 9 `smoke-render.sh` PASSED (đủ 3 tín hiệu:
    `asc_onDocumentContentReady` · `id_viewer 2190x958` height>0 · `GET /fonts/sys/Symbol.ttf`);
    full `verify.sh` **ALL CHECKS PASSED, exit 0** (27.09).
  - **(b) scroll/edit — UNTESTABLE-SAFELY:** boot 2/2 lần 27.09 đều hiện dialog lỗi T-047
    (không tạo dòng `asc_onError` trong log — đúng như T-047 đã ghi). Baseline tương tác =
    dòng 191 của `/tmp/t003-edit.log`; sau marker chỉ có probe định kỳ của shim
    (`canvas probe-2` 08:35:53, `viewer=1151x479`) — **0 dòng bridge mới**, tức phím probe
    (PageDown+T) đã rơi vào foreground app của user, không phải NanoOffice. Log có
    `onDocumentModifiedChanged([false])` nhưng chỉ boot-time (không phải bằng chứng edit).
    Screenshot trước tương tác: app frontmost sau AXRaise, render đủ 5 đoạn fixture + dialog
    T-047 (`/tmp/t003-before3.png`, file /tmp không commit). Không crash, không error storm.
  - **(c) baseline đã đo — bảng dưới** (số tươi 27.09, không copy số cũ).
  - **(d) không phát hiện gì phá giả định WKWebView no-CEF** — T-047 là race đã biết
    (backlog PH-1), canvas sống, không CEF.
- **Baseline 27.09 (debug build, macOS arm64; RSS = `ps -o rss=` KB→MB/1024; "helper" = 3 WebKit
  XPC (WebContent/Networking/GPU) nhận diện theo PID kề app; node vite ~160 MB là dev-only
  không ship; đo khi `pnpm dev` :1420 + `NANO_OFFICE_DEVTOOLS=0 RUST_LOG=info
  ./src-tauri/target/debug/nanooffice` — idle: không `NANO_OFFICE_OPEN`; 1-doc: có):**

| Chỉ số | NanoOffice (debug) | ONLYOFFICE.app | Ghi chú |
|---|---|---|---|
| Shell binary | **29 MB** (`ls -lh src-tauri/target/debug/nanooffice`) | — | 22.09: 30 MB (khớp) |
| engines/ payload | **313 MB** (`du -sh engines/`) | **1.1 GB** (`du -sh /Applications/ONLYOFFICE.app`) | budget ≤100 MB → T-040 (PH-4) |
| RAM idle (start page) | app 114.4 + helper 88.4 (WC 35.8/Net 23.0/GPU 29.6) ≈ **203 MB** · 4 processes | **570 MB** · 5 processes (đo 27.09: `open -a ONLYOFFICE`, sleep 15s, chưa mở document) | 22.09: 133 MB — số cũ ĐO KHI DEVTOOLS MỞ, không so trực tiếp được |
| RAM 1 doc (fixture 1.9 KB) | run1 ≈ **561 MB** (app 112.1 + WC 369.3/GPU 52.3/Net 26.9); run2 ≈ **579 MB** (app 114.8 + WC 375.1/GPU 53.3/Net 36.0) | 21.09: ~1.07 GB / 7 processes (số cũ, chỉ đối chiếu) | engine (WebContent) chiếm ~370 MB — mục trim chính cho T-040 |
| Cold start → contentReady | **3.62 s** (run1) · **3.37 s** (run2) — wall time, poll 100ms | — | marker `asc_onDocumentContentReady` (nghiêm hơn marker `frame ready:` của shim `desktopEditor.js:574`); dev server warm, chưa phải số release |
| Processes khi chạy | shell + 3 WebKit XPC = 4 | 5 (start page) · 7 (21.09, có document) | |

- Lệnh đo ONLYOFFICE: `ps aux | grep -i onlyoffice | grep -v grep | awk '{sum+=$6; n++} END
  {print n, sum/1024 " MB"}'` → `5 569.688 MB`; quit bằng `osascript 'quit app "ONLYOFFICE"'
  (exit 0, không orphan). App NanoOffice sau mỗi run: kill trực tiếp + kiểm tra `ps`.
- **Trạng thái:** (a)(c)(d) reproduced; (b) candidate — chờ xác nhận thủ công 30s của user.
  PH-0 đóng với caveat này; baseline release đo ở PH-4 (T-021/T-040).

---

## 2026-09-27 · Review kiến trúc định kỳ (improve-codebase-architecture) — 5 candidates, landing

- **Phạm vi quét:** hot-spot từ git history (shim, bridge, filesrv, lib — 5 commit code gần
  nhất) + agent Explore sweep phần còn lại. Báo cáo đầy đủ là file HTML tạm trong $TMPDIR
  (thiết kế skill: không vào repo); quyết định + bằng chứng chính được lưu ở đây.
- **C1 ✅ smoke gate behavioral (Strong)** → T-042 (82502c1) — làm ngay trong phiên.
- **C2 Strong — bridge method surface 3 nơi/2 ngôn ngữ:** mỗi method sửa tay ở shim
  (`desktopEditor.js` KNOWN_METHODS + LOCAL_TRUTHS), Rust (`bridge/mod.rs` match), docs
  (`reference.map.md`); `globals.d.ts` khai báo mọi method `Promise<unknown>` trong khi 11+
  method sync-truth trả đồng bộ — interface typed duy nhất của dự án đang sai contract. Fix
  dạng ponytail: 1 registry + check script đối chiếu 3 bảng + sinh type sync/async. → **T-044**.
- **C3 Worth exploring — open-delivery chain:** ordering sống trong prose/magic number
  (delay 1500ms `deliverDocument`, retry 120×500ms `consumeDeliveredDocument`, chữ ký
  `EndLoad` chỉ trong comment, `_offline_` magic, adapter font hai đầu Rust↔JS không chung
  constant); ~120 dòng probe diagnostics lẫn đường production sẽ mục ở PH-1. Fix: delivery
  module có tên + probe sau `NANO_OFFICE_PROBE`. → **T-045**.
- **C4 Worth exploring — stub `sidecar/` + `window/` fail deletion test:** 0 dòng code trong
  khi conventions.md tuyên bố x2t thuộc sidecar (thực tế inline ở `lib.rs`
  `get_document_for_delivery`). Fix: xoá stub + sửa docs. → **T-043**.
- **C5 Speculative — config env sprawl:** 4 biến `NANO_OFFICE_*` đọc tại điểm dùng, 2 quy tắc
  khám phá artifact (engines/ vs x2t), `:14321` hardcode trong docs trong khi filesrv có thể
  rơi về `:0`. Fix: `env.rs` deep module — **chưa làm**, trigger PH-1. → **T-046**.
- **Đang deep tốt (giữ nguyên):** `protocol/mod.rs`, `filesrv.rs`, `bridge/mod.rs` — interface
  nhỏ, unit test có thật, Range/MIME/path-traversal nằm sau handler mỏng. Minor ghi nhận:
  logging adapter lặp 2 nơi (3 dòng/nơi), port 1420 duplicate (convention Tauri, benign).
- **Trạng thái:** observed (đọc code firsthand) + C1 reproduced; còn lại chờ trigger/claim.

---

## 2026-09-27 · T-042: verify.sh thêm gate 9 — smoke render behavioral (chốt Candidate 1, architecture review)

- **Bối cảnh:** review kiến trúc 27.09 (improve-codebase-architecture + agent sweep) chỉ ra
  verify.sh 8 gate toàn compile/lint — chuỗi render đắt nhất dự án (5 ngày debug) không có
  kiểm tra tự động; GREEN của TC-D01-06 chỉ tồn tại dạng khai báo tay.
- **Gate mới (reproduced):** `harness/scripts/smoke-render.sh` (~60 dòng bash, 0 dependency)
  boot app thật (`NANO_OFFICE_OPEN=<fixture> NANO_OFFICE_DEVTOOLS=0 pnpm tauri dev`; dọn dẹp
  bằng process-group kill qua `set -m` + disown, đã kiểm tra không sót orphan) rồi poll log
  stdout chờ đúng **3 tín hiệu TC-D01-06**: ① `asc_onDocumentContentReady` ② `viewer=WxH` /
  canvas probe với height > 0 ③ `GET /fonts/` (filesrv log mức info). Đủ 3 trong timeout
  (default 240s, `SMOKE_TIMEOUT`) → exit 0; hết giờ/app chết sớm → exit 1 kèm log tail.
  Fixture tự sinh qua `harness/fixtures/make_docx.py` nếu thiếu; chặn sớm nếu port 1420 bận.
- **Verify (đều chạy thật, 27.09):** positive **exit 0** (contentReady + canvas 2190×958 +
  `GET /fonts/sys/Symbol.ttf`); **negative exit 1** (`NANO_OFFICE_X2T=/nonexistent` → delivery
  fail → timeout → FAILED) — gate biết cách fail; full `verify.sh` 9 gate **exit 0**.
- **Giới hạn (ghi rõ):** gate phụ thuộc log contract của shim — đổi text các dòng evidence
  phải đổi gate cùng lúc (hai đầu đều có comment trỏ nhau). Window app bật/tắt ngắn khi chạy
  verify.sh là hành vi designed của gate local. CI headless (T-021) sẽ cần revisit cách boot.

---

## 2026-09-27 · T-002: PHẢN RENDER PH-0 ĐÃ QUA — TC-D01-06 GREEN (oracle-diff CDP, phương án a)

- **Kết quả (reproduced, WKWebView thật — không CEF):** mở fixture DOCX 1.9 KB →
  `asc_onDocumentContentReady` bắn · `id_viewer` canvas **2302×958** (clientHeight 479 CSS) ·
  **14 request `/fonts/…`** chạm filesrv · font-oracle `Hcc=15 loadedFonts=15` (Arial/Courier
  New/Times New Roman — **đúng bằng bộ 15 stream của app thật**, oracle TC-D01-09) · text render
  đủ 5 đoạn fixture: heading serif, bold run, tiếng Việt có dấu, tab — screenshot:
  `docs/media/2026-09-27-tc-d01-06-render.png` (git-ignored) · log: contentReady + oracle trong
  stdout của `pnpm tauri dev`.
- **Root cause 1 — shim KHÔNG BAO GIỜ gọi open-call (confirmed, counterfactual = fix):**
  `consumeDeliveredDocument` chỉ decode-probe + gắn hooks rồi… dừng, thiếu chính lời gọi
  `window.DesktopOfflineAppDocumentEndLoad(folder, b64, decodedLen)` — engine không bao giờ nhận
  document (mọi quan sát "parse chạy sâu" trước 27.09 là của engine TRỐNG document). Chuẩn shell
  thật: desktop-sdk `client_renderer_wrapper.cpp:6480` — signature `(folder, base64, decodedLen)`
  (cpp:6440–6485: RecoverFolder → SetSourcePath → [ApplyChanges nếu có changes] → EndLoad →
  Signatures). Fix: gọi EndLoad đúng chỗ trong `src/shim/desktopEditor.js`.
- **Root cause 2 — font pipeline FULL build đi qua scheme CEF-only (confirmed):** trong pinned
  word build, CUỐI sdk-all.js định nghĩa LẠI `AscFonts.sje.prototype.BSa` (LoadFontAsync) thành
  XHR **`ascdesktop://fonts/<Va>`** — scheme mà CEF shell chặn native trả bytes (không phải
  `LoadFontBase64` như nghi vấn trước — contract đó có thật ở wrapper cpp:1475
  `window["<path>"]="<size>;<base64>"` sync nhưng build word 9.4 không dùng). Không có scheme
  handler trong Tauri → mọi font XHR chết im lặng → `jM` pump treo vĩnh viễn. Fix: shim patch
  entry `AscFonts.sje.prototype.BSa` (sau khi eval sdk-all.js) — same-origin XHR tới URL font
  loopback mà filesrv đã serve, nạp cùng registry/status/callback (`AscFonts.Hcc` + `AscFonts.G2b`,
  UB 2→0/1, `wOb`, `sV`) — re-implement từ hành vi quan sát được, không copy code (MIT purity).
  Chứng minh đối chiếu: app thật 15 streams UB=0 · của ta sau fix 15 streams UB=0.
- **Root cause 3 — 4 bridge method thiếu gây cascade hậu open (confirmed):** engine gọi sync ngay
  sau open: `LocalFileGetSaved` (desktop-sdk cpp:2221 trả `m_bLocalIsSaved` — thiếu → TypeError
  trong font pump → **EditingError −25** → dialog "Save as backup"), `LocalFileGetOpenChangesCount`,
  `onDocumentModifiedChanged`, `LocalFileRecents/Recovers` (web-apps documenteditor app.js gọi
  không điều kiện sau open; fire-and-forget). Fix: sync-truth/ack trong shim + mirror Rust.
- **Candidate còn mở (1):** dialog "An error has occurred while opening the file" (
  `openErrorText` ← `ConvertationOpenError` `iLa`) vẫn hiện **một số lần** dù contentReady đã
  bắn và render hoàn chỉnh; emitter `iLa` nằm ở viewer api riêng (`AscViewer.open`,
  sdk-all-min `g$g`) KHÔNG đi qua `editor.cc` của api chính (đã wrap, không thấy event) —
  nghi delivery-vs-boot race hoặc viewer ẩn instance. Không chặn PH-0 (render + open OK);
  để backlog PH-1. Ảnh có dialog: `docs/media/2026-09-27-tc-d01-06-render-with-dialog.png`.
- **Phương pháp oracle (T-001 = (a), user ủy quyền 27.09):** CDP `Runtime.evaluate` trong editor
  iframe của app thật (`--remote-debugging-port=9222`, plain Node ≥22, `/tmp` script không vào
  repo) — đối chiếu `Wg.Of/aVc`, `jM.SIa/PVd`, `Hcc`, loader `Va`/`UB` + Chrome replica của
  harness (polyfill `__TAURI__` + chính shim của ta, inject qua
  `Page.addScriptToEvaluateOnNewDocument`) để có frame-level observability. Ghi chú bẫy:
  init-script CDP KHÔNG lan qua OOPIF iframe — phải attach riêng target iframe.
- **Devtools docked làm mép layout:** debug build tự mở devtools → viewport webview còn 185px
  → `id_viewer` 0/10px gây误导. `lib.rs` thêm guard env `NANO_OFFICE_DEVTOOLS=0` để tắt khi
  verify layout (mặc định vẫn mở).
- **Trạng thái:** confirmed cho 3 root cause (fix → hết triệu chứng, lặp lại 3+ lần);
  candidate cho dialog còn lại. TC-D01-06: **GREEN** · TC-D01-09: GREEN (oracle khớp 15 fonts).

---

## 2026-09-27 · QĐ8: migration docs sang chuẩn OKF v0.2 thin + gate check-docs.mjs

- **Nhiệm vụ (T-022):** sau điều tra OKF (report tạm 27.09 — verdict "thin": triết lý repo đã
  hội tụ ~70–80%, chỉ đáng lấy frontmatter type + doc gate), user chốt migrate.
- **Đã làm:** (1) 20/20 file `docs/**/*.md` thêm frontmatter `type` theo vocabulary 11 giá trị
  (decision/convention/evidence/log/design/test/template/queue/handoff/registry/research);
  (2) `harness/scripts/check-docs.mjs` (zero-dep) — check frontmatter type, markdown link nội
  bộ resolve, coverage index trong docs/README.md; (3) gate thêm vào `verify.sh`; (4) ràng buộc
  agent-sinh-doc: conventions § OKF-thin (vocabulary + quy tắc), docs/README quy tắc, 3 template
  ghi chú "đổi type khi copy"; (5) vision QĐ8 (trigger nâng cấp full: public + external agent
  hoặc v1.0).
- **Gate bắt vi phạm thật ngay lần chạy đầu (reproduced):** 5 lỗi — 2 link placeholder trong
  template (TEST.md/DD.md chưa tồn tại cạnh template — đổi sang inline code) + 3 thư mục stub
  dùng brace `{A02,C01,X01}` trong index không máy-đọc được (tách 3 path đầy đủ). Sau sửa:
  `20 files · 0 link gãy · coverage đủ`, exit 0.
- **Ghi chú:** `docs/techstack-analysis.md` (untracked) cũng được gán `type: research` — file
  nghiên cứu không commit nhưng nằm trong cây docs thì phải qua gate khi có mặt.
- **Trạng thái:** confirmed (quyết định user; verify.sh full exit 0 — kết quả trong báo cáo).

---

## 2026-09-27 · Gộp handoff vào docs/handoff/ + vòng đời viết/ghi tiếp/xoá

- **Nhiệm vụ:** user muốn folder tập trung handoff để quản lý context agent dài hạn — chọn
  `docs/` (không phải `harness/`: handoff là artifact theo phiên, không phải máy móc/quy trình)
  + README hướng dẫn agent khi nào viết, khi nào xoá.
- **Migration:** `docs/_TEMPLATE-handoff.md` → `docs/handoff/_TEMPLATE.md` ·
  `docs/handoff-ph0.md` → `docs/handoff/ph0.md` · `docs/handoff-techstack-analysis.md` →
  **đã dọn (xoá)** theo đúng quy tắc mới: task research stack đóng từ 26.09 (QĐ ghi vision,
  commit ea24231), tri thức đã về nhà — git history giữ bản gốc. 7 điểm tham chiếu sửa:
  CONTRIBUTING, conventions (G2, G5 +§Handoff), docs/README (routing + index), D01 TEST ×2,
  AGENTS (layout tree + checklist step 2), header `docs/techstack-analysis.md` (untracked).
- **README vòng đời (mới) tại `docs/handoff/README.md`:** VIẾT khi mở task vừa/lớn hoặc phiên
  dở giữa chừng · GHI TIẾP khi còn cùng task (1 task đang mở ≤ 1 handoff) · XOÁ khi task đóng
  (verify exit 0 + tri thức về đủ nhà) trong cùng commit đóng việc, chỉ xoá handoff task mình
  claim, sửa pointer trỏ tới trước khi xoá.
- **Trạng thái:** confirmed (quyết định user; verify.sh cuối task — kết quả trong báo cáo).

---

## 2026-09-27 · QĐ7: TODO queue theo phase + template handoff + vòng đời task + protocol đa-phiên

- **Nhiệm vụ:** user duyệt format TODO (chia section theo phase, priority = thứ tự, dashboard
  đầu file) + placement (`docs/TODO.md` — root chỉ chứa contract), yêu cầu migrate toàn bộ tài
  liệu theo kiến trúc mới rồi commit.
- **Artifacts (mới):** `docs/TODO.md` — seed nội dung thật: PH-0 (T-001 ⏸ chờ user chọn
  phương án phá blocker, T-002/003 ⏜, 3 ✅ 22.09), PH-1 (2 ⬜), META (T-020 add 2 skill theo
  audit = chờ user duyệt; T-021 CI trigger PH-4; 4 ✅) · `docs/_TEMPLATE-handoff.md` (6 section,
  G2 done-criteria trước khi code).
- **Wire-up:** AGENTS.md — "Việc còn treo" → pointer TODO; checklist đầu phiên +bước claim;
  sửa comment stale rules "(migration, evidence, release)" → đúng 4 rules thật ·
  conventions.md — section "Vòng đời task" (6 sân khấu, G1–G5), checklist +claim,
  mục "Handoff vào/ra" + "Protocol đa-phiên" (git add scoped, chụp status, không
  commit/revert thay đổi lạ) · docs/README.md routing + index · vision.md QĐ7.
- **Commit:** batch 1 (QĐ5+QĐ6+mã hóa) = `bbb99b1` trước khi làm task này; batch 2 (QĐ7) commit
  sau verify cuối. `docs/techstack-analysis.md` cố ý untracked (research).
- **Trạng thái:** confirmed (quyết định user; verify.sh 2 lần exit 0 — trước mỗi commit).

---

## 2026-09-27 · QĐ6 bổ sung: mã hóa màn hình trong docs/design + migration

- **Nhiệm vụ:** user yêu cầu tổ chức lại docs/design theo mã màn (`[Chữ][2 số]`, màn con `-n`,
  folder `[Mã]-[tên]`, tài liệu màn con trong thư mục màn chính). Phân tích pros/cons qua
  improve-codebase-architecture (report HTML tạm 27.09): chấp nhận kèm cải thiện — bảng cấp chữ
  trong registry, quy tắc không tái dùng mã, mã dùng thống nhất cho OQ/TC.
- **Bảng chữ user chốt (27.09):** A = trang chủ + thông tin chung (Intro/About/TOS) ·
  B = tài liệu về phần mềm (hướng dẫn) · C = màn dùng chung nhiều màn · D/E/P = DOCX/XLSX/PPTX ·
  M = markdown editor · T = txt editor · X = settings. M/T ngoài roadmap hiện tại (chỉ namespace).
- **Migration (map cũ → mới):** `design/editor/` → `design/D01-editor/` (TC-ED-nn → TC-D01-nn,
  16 chỗ) · `design/startpage/` → `design/A01-startpage/` (TC-SP-nn → TC-A01-nn) ·
  `design/print-preview/` → `design/C01-print-preview/` (top-level cụm common, theo quy tắc C
  của user) · `design/settings/` → `design/X01-settings/` · `design/about/` → `design/A02-about/`.
  Files untracked → `mv` thường, không ảnh hưởng git history. Entry cũ trong file này nhắc đường
  dẫn cũ (TC-ED-06…) — **giữ nguyên không sửa** (log); tra cứu theo map ở trên.
- **Wire-up:** design/README.md viết lại (registry + bảng cấp mã + 12 quy tắc) · 2 template
  thêm trường Mã/Mã cha + quy ước `TC-<mã>-nn` · docs/README.md (routing + index) ·
  conventions.md (`<id>` → `[Mã]-[tên]`, đường dẫn smoke) · vision.md QĐ6 (bullet bổ sung 27.09).
  AGENTS.md/CONTRIBUTING.md không phải sửa (chỉ trỏ design/ chung chung — đã grep).
- **Trạng thái:** confirmed (quyết định user; verify.sh chạy cuối task — kết quả trong báo cáo).

---

## 2026-09-26 · Quyết định 6: hệ tài liệu public DD + TEST theo màn hình + CONTRIBUTING

- **Nhiệm vụ:** user yêu cầu tài liệu detail design (DD) theo màn hình + test plan tương ứng
  làm source of truth, chuẩn public official cho contributor ngoài (kiểm kê 26.09 xác nhận:
  chưa có ARCHITECTURE/DESIGN/Screenlist riêng — kiến trúc sống phân vai ở conventions/vision/
  reference.map, giữ nguyên không gộp).
- **Quyết định user (26.09):** ngôn ngữ **tiếng Việt** · khởi tạo **editor + startpage + 3 stub**
  (print-preview, settings, about) · **CONTRIBUTING.md minimal tạo ngay**.
- **Artifacts (mới):** `docs/design/` — README (screenlist + vòng đời planned→draft→
  in-development→stable→deprecated, 6 quy tắc) · `_TEMPLATE-DD.md` (11 section, cảm hứng
  Google Design Doc + RFC status) · `_TEMPLATE-TEST.md` (traceability DD, trạng thái
  GREEN/RED/BLOCKED) · `editor/{DD,TEST}.md` — chốt open-pipeline + boot-flow từ
  diagnostics/reference.map thành spec; TEST ghi rõ TC-ED-06 = RED (blocker font pipeline)
  · `startpage/{DD,TEST}.md` (draft PH-2) · 3 stub planned. Ngoài ra: `docs/README.md`
  (chỉ mục + bảng định tuyến — lấp gap đã thống nhất các phiên trước) · `CONTRIBUTING.md`
  (root). Wire-up: conventions.md thêm section "DD + TEST", AGENTS.md layout tree,
  vision.md QĐ6.
- **Trạng thái:** confirmed (quyết định user; verify.sh chạy cuối task — kết quả trong báo cáo).

---

## 2026-09-26 · Quyết định 5: chốt "hai ngôi nhà" `harness/` vs `.agents/` (ADR vào vision.md)

- **Nhiệm vụ:** owner hỏi vì sao có `harness/` riêng thay vì gộp vào `.agents/` → chạy
  improve-codebase-architecture (scope chỉ định sẵn), report 3 candidates (HTML tạm, không vào repo).
- **Bằng chứng (confirmed):** zero trùng nội dung giữa hai folder (grep hai chiều 26.09);
  `harness/` từ bootstrap `f5b6f69` (21.09), bị tham chiếu bằng path từ code chạy thật —
  `harness/scripts/verify.sh:20` exec `check-licenses.mjs`, AGENTS.md ×4, comment trong
  `src-tauri/src/{bridge,sidecar,protocol}/mod.rs`, `licensing.md`, `.gitignore`;
  `.agents/skills/` từ `1830d50` (22.09) — path do CLI skills.sh dictate, hash-lock
  `skills-lock.json`, 9 skill import generic, tool-discovery qua frontmatter.
- **Quyết định user (26.09):** chọn card 2 — ghi ADR "hai ngôi nhà" vào `vision.md`
  § Quyết định 5 (cấm merge khi không có bằng chứng mới) + sửa câu chéo `conventions.md`
  § Workflow skills. Card 1 (merge) bác bỏ; card 3 (pointer-skill cho 3 skill harness trong
  `.agents/`) để backlog, kèm gate: probe `npx skills update -p` xem có prune entry lạ không.
- **Trạng thái:** confirmed (quyết định của user; verify gate chạy cuối task, kết quả trong report phiên).

---

## 2026-09-26 · Chốt stack (giữ Rust) — research Bun/Go/Rust

- **Nhiệm vụ:** agent nền thực hiện phân tích Bun vs Golang vs Rust theo
  `docs/handoff-techstack-analysis.md` → đầu ra `docs/techstack-analysis.md` (3.779 từ,
  10 section, decision matrix 3×6, đo thật trên máy).
- **Kết quả:** Rust+Tauri 4.85/5 > Go+Wails 3.60 > Bun 1.90. Số observed: binary release
  Rust 5.0 MB (app 1.141 LOC) vs Wails 7.9 MB vs Bun compiled 61 MB; RSS idle ~181 MB
  (chủ yếu WebKit macOS) vs Wails ~198–218 MB; chỉ Tauri có mobile stable. Phát hiện:
  Bun MIT nhưng binary compiled nhúng JSC/WebKit **LGPL-2.1 static-link** → nghĩa vụ
  relink/source-offer.
- **Quyết định user (26.09):** **không đổi stack** — giữ Rust + Tauri; file phân tích là
  research, KHÔNG commit (để untracked). Đã ghi vào `vision.md` § Định vị (câu hỏi ĐÓNG)
  + `AGENTS.md` § Việc còn treo (dọn stale: scaffold-tmp đã commit từ b60df56).
- **Trạng thái:** reproduced (số liệu trong file phân tích); quyết định = user.

---

## 2026-09-22 (bổ sung) · Phương án 1: phát hiện LoadJS bootstrap contract; oracle CDP app thật; render vẫn chưa qua vạch

- **Bootstrap contract (reproduced — fix có thật):** `word/sdk-all-min.js` chỉ là BOOTSTRAP —
  engine thật `sdk-all.js` được shell nạp qua bridge **`AscDesktopEditor.LoadJS(url)`** (return
  1 sync / 2 async) phối hợp `window.local_load_add(store, name, url)` / `local_load_remove`
  (sdk-all-min `rb()`; callback `completeLoad` phải bắn sau khi eval xong). Thiếu em → runtime
  dual-build inconsistent. Shim đã implement: fetch text → eval → completeLoad → return 2
  (sdk-all.js, fonts.js, ChartStyles.js nạp đúng qua LoadJS).
- **Bẫy wasm scriptDirectory (reproduced — fix có thật):** emscripten glue resolve
  `fonts.wasm` theo `document.currentScript` — khi JS chạy qua LoadJS eval, currentScript =
  null → glue rơi về thư mục PAGE (`documenteditor/main/fonts.wasm` → 404) →
  `RuntimeError: Aborted (both async and sync fetching of the wasm failed)` → font engine
  chết im lặng. Fix 2 lớp: shim set `window.Module.locateFile` (map theo basename) trước mỗi
  eval + filesrv redirect mọi engine payload request sai thư mục về vị trí chuẩn
  (`resolve_engine_payload`). Sau fix: 0 Aborted, wasm init OK.
- **Oracle CDP app thật (observed — phương pháp cho phiên sau):**
  `/Applications/ONLYOFFICE.app/Contents/MacOS/ONLYOFFICE --remote-debugging-port=9222` +
  `open -a ONLYOFFICE <file>` → CDP `Runtime.evaluate` trong editor iframe (frames[0] của
  boot page). Số liệu app thật mở fixture: editor frame có `Asc.editor` ✓, viewer canvas
  **2654×1312** (đang render), `window.native` = undefined, `NATIVE_EDITOR_ENJINE` = undefined
  → **app thật chạy cùng pipeline wasm+canvas, KHÔNG phải native graphics**; không thấy font
  fetch qua network trong frame (entries có thể không ghi với file:// scheme — chưa chốt).
- **Vẫn chưa qua vạch render (candidate):** runtime NanoOffice sau các fix: parse + styles
  preload chạy sâu (56 `II(styleName)` ok — `I3.prototype.gCh` là styles manager, KHÔNG phải
  font loader như suy luận trước), `SetDocumentName` bắn, KHÔNG exception, KHÔNG
  asc_onError — nhưng `id_viewer` vẫn 0 height, không `asc_onDocumentContentReady`, font
  pipeline của FULL build (AscFonts.Wg — glyph manager, KHÔNG dùng `g_font_files`/
  `g_fontApplication` như apiBase cũ) không khởi động. Hệ quả cấu trúc phát hiện thêm:
  `word/sdk-all.js` KHÔNG chứa `DocsCoApi`/apiBase/GlobalLoaders (chúng nằm trong
  sdk-all-min bundle); `DocsCoApi` không tồn tại trong word build → "web-mode cần docsco" là
  suy luận SAI cho word — socket polling `/editors/doc/<key>/c/` đến từ web-apps (Gs class),
  không phải docsco.
- **Bẫy vặt ghi lại:** `safe_join` trả Some(path) dù file không tồn tại → fallback
  `or_else` không bao giờ chạy (dùng `is_none_or(|p| !p.is_file())`); python `str.replace`
  patch file im lặng không làm gì khi pattern lệch — phải verify sau mỗi patch;
  `include_str!` không kích hoạt cargo rebuild khi file include đổi.
- **Đề nghị phiên kế (cho user chọn):**
  (a) tiếp tục oracle-diff: cdp-eval từng trạng thái `AscFonts.Wg` (vue/handle, font stream
  nguồn) của app thật vs của ta ngay sau EndLoad — khai thác còn 1 mù về nguồn font data của
  FULL build; (b) hoặc chấp nhận dùng **EditorForCompare/Editor.bin của chính app thật**:
  mở file trong ONLYOFFICE thật rồi copy `~/Library/.../recovery`/`cache` — không giải quyết
  render; (c) hoặc reverse `AscFonts.Wg` trong full build (9.9 MB, tên đọc được — I3/gCh đã
  tìm đúng vị trí, cần trace font data entry point `Wg.wf`/`Vue` handle).

- **Trạng thái:** observed/reproduced từng phần; render = **chưa đạt** — theo chỉ đạo user,
  phương án 2 (docsco/Gs tối giản) là bước dự phòng, nhưng phát hiện mới cho thấy word build
  không dùng DocsCoApi → phương án 2 cần reverse `AscCommon.Gs` (protocol socket của word),
  không phải docsco — cập nhật lại kế hoạch trước khi làm.

---

## 2026-09-22 · PH-0: engine boot được trong WKWebView; open pipeline treo sau khi nhận Editor.bin

- **Kiến trúc chạy được (reproduced):** engine sdkjs/web-apps (AGPL artifact copy từ
  ONLYOFFICE.app build 9.4.0.129 → `engines/editors/`, git-ignored) boot đầy đủ trong Tauri
  WKWebView qua loopback HTTP `http://127.0.0.1:14321` (Rust `protocol/filesrv.rs`: statics
  `/editors/…` + fonts `/fonts/app|sys/…` + documents `/file/<id>/…`, Range 206, ACAO `*`,
  OPTIONS preflight). Logs: hàng trăm asset 200; `fonts.wasm` tải OK; UI toolbar/ribbon/canvas
  hệ thống hiện; không crash WKWebView → **bằng chứng mạnh: không cần CEF**.
- **Bridge 2 chiều hoạt động:** shim vanilla JS inject `initialization_script_for_all_frames`
  (main frame → boot page iframe → editor iframe, relay postMessage 2 chiều, sync-truth table
  cho method engine gọi sync). Observations (log stdout): `CreateEditorApi`, `IsLocalFile`,
  `GetEncryptedHeader`, `CheckUserId`, `GetInstallPlugins`, `IsSignaturesSupport`,
  `SetDocumentName`, `LocalFileSetSaved/RecoverFolder/SetSourcePath/GetSourcePath`,
  `LocalStartOpen`, `execCommand` (`webapps:entry`, `webapps:features`, `editor:onready`,
  `editor:config`, `title:button`), `IsLocalFile(true)` gates — chi tiết trạng thái từng method
  trong `docs/reference.map.md`.
- **Open contract desktop tái tạo (reproduced):** LocalStartOpen (engine) → x2t convert
  docx→Editor.bin (magic `DOCY`, **28–116 ms** cho fixture 1.9 KB) → deliver base64 + decodedLen
  → `window.DesktopOfflineAppDocumentEndLoad(folder, b64, len)` — khớp chuẩn shell thật
  (desktop-sdk client_renderer_wrapper.cpp `onlocaldocument_loadend` → `GetFileData2` =
  `GetFilePathBase64`). EndLoad chạy qua không exception; `brb` vào nhánh `Uwe` (tRb=true).
- **Điểm treo (candidate, chưa giải mã hết):** sau khi nhận Editor.bin, pipeline open dừng im
  lặng — không `asc_onError`, không `asc_onDocumentContentReady`, KHÔNG có request fonts
  (0 fetch `/fonts/…`), `id_viewer` canvas 2302×0. Data đầu vào đã kiểm chứng tuyệt đối:
  zP.decode → Uint8Array 1586 bytes head `68,79,67,89`; roundtrip x2t Editor.bin→docx giữ
  nguyên nội dung (kể cả tiếng Việt). Nghi vấn chính: một thủ tục async của app-layer thật
  (editor:config answer từ native app, hoặc chữ ký OfflineApp/docsco chưa khớp) — cần trace
  tiếp bằng devtools engine frame hoặc so trace app thật.
- **Bẫy đã gặp (đề phòng tái diễn):** (1) `read_to_end` vào buffer pre-fill `vec![0u8; len]`
  → response bị nhân đôi NUL (api.js SyntaxError) — có unit test hồi quy; (2) engine từ chối
  document URL scheme lạ (`getUrlType` → Invalid, allowlist `https?|ftps|file|tessa|joplin|smb`
  — editorscommon.js:2939) → KHÔNG dùng được `nanooffice-file://` làm document URL của engine,
  phải qua loopback HTTP (deviation kiến trúc, xem reference.map); (3) custom-scheme page
  (secure context) fetch `http://` = mixed-content block → engine + document phải cùng origin
  loopback; (4) `include_str!` không kích hoạt rebuild khi file JS đổi — phải `touch`/build tay;
  (5) vite 8 default port 5173 ≠ devUrl 1420 — `vite.config.ts` pin port + strictPort.
- **Baseline PH-0 (debug build, dev server — số chưa phải release):** shell binary 30 MB
  (debug); engines payload 313 MB (full copy, chưa trim per-app — budget ≤100 MB là việc
  PH-4/trim); RAM idle start-page: process shell 133 MB RSS (devtools mở, debug build;
  node vite ~160 MB là dev-only không ship). ONLYOFFICE.app cài sẵn: 1.1 GB disk,
  ~1.07 GB RAM / 7 processes (đo 21.09). Cold start → start page loaded ~1–2 s (dev).
  → Đủ dữ liệu sơ bộ cho Quyết định 2, chốt số release khi PH-4.
- **Trạng thái:** observed/reproduced (các phần nêu trên); open pipeline = **candidate-treo**,
  chờ user quyết hướng debug tiếp.

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
