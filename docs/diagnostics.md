---
type: log
---
# docs/diagnostics.md — nhật ký chẩn đoán

Quy tắc: mỗi entry ghi thời điểm, triệu chứng, bằng chứng, trạng thái
(observed / candidate / reproduced / confirmed). Entry mới thêm TRÊN CÙNG.

---

## 2026-10-11 · T-063: Surface document hand-off keyed theo window_label (Candidate 3 review #7)

- **Nhiệm vụ:** Thay thế 2 struct `QuickViewRequest` và `NanoLightRequest` độc lập (dùng pattern `Mutex<Option<String>>` unkeyed) bằng cơ chế tra cứu theo `window_label`.
- **Thiết kế sâu:**
  - Tạo struct `SurfaceHandoff` trong `src-tauri/src/window.rs` dùng `Mutex<HashMap<String, String>>`.
  - `stash(label, path)` và `take(label)`: Keying trực tiếp theo nhãn cửa sổ (`quickview-1`, `nanolight-1`, v.v.).
  - Cả Quick View và Nano Light dùng chung một managed state `SurfaceHandoff` duy nhất trong `lib.rs`.
  - Bổ sung 2 unit tests cho `SurfaceHandoff` trong `window.rs`.
- **Kết quả:**
  - Xóa bỏ hoàn toàn 2 struct `QuickViewRequest` và `NanoLightRequest` unkeyed; loại bỏ triệt để nguy cơ race condition khi mở đồng thời nhiều tài liệu.
  - Gate 13 (quick-view UX wiring) và Gate 15 (Nano Light editor reopen) đều PASSED; cargo test 72 passed.
  - `./harness/scripts/verify.sh` FULL exit 0 cả 20 gates.

---

## 2026-10-11 · T-062: Atomic save seam an toàn đĩa giữa bridge/save.rs và editor/mod.rs (Candidate 2 review #7)

- **Nhiệm vụ:** Gom logic an toàn file đĩa (tạo file tạm sibling cùng volume, kiểm tra output không rỗng, và rename nguyên tử chống corrupt file) bị phân mảnh giữa `bridge/save.rs` và `editor/mod.rs`.
- **Thiết kế sâu:**
  - Tạo module sâu `src-tauri/src/storage.rs`:
    * `sibling_tmp_path`: Quy tắc tạo đường dẫn file tạm cùng thư mục cha (cùng filesystem volume).
    * `atomic_swap`: Hoán đổi nguyên tử file tạm đã sinh ra đè lên file đích, tự động dọn dẹp file tạm khi fail hoặc khi file rỗng.
    * `atomic_write_bytes`: Ghi mảng byte bộ nhớ qua file tạm rồi rename nguyên tử.
    * `atomic_save`: Closure writer seam linh hoạt.
    * Bổ sung 5 unit tests độc lập cho `storage.rs`.
- **Kết quả:**
  - `bridge/save.rs` và `editor/mod.rs` chuyển sang dùng `crate::storage`; xóa bỏ hàm `atomic_replace` cục bộ.
  - Gate 10 (smoke engine save) và Gate 15 (smoke Nano Light save) đều PASSED; cargo test 70 passed.
  - `./harness/scripts/verify.sh` FULL exit 0 cả 20 gates.

---

## 2026-10-11 · T-A33: A5 mở màn UI riêng (DD U01 design-first + slice 0 Nano Light chrome)

- **Nhiệm vụ:** Mở màn Milestone A5 — tự chủ UI editor thay web-apps cho các surface của mình (DD U01 + slice 0).
- **Quyết định (DD U01):**
  - Framework RATIFY: Slice 0 chạy Vanilla TypeScript + Tailwind v4 (0 runtime dep, 0KB overhead); trigger kích hoạt Preact (~4.5KB) khi bước vào component stateful phức tạp (Tab bar đa tài liệu hoặc Toolbar định dạng văn bản) vượt ngưỡng 100 dòng manual DOM sync. Loại Svelte 5 vì agent-fluency thấp hơn JSX.
  - Theme values: Chốt bảng màu Graphite neutral-minimal (zinc/slate tones: `--color-bg: #f4f4f5`, `--color-fg: #18181b`, `--color-panel: #ffffff`, `--color-panel-active: #e4e4e7`, `--color-edge: #d4d4d8`, `--color-accent: #2563eb`, radius `6px`). Dark mode tự động theo prefers-color-scheme.
  - Kiến trúc chrome: Header chrome bọc ngoài viewport canvas (`editor.html`), hiển thị `#doc-name`, `#dirty-dot`, `#save-btn` gọi `nano_light_save` thật.
- **Bằng chứng:**
  - DD U01 commit trước code (`8b3aa2b`); check-docs 33 files GREEN.
  - Gate 15 smoke Nano Light: first-draw 19.0 ms, save 2.9 ms, positive PASSED + negative garbage/doctor exit 1.
  - Gate 12 & 13 (quick-view) không thoái hóa (first-draw 15–81 ms).
  - `./harness/scripts/verify.sh` FULL exit 0 cả 20 gates.

---

## 2026-10-11 · T-064: DocumentSurface host container (Candidate 4 review #7)

- **Nhiệm vụ:** Rút trích container chung cho canvas viewport/resize/dpr trước khi chrome Milestone A5 phân nhánh giữa viewer và editor.
- **Thiết kế sâu:**
  - `src/viewer/surface.ts`: Class `DocumentSurface` đóng gói toàn bộ vòng đời canvas viewport:
    khởi tạo context 2D + dpr, đo đạc kích thước viewport linh hoạt theo container hoặc window (`vw`/`vh`),
    điều phối `layout()` + `draw()` + `layoutAndDraw()`, đếm pixel `countInked()`,
    hiển thị lỗi fatal `showFatal()`, và banner cảnh báo `showWarnings()`.
  - `src/viewer.ts` và `src/editor.ts` chuyển sang consume `DocumentSurface`: toàn bộ boilerplate lặp lại
    về canvas context, DPR, resize debouncing, pixel sampling loop và fatal/warning banner biến mất.
- **Kết quả:**
  - Vite tự động phân rã và tách `surface` thành shared bundle chunk riêng (`dist/assets/surface-*.js`)
    dùng chung cho cả `viewer.html` và `editor.html`.
  - Gate 12 (quick-view metrics), Gate 13 (quick-view UX wiring E2E) và Gate 15 (Nano Light editor round-trip)
    đều PASSED với các chỉ số first-draw nhanh (16–88 ms).
  - `./harness/scripts/verify.sh` FULL exit 0 cả 20 gates.

---

## 2026-10-11 · T-046: module config `config.rs` + `window.rs` gom 10 env knobs & window factory seam (Candidate 1 review #7)

- **Nhiệm vụ:** Gom các biến môi trường cấu hình launch/boot/probes còn rải trong `lib.rs`
  về 1 nhà và chuẩn hóa seam dựng cửa sổ (Candidate 1 trong Architecture Review #7;
  đóng nốt task code cuối cùng của phase PH-1).
- **Thiết kế sâu:**
  - `src-tauri/src/config.rs`: Struct `AppConfig` gom 10 biến (`NANO_OFFICE_BACKGROUND`,
    `NANO_OFFICE_OPEN`, `NANO_OFFICE_DEVTOOLS`, `NANO_OFFICE_QUICK_VIEW`, `NANO_OFFICE_NANO_LIGHT`,
    `NANO_OFFICE_PROBE`, `NANO_OFFICE_SAVE_PROBE`, `NANO_OFFICE_UX_PROBE`, `NANO_OFFICE_NL_PROBE`,
    `NANO_OFFICE_QV_HITTEST`). Hàm pure `from_lookup` cho phép unit test độc lập không race env.
    `startup_document` chuyển sang `Mutex<Option<String>>` với `take_startup_document()`,
    xóa bỏ `std::env::remove_var` bẩn.
  - `src-tauri/src/window.rs`: Seam `create_boot_window` và `open_surface_window` (dùng chung cho
    viewer và editor). Tự động áp dụng Rule 6b background offscreen sliver và tiêm probe scripts.
- **Kết quả:**
  - `src-tauri/src/lib.rs` co từ 490 xuống 330 dòng; hook `setup()` giảm từ 140 dòng xuống 25 dòng.
  - 4 unit tests mới cho `AppConfig` trong `config.rs`.
  - `./harness/scripts/verify.sh` FULL exit 0 cả 20 gates (cargo check/fmt/clippy, pnpm build,
    gate 14 unit test 114 checks, 7 smoke behavioral gates).

---

## 2026-10-11 · T-047: dialog "An error has occurred…" = catch-all uncaught-error của engine, KHÔNG phải lỗi mở file

- **Root cause (confirmed bằng code-chain, full bằng chứng file:line ở reference.map §
  dialog mở-file 11.10):** `sdkjs/common/apiBase.js:356-386` (`sendUnhandledError`) map
  MỌI uncaught error/rejection ĐẦU TIÊN (wiring one-shot `window.onerror` +
  `onunhandledrejection`, `apiBase.js:379-424`) trước `isDocumentLoadComplete=true`
  (`apiBase.js:1511`, chỉ đặt trong `onDocumentContentReady`) thành
  `ConvertationOpenError` Critical → web-apps `Main.js:241/2176/2213-2216/2424-2442`
  render thành đúng dialog "An error has occurred while opening the file / Press OK to
  close the editor". Vì handler one-shot tự khôi phục sau lỗi đầu, open VẪN TIẾP TỤC →
  contentReady bắn + render hoàn chỉnh ĐẰNG SAU dialog — đúng ảnh 27.09. Dialog ≠ open fail.
- **Sửa nhận định 27.09 (T-002):** `iLa` KHÔNG phải emitter viewer api — là hằng error-id
  minified (`Kd.ConvertationOpenError=Kd.iLa`, sdk-all-min.js); build word chứa 0 call-site
  `AscViewer.open` (grep). Lỗi đi đúng đường `asc_onError` của api chính — đường
  `showerror` (Main.js:259) loại (chỉ ComboSeriesError). Mọi raise-site
  `ConvertationOpenError` khác loại cho flow offline (bảng loại trừ ở reference.map).
- **Không tái hiện được tự nhiên:** 0/15 boot gate 10-11.10, 0 dòng lỗi trong
  /tmp/nanooffice-smoke-* (trừ `probe decode failed` — do MÌNH catch sẵn). Trigger cụ thể
  (lỗi lạc nào) chưa định danh — tái hiện cưỡng bức cần env riêng → phải đụng lib.rs
  (ngoài scope T-047 theo GUARD) → seed.
- **Instrument đã land (shim của mình, ADDITIVE):** `desktopEditor.js`
  `installErrorForwarding` gắn `| stack: …` (≤500 chars) vào log uncaught error +
  unhandled rejection → lần occurrence kế cho ra trigger + stack ngay trên stdout
  (trước đây chỉ có message @ file:line:col, thiếu call-chain).
- **Trạng thái:** root cause confirmed (code-chain + khớp triệu chứng); trigger = candidate
  chờ bằng chứng runtime; seed task cho trigger-hunt (+ cân nhắc env-gated dialog-probe
  counterfactual khi đụng được lib.rs).
- **Flake verify quan sát được (2/4 FULL run sáng 11.10, KHÔNG do diff T-047):** run 1 gate
  15 fail `paste result 'Xin Xin chàochào' not in parse-back` — copy-all TRONG NHỚ đúng,
  file lưu parse-back thành `'Xin rXin chàochào'`; gate 15 standalone ×3 PASS + run cuối
  FULL GREEN trên cùng tree (diff T-047 = shim JS không load ở Nano Light + docs). Run fail
  có `op=copy sys=ok`, run pass `sys=fail` (đường clipboard hệ thống vs stash) — nghi class
  flake boot-gate/timing (bài học T-050); run 2 fail step sớm (log mất — bẫy `| tail` cắt
  evidence verify dài: bắn full log ra file từ đầu). Theo dõi: tái diễn → mổ riêng.

---

## 2026-10-11 · T-049: save multi-format XLSX/PPTX wired — gate 16 tầng x2t, edit-probe cell/slide PARKED

- **Nhiệm vụ:** save chỉ docx (gate 10) — xlsx 257 known-unwired, pptx unidentified.
- **QĐ1 — pptx = fallback-extension:** KHÔNG truyền `m_nFormatTo` (514/515/516/512/1025/
  8193/4097/258 đều fail từ oracle 29.09), extension output `.pptx` quyết định; variant mới
  `SaveCode::WiredByExtension`, xoá `KnownUnwired`+`Unidentified` (0 consumer còn lại);
  pdf = `NotWired` (pipeline khác `localSaveToDrawingFormat` — OQ-F01-1, 513 là export-PDF).
- **QĐ2 — gate 16 ở tầng x2t, không boot app:** engine edit-probe cell/slide KHÔNG có API
  public string-keyed — `asc_enterText` có trong source (`cell/api.js:9102`, `slide/api.js:8766`)
  nhưng 0 hit trong build `engines/editors/sdkjs/{cell,slide}/sdk-all-min.js` (chỉ word có
  export entry); `asc_Save` no-op khi doc chưa sửa (`apiBase.js:3130` guard
  `asc_isDocumentCanSave()/_haveChanges()`) → "save-only qua engine UI" không đáng tin.
  Gate = round-trip fixture thật → Editor.bin → params XML từ CHÍNH builder thuần mới tách
  trong `bridge/save.rs` (không drift) → assert zip root part (`xl/workbook.xml` /
  `ppt/presentation.xml`); negative = Editor.bin rác phải fail + pdf vẫn refuse.
- **Bằng chứng:** gate 16 3/3 exit 0 (~1.6 s); cargo test --workspace 44 shell (1 ignored)
  + 11 + 6 crate, 0 failed; verify FULL exit 0 (19 ▸ 20 steps) ×2.
- **Gotcha:** x2t "Couldn't create temp folder" khi dir đích chưa tồn tại (test quên
  `create_dir_all` workdir — app làm sẵn ở delivery); exit code cargo test qua pipe zsh
  đọc `${pipestatus[1]}` (cắn lần 2 — lesson cũ tái diễn).
- **Ceiling:** from-changes=true cho xlsx/pptx chưa chạy end-to-end (đợi engine edit-probe —
  DoctRenderer apply cho cell/slide untested; font params đã truyền như docx). PDF save
  OQ-F01-1. Spec-ref `AVS_OFFICESTUDIO_FILE_*` cho pptx code đúng chuẩn = việc sau.

## 2026-10-05 · Setup: gate-cmds 1 nhà + dev-check + pin 5 format mark + conventions vận hành (từ tự-review 04.10)

- **Nguồn:** tự-review phiên T-055 #6 + T-V6 — 4 verify lãng phí, ~1 giờ parity chạy lại,
  2 edit hỏng file; báo cáo HTML `$TMPDIR/architecture-review-20261004-setup.html`;
  user chọn cả 3 candidate.
- **C1 — `harness/scripts/gate-cmds.sh` + `dev-check.sh`:** lệnh gate (cargo
  check/fmt/clippy/build/gate-14) là HẰNG SỐ 1 nhà, `verify.sh` source + `eval` (log
  label nguyên văn); `dev-check.sh` chạy cùng lệnh KHÔNG boot app (~4 s đo thật) —
  "local check" không thể trôi khỏi gate (sự cố fmt `--all` pass ảo ×2 + E0063 sót
  workspace chỉ lộ ở gate 15). verify.sh FULL vẫn phán quan cuối.
- **C2 — pin đủ 5 ST_NumberFormat §17.18.59** trong
  `resolve_numbering_marks_formats_counters_and_separator`: decimal/lowerRoman/bullet
  (cũ) + lowerLetter "a)" / upperLetter "A." / upperRoman "I." (mới) — blind-spot
  trước đây chỉ parity (5–40 phút) phủ, giờ unit 4 s.
- **C3 — `conventions.md`:** mục "Kiểm chứng mỗi vòng" thêm dev-check (1 nhà, không tự
  ghép lệnh); mục mới "Vận hành agent" 4 bullet: old_string copy từ file thật · đổi
  model Rust → check toàn workspace · `git status` trước commit add-tay (tree đa-
  phiên) · corpus NOTES.md = spec đầu vào + parity không deadline.
- **Loại có lý do:** parallel hóa parity (contention tạm — YAGNI) · pre-commit hook
  (local-only) · tạo CONTEXT.md (phân tán nhà ngôn ngữ — vision/conventions đã là nhà).
- **Bằng chứng:** dev-check exit 0 (4 s) · crate 11+6 rc=0 (5 format + separator) ·
  verify FULL exit 0 (chính verify.sh đã source gate-cmds — test sống của C1).
- **Gotcha:** diagnostics.md dùng chung với session song song (entry Rule 6b của họ
  chưa commit) — stage hunk của mình bằng strip-script, giữ nguyên working copy của họ.

## 2026-10-04 · Rule 6b enforce: Accessory activation policy set TRƯỚC run (hết cướp focus khi gate boot app)

- **Triệu chứng (user báo 04.10):** mỗi lần agent boot app để test, màn Terminal mất
  focus — phải Cmd-Tab lại. `visible(true)+focused(false)` (enforce 03.10) chỉ dừng
  WINDOW lấy key, không dừng PROCESS activate.
- **Nguyên nhân (đọc tao 0.35.3 source):** `app_state.rs::launched()` (=
  `applicationDidFinishLaunching`) chạy `apply_activation_policy` (đọc aux-state,
  default Regular) → `window_activation_hack` (`makeKeyAndOrderFront` mọi window
  visible) → `activateIgnoringOtherApps(true)` — app Regular activate lúc launch,
  TRƯỚC khi tauri setup chạy.
- **Bẫy:** tauri `setup` chạy tại `RunEvent::Ready` — BÊN TRONG event loop, SAU
  launched → `set_activation_policy` trong setup (cả qua `app.handle()`, message
  xử lý sau) là MUỘN, vẫn steal (đo được: app giữ front ~6 s). Đường đúng:
  `Builder::build()` → `App::set_activation_policy(&mut)` khi runtime còn `Some` →
  ghi thẳng aux-state delegate → `launched()` áp Accessory TRƯỚC activate
  (`lib.rs::run`, env `NANO_OFFICE_BACKGROUND=1`).
- **Wire 1 nhà:** `smoke-lib.sh::smoke_boot` prepend `NANO_OFFICE_BACKGROUND=1`
  (đầu mảng envs — env trùng tên sau đó vẫn ghi đè) → mọi gate boot tự nền.
- **Bằng chứng:** `lsappinfo list` app `type="UIElement"` (Accessory applied);
  System Events `frontmost` 27/27 mẫu (300 ms) = Terminal xuyên boot + mở window;
  gate 12 GREEN (first-draw 27 ms, RSS 118 MB); verify FULL exit 0.
- **Gotcha đo đạc:** `lsappinfo front` thỉnh thoảng báo app UIElement là "front"
  (z-order layer) — chỉ tin System Events `frontmost` (menu-bar owner thật).
  Full-hide vẫn cấm (engine save-probes throttled — 03.10); Accessory giữ window
  visible nên probes nguyên vẹn.
- **Follow-up cùng ngày — offscreen sliver (user chốt phương án A):** Accessory chỉ
  bỏ activation, window vẫn `orderFront` đè lên màn (tao window.rs:631). Fix: bg mode
  → `decorations(false)` + `position(sw-8, sh-8)` trên primary monitor
  (`lib.rs::bg_sliver_position`, áp cho cả 4 builder — 3 trong setup + `viewer.rs::
  open_surface_window` cho window UI mở runtime). Invariant: **≥1 pixel lộ = full
  render** (WebKit check occlusion theo-window, không theo-area; full-occluded mới
  throttle — T-050/full-hide 4fe56f9), position-only nên `innerWidth/innerHeight`
  không đổi → số liệu frozen của gate 12/15 byte-identical. Bằng chứng hình học:
  CGWindowList (không cần quyền assistive) — screen logical 1440×900 → window
  bounds X=1432 Y=892 W=1000 H=700, sliver 8×8 góc dưới-phải, còn lại ngoài màn;
  CGWindowList `.optionOnScreenOnly` vẫn liệt kê → WindowServer tính là on-screen.
  Lưu ý đo: System Events đọc `position of window` cần Accessibility (bị từ chối
  -1719) — dùng swift CoreGraphics thay thế.

## 2026-10-04 · T-055 #6: review kiến trúc post-rung A4 (3 reviewer song song — 7 LAND / 4 DEFER / 7 REJECT)

- **Bằng chứng (3 commit, mỗi cụm 1 lần verify FULL exit 0):** `712bee4` TS C1+C2+C3
  (Hit seam: hitTest trả `{caret,line,run,offset}` — round-trip `caretXY(hitTest(x,y).caret)`
  không glue, xoá pi khỏi Hit, caretXY full-identity bắt buộc; evidence/log builders
  `layoutMetricLine`/`mediaEvidenceLine`/`drawFatal` 1 nhà render.ts — byte-identical
  chuỗi frozen; `ordered()` 1 nhà invariant thứ tự selection) → gate 14 **103 checks** +
  verify FULL exit 0 · `3a8df66` Rust C8 — `editor/mod.rs` (1691) + `editor/mutations.rs`
  (462, 12 fn `pub(super)` thuần `Vec<Block>+Caret→Caret`, move verbatim, tests dùng lại
  qua `use super::*`) → cargo 41 rc=0 · clippy lib 0 · verify FULL exit 0 · `2cad6ff`
  crate C1+C4+C5 — OPC plumbing 1 nhà `opc.rs` (`load_rels/find_target/part_dir/
  file_name/RelsList/utf8` + **`main_document_part()`**: invariant "main part là data"
 từ 3 shape/3 error-policy → 1; parse_docx giữ content-type check; detect map
  err→no-warn; writer bỏ dup + `crate::` gọi chéo) + docs invariant hyperlink
  (`rel_id` ⇒ `target`/`external`, writer REFUSE — error mode là phần interface) + gộp
  2 warn-door arm (flag `tracked|tblstyle`) → cargo + crate rc=0 · gate 15 positive
  exit 0 + warndoor/tbldoctor exit 1 · **verify FULL exit 0 ALL CHECKS PASSED**.
- **DEFER có trigger:** Rust C9 gộp surface shell viewer↔editor (surface thứ 3 / C4 #5)
  · TS C5 selection-math module (consumer thứ 2 đọc body theo caret) · TS C6 clipboard
  drift seam — range↔text 2 ngữ + ordering key 2 implementation + paste N-undo-unit
  (clipboard fidelity) · Crate C2 PackageAcc gom 7 `&mut` (kênh state mới trong
  write_docx). **REJECT có deletion-test:** session-lifecycle module · UndoStack type ·
  gộp atomic_replace · overlay draw module · detect_save_losses tách module/parser-
  driven · tests/parse.rs tách file · table-driven negatives + doctor-lib chung (1
  adapter — tiền lệ C4 #5).
- **Normal-review findings (không sửa trong review):** (1) paste chứa `\t`: TS
  pasteFromClipboard chỉ split newline, không lọc control chars; Rust apply_insert từ
  chối `\t` → paste nhiều dòng có tab đứt giữa chừng, phần trước đã chèn (không
  transaction) — fix thuộc TS C6 trigger. (2) Silent-drop policy: `w:hyperlink` trong
  `mc:Choice` + `mc:AlternateContent` block-level (textbox `w:pict`/`w:txbxContent`)
  bị skip KÈM text, `detect_save_losses` chưa có pattern → mất mà warn không bắn
  (policy gap matrix §5.1 — hẹn slice detect kế). (3) clone-per-keystroke + resolve()
  toàn model mỗi edit — chủ đích DD L01 §2, theo dõi khi document lớn. (4) comment
  single-slot race sai phạm vi — ĐÃ sửa trong C8.
- **Gotchas:** (1) `cargo fmt --check` của verify dùng `--all` (workspace) — chạy fmt
  thủ công KHÔNG `--all` bỏ lỡ member `nanooffice-ooxml` (2 lần verify exit 1 từ
  blank-line thừa ở lib.rs sau xoá helpers — bài học: fmt luôn `--all` bằng đúng lệnh
  của gate). (2) Edit tool khớp fuzzy theo text — chép old_string TỪ file thật, không
  gõ lại từ nhớ (2 lần: render.ts columnWidth displaced, opc.rs attr body typo).
  (3) Bulk-regex trên layout.test.ts bỏ lỡ literal chứa `cell: [0, 0]` (comma trong
  `[^,]+`) — 2 tie-break assert sửa tay; pattern bulk phải liệt kê đủ hình dạng.
- **Report:** `$TMPDIR/architecture-review-20261004-a4.html` (đã mở cho user).

## 2026-10-04 · T-A32: A4 closure slice — tblPr/tcPr detect+warn (đóng dòng mất-im-lặng cuối matrix §5.1) + ĐÓNG RUNG A4

- **Bằng chứng (2 commit: 321c68f design → a2381b5 crate+gate15):** gate 15 standalone ×3 —
  positive `table fidelity` **exit 0 KHÔNG warn** (bảng fixture không tblPr → exact-boundary
  pattern đúng) · **tbldoctor exit 1** với warn thật hiện nguyên trong failure message
  (`warnings ['save will drop: table styling']`) — chứng minh detect fired trên đường open
  thật + gate ĐỌC nội dung warn · **warndoor regression exit 1** (real 'tracked changes'
  không thoái hóa). verify FULL exit 0 **ALL CHECKS PASSED** (9 smoke + gate 14, 101 s);
  cargo test crate 6/6 (detect test +styled-table +tcPrChange lookalike).
- **Thiết kế chốt (DD L01 §5.7):** detect KHÔNG phải model — tblPr/tcPr vẫn skip trong
  parser/writer (`document.rs`: w:tbl §17.4.38 "tblPr skipped" / w:tc §17.4.66 "tcPr
  skipped"); criterion nâng của dòng tblPr/tcPr ("khi edit bảng thật đòi, cùng lúc bỏ
  refuse §2") đã CHÁY từ T-A30 → fix = `detect_save_losses` +pattern `<w:tblPr>`/`<w:tblPr `
  + `<w:tcPr>`/`<w:tcPr ` (khớp đúng biên tag). **Warn BẮT CẢ file không bị sửa**: save
  Nano Light drop tblPr/tcPr của MỌI bảng bất kể user đụng đâu — đúng semantic OQ-L01-2
  (cảnh báo mất-khi-save, không phải những gì user vừa làm).
- **Gotchas:** (1) `<w:tblPrChange>`/`<w:tblPrEx>` (revision machinery) bắt đầu bằng
  `<w:tblPr` nhưng KHÔNG khớp pattern đúng biên (ký tự kế là `>`/space) — test lookalike
  chốt; self-closing `<w:tcPr/>` cũng không dính (tcPr rỗng = không gì để mất — đúng).
  (2) fixture gate T-A30 `TABLE` cố tình KHÔNG có tblPr → positive gate 15 không bị warn
  mới cắn — kiểm tra fixture TRƯỚC khi đụng detect (grep generator trước code). (3) warn
  taxonomy dedupe theo label nên 2 pattern cùng label gộp 1 dòng warn.
- **ĐÓNG RUNG A4 04.10 (user chốt):** criterion parity matrix §5.1 = 9 lớp GIỮ + dòng mất
  còn lại toàn bộ detect+warn; OQ-L01-2/3 ĐÓNG; **OQ-L01-1 default-switch DỜI A5** —
  vision.md row A4 sửa bỏ chữ "mặc định". Kế tiếp: T-055 #6 review kiến trúc post-rung;
  T-V6 khi parity đòi.

## 2026-10-04 · T-A31: A4 slice #4 — save-fidelity: hyperlink/sectPr vào model + settings/theme/docProps passthrough + refuse-warn (ĐÓNG OQ-L01-2)

- **Bằng chứng (4 commit: 9823571 design → 24158d6 crate → 61c617f editor+gate15 →
  b5ecd3c cuts):** gate 15 positive exit 0 ×2 (first-draw 50.0/99.0 ms / parse
  7.8/4.7 ms · save 3.2 ms / 4503 B · parse-back: link target + **T31LINK trong link
  run** + anchor nội bộ + mid sectPr pgSz/pgMar · settings/theme/docProps sha256 ==
  snapshot · rels `TargetMode="External"` · positive không warn) + **negative 7/7
  exit 1** (5 cũ re-proven trên fixture fidelity + **linkdoctor** + **warndoor** —
  warn thật `'save will drop: tracked changes'`, assert đòi 'hyperlinks' cắn).
  parity-a2 exit 0 ×2 (11 file, hard-fails 0, INVARIANT 11/11, check (e) passthrough
  mới; diff fidelity 4 dòng = 2 lớp vô hại cũ: hex lowercase + rel_id renumber).
  verify FULL exit 0 **19 ▸ steps** ×2; gate 12 số KHÔNG đổi (16 dòng · 601.7 ·
  600.5 · 750,-250,220 · lhs 17.9 · RSS 133 MB); gate 14 103 checks; **layout.ts 0
  diff — parity-view không chạy**; cargo 41 shell lib (+1) + 16 crate (+4) rc=0 tuần
  tự; **bonus**: smoke-render file-NL-đã-save trong ENGINE exit 0 (writer mới sống
  qua x2t/canonical).
- **Thiết kế chốt (DD L01 §5.6):** hyperlink = `Run.link` (flatten nhóm `w:hyperlink`
  §17.16.22 — runs giữ địa chỉ hitTest, 0 dòng mutation đổi, enter nhân bản link cả
  2 nửa, gộp run giữ link run ĐẦU); sectPr = `PageMar` + `Paragraph.sect` (Box —
  clippy large_enum_variant, None = 0 chi phí) + `Document.page_mar`; enter giữ sect
  ở HEAD (không nhân bản section break); passthrough = writer-side discovery trong
  `media_src` (model + JSON CLI bất biến, content-type verbatim từ nguồn, skip-im
  khi nguồn thiếu/hỏng); refuse-warn = `detect_save_losses` byte-scan (ins/del/
  fldChar/instrText/footnote/endnote/sectPr-lạ) → log + banner + payload `warnings`.
  **OQ-L01-2 ĐÓNG: mở + cảnh báo, không refuse** (D1). **GO sectPr** (ước lượng nửa
  ngày < ngưỡng 1 ngày — không rơi nhánh fallback).
- **Matrix §5.1:** 6 dòng MẤT → GIỮ (hyperlink · sectPr giữa · pgMar cuối · settings ·
  theme · docProps) + tracked/footnotes/fields lên warn. **Còn MẤT (trung thực)**:
  tblPr/tcPr · nội dung tracked/footnotes/fields · con khác của sectPr (warn) —
  đánh giá đóng rung A4 để orchestrator/user chốt (khuyến nghị: matrix đủ để đóng
  "save-fidelity văn bản cơ bản"; slice kế nếu muốn = selection render styling link
  hoặc T-V6 parity render).
- **Gotchas phiên:** (1) **bug `_rels/.rels` passthrough im lặng** — gọi
  `load_rels(src, "", ".rels")` sinh `_rels/.rels.rels` (fn là rels CỦA PART, tự thêm
  `.rels`) → root rels rỗng → passthrough copy 0 part, KHÔNG lỗi (thiết kế skip-silent
  che bug) — phát hiện vì gate tự-so sha cắn ngay lần chạy đầu; fix = đọc
  `_rels/.rels` trực tiếp (cùng bug ở `main_part` của detect). Bài học: skip-silent
  phải đi kèm gate khẳng định-side (check (e) parity-a2 + sha256 gate 15). (2)
  **clippy sau mở model**: `large_enum_variant` (Section 184 B vào Paragraph) →
  `Option<Box<Section>>`; `type_complexity` trên tuple trả về 3 phần tử của
  `parse_body` → struct `BodyParse` — 2 lint bắt đúng chỗ đáng cảm ơn. (3) **pgSz
  clause lệch số**: mirror open-xml-docs (ISO/IEC 29500-1 1st ed) bảng con của
  sectPr ghi pgSz §17.6.13 / pgMar §17.6.11, trong khi comment crate cũ (T-A21)
  ghi pgSz §17.6.11 — ghi nhận, không sửa comment cũ trong slice (K01 §5.1 bổ sung
  ghi LƯU Ý). (4) fixture `make_docx_layout.py` +flag `fidelity` chèn LINK/SECT
  para TRƯỚC MEDIUM để `lastTop` (triple-click T-A30) vẫn trúng MEDIUM — sai chỗ
  là assert "bundled font metrics" vỡ. (5) probe T31LINK dùng word-token 'fidelity'
  (tokens không nguyên câu — item 'fidelity link' không tồn tại, hitTest theo item
  mid-x + baseline dòng).
- **Còn kế tiếp (đánh giá đóng rung A4 theo §5.1 / T-055 #6 nếu user chốt):** matrix
  còn dòng warn-only + tblPr/tcPr + T-V6; default-switch = OQ-L01-1 (A4 muộn/A5).
- **Trạng thái:** confirmed — bằng chứng trong DD L01 §5.6(g) + báo cáo phiên.

---

## 2026-10-03 · T-A30: A4 slice #3 — double/triple-click + blink + mở edit trong bảng single-cell (bỏ refuse §2)

- **Bằng chứng (3 commit: f1e8409 design → 3f35db4 ts+rust → f8da488 ponytail
  cuts):** gate 15 positive exit 0 (first-draw 66.0 ms / parse 8.4 ms · body
  blocks 7 = 6 + bảng · selection span 4 = dblclick "Beta" · save 2611 B /
  1.9 ms · parse-back: `T30B`/`T30P`/`T30C`/`T30C2` có, "Beta" +
  "bundled font metrics" MẤT, JSON cell A 2 para + cell B "T30B cell" · media
  byte-exact · reopen render marker) + **negative ×5 exit 1** (garbage ·
  doctor · seldoctor · clipdoctor nguyên vẹn + **celldoctor** — assert cell A
  1 para cắn trên cấu trúc thật 2 para). verify FULL exit 0 **19 ▸ steps** ×2
  (trước + sau cuts); gate 12 số KHÔNG đổi (16 dòng · 601.7 · 600.5 ·
  750,-250,220 · lhs 17.9 · RSS 252 MB); gate 14 **93→103 checks**; **parity
  6/6 exit 0 ×2 px-identical** (0.64/4.07/0.00/6.47/2.73/2.73, maxdx 47.00 —
  layout.ts đụng, cuts = comment-only); cargo 40 shell (+3 cell −1 obsolete)
  + 12 crate rc=0 tuần tự; clippy -D warnings sạch.
- **Thiết kế chốt (DD L01 §5.5):** `cellPara` additive 3 tầng — Rust `Caret`
  +`cell_para` serde-default; layout `walk` chỉ stamp cellPara cho cell CẤP 1
  (nested = null → refuse, chặn silent-wrong-cell); `caretXY` full-identity
  refine khi caret mang cell (legacy block-only byte-identical). Rust verbs
  generic trên `Vec<Block>` + `dispatch` rebase/unrebase (para ↔ cellPara);
  refuse mixed Cell↔top/Cell↔Cell, same-cell cross-cellPara OK. dblclick =
  click-count thủ công 500 ms/4 px (WebKit không có triple event; probe gọi
  `mouseDownAt(x,y,extend,clicks)` — tiền đề offsetX); word = expand non-space
  trong run text (tokens có sẵn, không regex mới). Blink 500 ms repaint từ
  model cache. Cmd-A GIỮ top-level (cell content sẽ bị refuse cross-cell từ
  đầu — §5.5g).
- **Gotchas phiên:** (1) **hitTest tie-break bắt buộc cho cell cùng hàng** —
  2 cell side-by-side có baseline TRÙNG (y-pick cũ greedy-so-kề giữ dòng
  TRƯỚC) → MỌI click vào cell phải resolve về cell trái; fix = so-with-best +
  tie-break theo x-span (dòng chứa x thắng khi baseline bằng) — table-free
  byte-identical, draw path không qua hitTest nên parity không chịu ảnh
  hưởng; gate 14 +checks trên cặp collision. (2) **bug caret junction
  apply_backspace merge (cũ, drive-by fix):** `at_run`/`at` tính từ runs của
  paragraph CHỨA CARET thay vì paragraph TRƯỚC → caret rơi sai điểm nối khi
  merge (test cell verbs bắt); fix = tính từ `prev_p` trước extend. (3)
  **clippy sau đổi signature:** `&mut Vec<Block>` → slice chỉ nơi body thật
  sự không cần Vec (remove/insert/drain giữ Vec — ptr_arg tự đúng); lifetime
  `'a` elide; `Table`/`TableCell` import phải xuống test module (top-level
  bị "unused" ở build non-test). (4) **zsh PIPESTATUS cắn lần 2** (T-A29
  lặp lại): `${PIPESTATUS[0]:-$?}` sau `gate | tail` trả rc của tail —
  celldoctor suýt báo rc=0; bắt exit TRƯỚC pipe. (5) **parity-view flake
  font-timing phía oracle (lần đầu thấy):** run 4 FAIL layout-fixture
  maxdy 15.93 ("break-point drift (wrap ceiling, font-metric)" — wrap point
  dịch giữa oracle/viewer) + edge-multilevel-list maxdy 464.60 (classified
  numbering ceiling, PASS) trong khi runs 1/2/3/5/6 đều 6/6 px-identical số
  chuẩn; cuts trên layout.ts comment-only (diff verified) + draw path không
  qua hitTest → code loại trừ; nghi ngờ font state của instance oracle
  ONLYOFFICE cũ còn nằm (`--remote-debugging-port=9222` PID 25215 từ phiên
  trước) — 2 chạy lại liên tiếp PASS số chuẩn; ghi nhận để version sau nếu
  tái hiện thì kill oracle cũ trước khi parity.
- **Còn cho slice #4 (T-A31, ceilings §5.5i/§10):** hyperlink/sectPr-một-đoạn
  + settings/theme + refuse-warn (OQ-L01-2) · Tab/Shift-Tab nav giữa cell ·
  tạo/xoá bảng/hàng/cột · merge/split cell · cross-cell selection ·
  tblPr/tcPr styling · arrows nhảy qua bảng · nested-table editing · word
  vắt run / kéo-theo-word sau dblclick · Cmd-A bao gồm cell content · hidden
  editable sink (real IME) · paste RTF/HTML.
- **Trạng thái:** confirmed — bằng chứng trong DD L01 §5.5 + báo cáo phiên.

---

## 2026-10-03 · T-A29: A4 slice #2 — IME composite tiếng Việt (đóng OQ-L01-3) + clipboard plain-text + Cmd-A

- **Bằng chứng (3 commit: 3ada9e8 design → 5a1a0e5 ts+gate15 → 9a2f24f ponytail
  cuts):** gate 15 positive exit 0 (first-draw 91–110 ms / parse 3.7–9.0 ms ·
  composition evidence 4 dòng, end `Xin chào` · clipboard `op=copy chars=8
  sys=fail` → `op=paste sys=stash` → `op=copy-all chars=1425` · selection span
  min −1 (Cmd-A) max 8 · save 2699 B / 2.0 ms · parse-back chứa marker +
  `paragraphT28` + `Xin chào` + `Xin Xin chàochào` · media byte-exact · reopen
  15 ms) + **negative ×4 exit 1** (garbage · doctor · seldoctor · **clipdoctor**
  — `'Xin chào'/8 != 'WRONG-CLIP'/999999` cắn trên evidence thật). verify FULL
  exit 0 **19 ▸ steps** ×2 (trước + sau cuts); gate 12 số KHÔNG đổi (16 dòng ·
  601.7 · 600.5 · 750,-250,220 · lhs 17.9 · RSS 243–281 MB ≤ 300); gate 14 93
  checks; **parity-view KHÔNG chạy — layout.ts 0 diff** (preedit vẽ ở overlay
  editor.ts; render.ts/viewer.ts cũng 0); cargo test 38 shell + 12 crate rc=0
  tuần tự (**Rust 0 đổi** — paste = chuỗi insert/enter có sẵn, cut = ranged
  backspace T-A28); clippy -D warnings sạch.
- **Thiết kế chốt (DD L01 §5.4):** composition = state machine 3 fn
  (start/update/end) mirror bảng DD 1:1 — `compositionEnd(data)` là core fn cả
  listener lẫn probe gọi; commit = 1 insert = 1 undo snapshot (semantics `apply`
  giữ nguyên); keydown guard `composing || isComposing` ĐẦU handler (IME sở hữu
  phím, non-IME đường cũ nguyên vẹn); preedit = text + underline tại caretXY,
  KHÔNG displacement inline (ceiling). Clipboard chốt **navigator.clipboard +
  session stash fallback, KHÔNG Tauri plugin** (dep + license gate không mua gì);
  paste `\n` = split qua enter cmd (N dòng = N undo units — ceiling gộp); Cmd-A
  anchor đầu / focus cuối paragraph block cuối (span −1). Đóng OQ-L01-3:
  pipeline per spec + probe E2E; real-IME engagement (hidden editable sink) =
  ceiling §10 — WebKit cần editable để giao marked text, pipeline sẵn sàng nhận.
- **Gotchas phiên:** (1) **zsh không có `PIPESTATUS`** (bash-only) —
  `${PIPESTATUS[0]:-$?}` sau `gate | tail` trả rc CỦA `tail` → suýt tuyên bố
  gate PASSED khi đang FAILED; phát hiện vì banner PASSED vắng trong output.
  Bài học: bắt exit code TRƯỚC pipe, kể cả khi "chỉ xem output". (2) **sai số
  học probe: sau shift-extend `caret` = FOCUS** (không phải anchor) —
  ArrowLeft ×4 từ 31 đưa về 19 (giữa "paragrap|h") thay vì 27; gate bắt qua
  assert `paragraphT28` biến mất khỏi copy-all → mổ log thấy paste cắm offset
  19; fix = ArrowRight ×4 từ focus 23 (DD §5.4c sửa theo run thật). (3)
  **WKWebView CompositionEvent constructing mang `data` đúng** — không cần
  fallback gọi handler trực tiếp (khác tiền lệ offsetX của MouseEvent T-A28);
  `navigator.clipboard.writeText/readText` bị chặn không user activation
  (`sys=fail`) — stash fallback giữ trọn round copy→paste (`src=stash`).
  (4) `readText` resolve chuỗi rỗng → rơi về stash (đúng ý — clipboard trống
  không xóa selection). (5) File rebrand UI của user vẫn dirty nguyên trạng
  (không đụng, không commit — scoped add từng file suốt phiên).
- **Còn cho slice A4 #3 (ceilings §10):** double/triple-click chọn word/para ·
  caret blink · selection/sửa trong bảng (kèm bỏ refuse §2 + tblPr/tcPr) ·
  arrows nhảy qua bảng · hidden editable sink (real IME) · paste format
  RTF/HTML · hyperlink/sectPr-một-đoạn + refuse-warn (OQ-L01-2).
- **Trạng thái:** confirmed — bằng chứng trong DD L01 §5.4(e) + báo cáo phiên.

---

## 2026-10-03 · T-A28: A4 slice #1 — selection kéo + arrows trên seam T-060 (Nano Light)

- **Bằng chứng (5 commit: dd1a908 design → 9a1f923 rust → 04a0d7b ts+gate15 →
  29244c8 C5 → 9ed3351 ponytail cuts):** gate 15 positive exit 0 (first-draw
  36.0 ms / parse 8.1 ms · selection evidence span 1→4 · type-replace → save
  2682 B / 1.6 ms · parse-back chứa `paragraphT28` · reopen 14.0 ms) +
  **negative ×3 exit 1** (garbage · doctor · **seldoctor** — assert
  `span == 0` cắn trên evidence thật, chứng minh gate đọc số liệu selection).
  verify FULL exit 0 **19 ▸ steps** ×2; gate 12 số KHÔNG đổi (16 dòng · 601.7
  · 600.5 · 750,-250,220 · lhs 17.9 · RSS tree 282 MB ≤ 300); gate 14
  90 → **93 checks**; **parity-view 6/6 exit 0** px-identical bảng §5.5 V01
  (layout.ts CÓ đụng — caretXY block-match, draw path không đổi); cargo test
  38 shell (+4 range/verbs/undo-sel) + 12 crate tuần tự rc=0; clippy -D
  warnings sạch.
- **Thiết kế chốt (DD L01 §5.3):** `SelRange{anchor,focus}` unordered, ordering
  `(para,run,offset)`; **`Caret.para` = BODY BLOCK index** (`Hit.block`) — fix
  pi≠block aliasing: `pi` đếm table = 1 block nên paragraph top-level SAU bảng
  có pi ≠ body index, mutation address `doc.body[para]` trúng sai block (walk
  layout tái hiện) — editor.ts consume `h.block`, caretXY match `line.block`,
  Rust không đổi (table-free: block == pi, byte-identical). Đụng cell → REFUSE
  selection (không clamp — mutation đã refuse trên cùng identity). Mutation
  range-aware: 4 verb mang `sel`, non-empty → `apply_delete_range` TRƯỚC verb
  (same-para cross-run giữ props từng run; cross-para pPr ĐẦU thắng; ảnh chết
  khi range phủ cuối text run; **range chứa Table refuse TOÀN BỘ** — chống
  data loss); snapshot undo `(doc, caret, sel)` dùng caret/sel CỦA CMD.
- **Gotchas phiên:** (1) caret sau redo = vị trí TRƯỚC undo (redo push capture
  pre-undo `self.caret` — hành vi T-A27 giữ nguyên) → probe đầu dispatch
  shift+ArrowRight chọn nhầm 4 ký tự tail; fix = shift+ArrowLeft ×4 chọn
  `" two"` + assert `paragraphT28`. (2) bug span emitSel: cùng-run trả
  `len − offset` thay vì khoảng cách — gate bắt qua số 14 vô lý. (3) WebKit
  không constructing `offsetX/Y` trên synthetic MouseEvent → probe gọi thẳng
  `mouseDownAt/mouseMoveAt/mouseUp` (phím vẫn dispatch KeyboardEvent thật qua
  handler). (4) **thay đổi lạ trong working tree KHÔNG của phiên** (logo
  `src/assets/logo.png` + icons + main.ts + 3 html — mtime 00:11–00:13, trước
  commit đầu của phiên): đã inspect diff (rebrand UI) + báo user, KHÔNG đụng,
  KHÔNG commit — mọi `git add` scoped từng file.
- **Còn cho slice A4 kế (#2 theo DD §4):** IME composite tiếng Việt
  (OQ-L01-3) · clipboard copy/cut/paste · select-all/double-click · blink ·
  parity matrix từng lớp = criterion đóng rung A4.
- **Trạng thái:** confirmed — bằng chứng trong DD L01 §5.3(h) + báo cáo phiên.

---

## 2026-10-03 · T-060: A4 pre-slice cleanup (C3+C1+C2 review #5) — refactor thuần, hành vi không đổi

- **4 commit**: `1298b02` C3 hit-seam · `e4d7a40` C1 render.ts · `5e874d0` C2 Rust shared ·
  `b90913a` ponytail cuts. Verify FULL exit 0 **19 ▸ steps sau MỖI commit** (4 lần); gate 12
  số layout KHÔNG đổi mọi lần (16 dòng · colw 601.7 · maxw 600.5 · metrics 750,-250,220 ·
  lhs 17.9). **parity-view 6/6 px-identical ×3 lần chạy** (sau C3 ×2 + cuối sau C2) — maxdy
  0.64/4.07/0.00/6.47/2.73/2.73, maxdx 47.00 đúng bảng DD V01 §5.5. Cargo test 34 shell
  (+1 mới) + 12 crate. Gate 14 42 → **90 checks**.
- **C3 (hit seam mang identity bảng):** `TextLine`/`Hit` +`{block, cell:[row,col]|null}`
  ADDITIVE vào layout.ts (block = index body top-level; line trong cell nhận block của
  TABLE + cell identity — đệ quy nested table kế thừa table ngoài). Thật ra demo được bug
  tiềm ẩn: corpus [P0, T(cell 2 para), P-after] → cell para 2 có pi=3 ≠ pi P-after=2, nhưng
  cell para ĐẦU pi=2 == pi P-after — hit 2 chỗ này trước C3 KHÔNG PHÂN BIỆT ĐƯỢC bằng
  {para,run,offset}; giờ `block`+`cell` tách (gate 14 test mới assert đúng cặp collision
  này). `caretXY`+`itemLen` chuyển từ editor.ts vào layout.ts cạnh hitTest (dùng
  `itemCharLen` có sẵn — xóa bản đúp); round-trip assert `caretXY(hitTest(x,y)) ≈ (x,y)`
  tại MỌI char boundary (fake measurer 10px/char exact + wrapped line) — mid-char floor
  về boundary theo rule (không half-char centering) nên chỉ assert tại boundary là đủ.
  Rust `Caret` +`cell: Option<[usize;2]>` `#[serde(default)]` (payload cũ vẫn parse —
  additive, không đổi tên/casing/bỏ field nào); `EditorSession::apply` refuse mọi verb
  mang caret `cell≠null` TRƯỚC khi resolve `para`; `paragraph_at` giữ arm refuse
  body[para]=Table (chặn stale caret trỏ thẳng vào block table) nhưng bỏ đoán-comment dài.
- **C1 (gộp C6) render.ts:** measurer/media-load (tag vào log prefix)/columnWidth/draw+
  overlay/docHeight/toEvidence/mediaInk/resizeRelayout — 1 nhà; viewer.ts/editor.ts còn
  phần thật sự riêng. **Evidence JSON byte-identical PROVEN bằng diff trực tiếp**: chạy
  gate 12/15 trước C1 giữ log, chạy lại sau C1, `diff` evidence JSON từng dòng = identical
  (cả `viewer evidence:` lẫn `nano light evidence:` các cửa sổ). toEvidence giữ key order
  bằng literal — JSON.stringify theo insertion order là contract ngầm, không reorder.
  **Python parser 3 gate KHÔNG gộp** (quyết định ponytail): soi 3 heredoc — không có
  đoạn nào trùng nguyên văn; mỗi parser parse prefix + asserts contract khác nhau
  (gate 12 = layout số + media + hit-test; gate 15 = edit chain + parse-back marker +
  reopen evidence; parity = 3-way compare với oracle) — gộp tạo indirection cho code
  không trùng thật, rủi ro log-contract cao hơn lợi.
- **C2 (+C5-mini) Rust shared:** `render_payload(registry, owner, path, tag)` ở viewer.rs
  (read→parse→resolve→metrics→register — tag vào error string để negative gates còn cắn
  đúng msg); `open_surface_window(app, label, html, title, tag)` focused(false);
  `atomic_replace(path, bytes, tmp_tag, err_tag)` — **1 caller thật** (save_session; đường
  engine save KHÔNG gộp: x2t viết tmp trên đĩa chứ không phải bytes-from-memory, và
  rename-fail của nó KHÔNG dọn tmp — gộp sẽ đổi hành vi); `EditorSessions::with_session`
  chọn **closure thay vì trả MutexGuard** (guard-return buộc caller get_mut lại → unwrap
  hoặc match khuôn thứ hai — closure làm poison/no-session error 1 nhà thật sự, 4 command
  còn 1 dòng). Log line `nano light edit:` giữ byte-identical (undo=undo_len qua tuple).
- **Ponytail-review toàn diff (mandate 03.10):** 3 findings shrink áp dụng — render.ts bỏ
  `export` MAX_CANVAS_H/docHeight (0 caller ngoài module); viewer.rs `families()` bỏ
  `pub(crate)` (editor không còn gọi trực tiếp sau render_payload) + comment stale. Verify
  FULL lại sau cuts exit 0.
- **Net LOC code (không docs): +662/−476 = +186**, phân bổ: render.ts +182 vs
  viewer+editor −242 → **phần gộp C1 thu ÂM −60**; test mới +~96 (gate 14 +46: table
  identity 9 checks + round-trip 39; Rust in_cell_carets_refused ~50); còn lại ~+90 =
  C3 plumbing data qua 3 tầng (TS fields + walk params + Rust serde/refuse) + doc comments
  — tính năng dữ liệu thật thay invariant-by-comment, không phải phần trương vô ích.
- **Gotchas:** (1) pi-count: cell para THỨ NHẤT đụng pi block sau table (không phải para
  thứ 2 — test đầu tiên viết nhầm index dòng, gate 14 bắt ngay: got 3 want 2); (2) perl
  multiline edit tạo BẢN SAO caretXY thứ 2 trong editor.ts thay vì xóa (Edit chèn nhầm
  hướng) — phải xóa cả 2 bản vì hàm đã chuyển vào layout.ts; (3) `parse_ms` của editor
  giờ đo đến hết register (payload) thay vì hết session-insert — lệch ns, format 1 chữ
  số thập phân không đổi; (4) evidence JSON key order = insertion order của object
  literal — "byte-identical" phụ thuộc thứ tự khai báo field, ghi vào comment load-bearing.
- **Còn cho slice A4 kế:** selection kéo + arrows (dùng `block`/`cell` để disambiguate
  click cell khi mở selection); C5-full (casing unify NanoLightDoc snake↔camel, type 1
  nhà) khi slice chức năng đụng contract; C4 surface-table chờ surface thứ 4.

---

## 2026-10-03 · Review kiến trúc #5 (T-055) — post-T-A27, pre-slice-A4 (user kéo sớm)

- **Phương pháp:** improve-codebase-architecture + vocabulary codebase-design (user yêu cầu skill cụ thể lần này;
  ponytail-audit không chạy — T-060 sẽ có ponytail-review pre-commit). Explore subagent đi bộ vùng nóng 2 ngày
  (T-A27/T-V7/T-059) → **10 friction kèm file:line → 7 candidates** → HTML report `$TMPDIR/architecture-review-
  20261003-223603.html` (không land repo — tiền lệ các review trước).
- **User chốt C3/C1/C2 → T-060** (một task tuần tự, hành vi không đổi): C3 hit-seam (`Hit` + cell-identity +
  caretXY vào layout.ts + round-trip assert gate 14 — click cell hiện trả para chỉ block SAU table, invariant chỉ
  nằm comment 2 phía; caretXY pure nhưng 0 test vì kẹt editor.ts import __TAURI__) · C1 render.ts (editor.ts copy
  ~37% viewer.ts verbatim — slice selection sẽ mọc bản sao render thứ 3; gộp C6: evidence emit ×2 + parse python
  inline ×3, 0 schema) · C2 Rust shared (`render_payload`/`open_surface_window`/`atomic_replace` — nano_light_*
  clone viewer_* 4 tầng: request-slot + window body + open pipeline dòng-cho-dòng + tmp/rename; kèm C5-mini
  `EditorSessions::session_mut`).
- **Defer có trigger:** C4 lib.rs surface-table (5 chỗ sửa/surface + thang probe 5 biến) — chờ surface thứ 4
  (YAGNI) · C5 phần còn lại (casing unify + type 1 nhà + gate assert shape) — hoàn khi A4 slice chức năng đụng
  contract tiếp · C7 smoke-lib hằng đơn-consumer — chỉ khi đụng file vì lý do khác (frozen block rủi ro > lợi).
- **Bài học:** `NanoLightDoc` camelCase vs `QuickView` snake_case tồn tại nguyên 1 NGÀY trước khi review bắt —
  contract song hành Rust↔TS phải chốt 1 casing ngay slice đầu (pain này T-V2 từng ghi: viewer.ts tự khai báo tay
  lại model Rust). T-060 không đụng casing (refactor thuần) — C5-full là nhà của việc đó.
- **Trạng thái:** T-060 land TODO (Track A, ← T-055 #5); T-055 reset ⬜; handoff `docs/handoff/t060.md`.

## 2026-10-03 · T-A27: A4 mở màn — editor core "Nano Light" vòng edit đầu (không sdkjs/x2t)

- **Bối cảnh:** mở màn rung A4 theo QĐ9/DD L01 (`docs/design/editor-a4/DD.md`,
  design-first commit 8b5356e TRƯỚC code 6e63399). Vòng 1: caret → gõ → xoá →
  enter → undo/redo → dirty → **save thẳng `write_docx`** — KHÔNG Editor.bin,
  KHÔNG x2t trên đường Nano Light; mặc định mở file vẫn engine (D1), Nano Light
  vào qua nút start page + env `NANO_OFFICE_NANO_LIGHT` (pattern V5). Consume
  `src/viewer/layout.ts` + `hitTest` NGUYÊN VẸN — **không đụng layout.ts/viewer.ts
  (0 diff)**: parity-view không cần chạy lại; vite tự gom layout thành chunk dùng
  chung viewer/editor (by-construction không fork được).
- **Kiến trúc land:** `src-tauri/src/editor.rs` — `EditorSessions` map
  window-label → `EditorSession` (giữ `Document` CHƯA-resolve + bytes gốc
  `media_src` + undo/redo snapshot-clone kèm caret, cap 100 + `saved` clone cho
  dirty = `doc != saved`); commands `nano_light_view/open/edit/save/reopen`
  song song delivery (tiền lệ C3), session drop cùng hook `WindowEvent::Destroyed`
  của T-059. Toạ độ mutation = hệ hitTest T-V7 `{para, run, offset}` — **offset
  là utf-16 code units** (JS string indexing; surrogate pair = 2 units — utf16↔byte
  map 2 chiều, backspace bớt đúng số units của ký tự bị xoá). Frontend
  `src/editor.ts` + `editor.html` (entry vite thứ 3 ở ROOT — gotcha V01):
  re-render sau mỗi edit từ RenderDoc re-resolve, caret forward-map thuần
  (walk item `ri`/`charStart` + measurer — mirror hitTest, KHÔNG sửa layout.ts).
- **Gate 15** `smoke-nano-light.sh` (pattern smoke-lib, thường trụ verify.sh):
  boot env + probe `NANO_OFFICE_NL_PROBE` chạy CÙNG `edit()` handler keydown dùng
  (không keystroke, rule 6b) → chuỗi insert/backspace×3/enter/undo/redo → save →
  **parse lại bằng crate CLI chứa marker** + **media part sha256 byte-exact** vs
  snapshot trước save + **reopen window 2 evidence chứa marker** ("mở lại render
  đúng nội dung đã edit"). Positive exit 0 ×3 (27–39 ms first-draw, save 1.6–2.3
  ms/2681 B); negative ×2 exit 1: garbage docx → `nano light failed:` cắn nhanh
  (pattern gate 13) + **doctor** (parse PRE-SAVE snapshot không marker → assert
  parse-back cắn — chứng minh assert đọc file đã save, không chỉ vắng mặt).
  Verify FULL exit 0 **19 ▸ steps** ×2 (trước + sau ponytail cuts); gate 12 số
  layout không đổi (16 dòng · 601.7 · lhs 17.9); cargo test 33 shell (+5 editor)
  + 12 crate.
- **Parity matrix ngoài-`Document`** (DD L01 §5.1, trung thực không nâng model
  mù): GIỮ — paragraph/run props + bảng grid + numbering + ảnh byte-exact +
  hdr/ftr + pgSz cuối · MẤT khi save Nano Light — **hyperlink (mất cả text —
  parser skip subtree theo policy S1, data loss thật phải biết)**, sectPr giữa
  tài liệu, pgMar + con sectPr khác, settings.xml, theme fonts, docProps,
  tracked changes/footnotes/fields. Mỗi mục có criterion nâng; edit bảng refuse
  (pi cell đụng độ block kế — ceiling T-V7).
- **Gotchas phiên:** (1) TS narrowing — `model` gán trong closure `render()`
  không tính cho main flow (`Property 'media' does not exist on type 'never'`)
  → `render()` trả model, gán ở call site; (2) `Document` không impl Default —
  test builder `document(body)` riêng; (3) test session semantics đầu tiên tự
  replay logic command (test-of-the-test) → refactor logic vào
  `EditorSession::apply()` (command thành vỏ mỏng) rồi test đường thật;
  (4) ponytail cuts bắt được: `NanoLightSaved{bytes,saveMs}` struct chết
  (frontend bỏ qua kết quả — số đã trong log Rust), `undoDepth` chết, 5 hằng
  `SMOKE_RE_NL_*` không gate bash nào grep.
- **Kế tiếp (slice A4 sau):** selection kéo + arrows (ceiling §10 L01) · IME
  tiếng Việt composite (OQ-L01-3) · hyperlink/sectPr-giữa vào model hoặc
  refuse-mở (OQ-L01-2) · T-V6 pgMar/spacing khi parity đòi · parity matrix từng
  lớp = criterion đóng rung A4.
- **Trạng thái:** confirmed — bằng chứng trong DD L01 §5.2 + báo cáo phiên.

---

## 2026-10-03 · T-059 + T-V7 — registry keying + layout core/hit-test (2 điều kiện trước A4)

- **T-059 (review #4 C3, bug shell chạy được hôm nay) — CONFIRMED + FIXED (4a12d00):**
  `FileRegistry::latest()` = max id được 4 consumer dùng (review #4 liệt kê 3 — tự tìm thấy
  consumer thứ 4 khi mổ code: `LocalFileSetOpenChangesCount` ở `bridge/mod.rs:83`, ghi
  open-changes-count cho save truncation — cùng lớp race) trong khi `viewer_open` đăng ký
  file quick-view vào CÙNG registry → Open A rồi Quick view B = delivery nhận nhầm B /
  save A hit workdir id B. **Fix:** xoá `latest()`; `register(owner=window-label)` +
  `unregister_owner` (hook `WindowEvent::Destroyed` — registry hết chỉ phình); 4 consumer
  keying theo fileId tường minh: `get_document_for_delivery(file_id)` + `flow_file_id`
  (payload.fileId) cho `LocalFileSaveChanges`/`LocalFileSave` + match bridge cho
  SetOpenChangesCount. JS: `open_document` trả `fileId` → `main.ts` stamp
  `window.__nanooffice_file_id__` trong `mountEditor` (choke point duy nhất) → relay
  `mainFrameInvoke` đính `payload.fileId` + `delivery.js` truyền `{fileId}`. Thiếu/unknown
  id = lỗi TO (fail loudly). Test race ×2: 2 register + consume chéo đúng file (A < B —
  B là "latest" giả định) + deltas A land đúng workdir A; counterfactual missing/unknown.
- **Bài học T-059 (bắt bằng gate 9):** Tauri command `#[derive(Serialize)]` KHÔNG tự
  camelCase — Rust `file_id` serialize thành `file_id`, TS đọc `doc.fileId` = `undefined`
  → delivery lỗi "no fileId". Fix = `#[serde(rename_all = "camelCase")]` trên
  `OpenedDocument`. Triệu chứng lần đầu: gate 9 timeout 240s với đúng 1 dòng ERROR
  `document delivery failed: delivery: no fileId` — mổ log app thật
  (`/tmp/nanooffice-smoke-*`) thay vì log tail của smoke script.
- **T-V7 (review #4 C1, điều kiện mở T-A27) — CONFIRMED:** layout tách thành module thuần
  `src/viewer/layout.ts` (không DOM/canvas; measurer tiêm vào; `LineModel` + `hitTest`);
  wrap/tab/OS-2/media port VERBATIM (pi counting, row[0], +4 giữ nguyên); viewer.ts thành
  consumer vẽ + evidence (shape `viewer evidence:` byte-identical). **Parity KHÔNG fork:**
  parity-view 6/6 PASS ×2, từng px giống bảng DD V01 §5.5. **Hit-test:** y = baseline gần
  nhất (biên midpoint), x = binary-search biên ký tự bằng chính measurer forward; offset
  run-relative (`ri`+`charStart` thêm vào tokenize, thứ tự token không đổi); tab = 1 ký tự
  (nửa rộng); dòng rỗng (br kép) → placeholder đầu paragraph (A4 refine); x trong advance
  ký tự i → caret trước ký tự i (không half-char centering — ceiling ghi DD §5.7, A4 có
  thể lớp centering trên cùng model). 2 tầng test: **gate 14** `test-layout.sh` (tsc →
  node, 42 checks tọa độ tay, measurer fake 10px/char) + probe E2E gate 12
  (`NANO_OFFICE_QV_HITTEST=1` → init script → round-trip mọi dòng → `viewer hit-test:
  16/16 lines ok`; negative doctor `15/16` cắn exit 1). Resize re-layout: listener → rAF
  → render() lại (cùng đường code first-draw; không gate riêng).
- **Bài học T-V7 (tsc 6.0):** truyền file trực tiếp cho `tsc` khi repo có tsconfig.json
  → error TS5112, phải `--ignoreConfig`; không muốn dep `@types/node` cho gate → test
  tự viết assert/deepEqual 15 dòng (không import node).
- **Trạng thái:** confirmed — verify.sh FULL exit 0 ×2 (T-059: 17 ▸ steps, gates
  9a–d/10/11/12/13 GREEN; T-V7: + gate 14, trên cây sắp commit). `viewer hit-test:` là
  log contract mới (khối đóng băng smoke-lib, emitting end viewer.ts — cùng commit).
  ⚠ Ghi cho T-A27: gate 12 hiện boot kèm `NANO_OFFICE_QV_HITTEST=1` — probe là gate food,
  không đụng product default.

---

## 2026-10-03 · Review kiến trúc #4 (T-055) — post-A3, focus cản A4 "Nano Light"

- **Phương pháp:** bộ đôi improve-codebase-architecture + ponytail-audit (codebase-design từ
  vựng), hot-spot `git b8e2490..HEAD` (35 commits, +4967/−424 — chuỗi A3: crate render-side +
  resolve, viewer + command layer, gates 12/13, Tailwind token-slot); 4 claim chịu lực kiểm
  chứng first-hand: (1) `delivery.js:197` invoke `get_document_for_delivery` KHÔNG tham số +
  `latest()` = max id (`protocol/mod.rs:64-69`) được consume ở 3 nơi engine flow
  (`lib.rs:137` delivery, `save.rs:79` append_changes, `save.rs:184` save_document) trong khi
  `viewer_open` đăng ký file vào CÙNG registry (`viewer.rs:110`, V3) · (2) layout interleaved
  trong draw (`viewer.ts:147-351`), `main()` one-shot, line model vứt sau draw, không hit-test
  ngược, không resize listener · (3) `viewer_open` parse→resolve→DROP `Document`
  (`viewer.rs:102-105`) — `RenderDoc` đã flatten thứ bậc §17.7.2 (resolve.rs), bản
  ghi-ngược-được là `Document` + `write_docx` (round-trip bất biến + media byte-exact T-A26) ·
  (4) gate 12/13 = consumer mỏng smoke-lib, fixtures share docxgen_common — ponytail lean.
- **Trả lời 3 câu hỏi trọng tâm A4 (handoff #4):** model/resolve **CHIA** được viewer↔editor
  (crate pub, parity bảo vệ) · layout **CHƯA** — không phải module (C1) · command layer
  **CHƯA** có chỗ cho edit+undo — model không sống Rust-side, `QuickViewRequest` single-slot
  có race tự nhận trong comment (`viewer.rs:20-22`) (C2) · save khi không có Editor.bin =
  **`write_docx(&Document)` đã proven** nhưng session phải giữ bản CHƯA-resolve + danh sách
  ngoài-model (sectPr giữa tài liệu/hyperlink/settings.xml/theme fonts/tblPr-tcPr) phải thành
  parity matrix (C2).
- **C3 (observed — bug shell chạy được hôm nay):** registry có 2 writer + keying `latest()` —
  Open A (boot engine hàng trăm ms) rồi Quick view B → delivery nhận nhầm B (wrong-open);
  save A sau đó hit workdir của id B ("Editor.bin missing" fail-closed, hoặc deltas ghi nhầm
  workdir — không mất dữ liệu user hôm nay vì workdir keyed id monotonic per-run + delivery
  `remove_dir_all` trước convert, nhưng sai kết cấu và A4 nhân số flow: session Nano Light +
  advanced-mode + quick view đồng thời). Amendment V3 trong DD ghi "luồng delivery không hề
  đụng tới" — đúng ở mức gọi hàm, sai ở nhất quán dữ liệu (`latest()` = shared state).
- **C1 (cản A4):** layout là tài sản parity ε=2.0px (OQ-V01-1) nhưng không tồn tại như module —
  A4 fork copy `wrapParagraph` = 2 layout engine trôi từ ngày đầu, parity không bảo vệ bản
  editor; cần LineModel + hitTest trước khi T-A27 code (regression: parity-view 6/6 + gates
  12/13 không đổi từng px).
- **C2 (cản A4):** không có nhà cho session/undo/save-đường-crate — constraint nạp T-A27
  (pattern T-A25 nạp deps review #3): DocSession window-scoped giữ `Document` + undo + dirty,
  mutation commands ở command layer (tiền lệ C3 song song delivery), save = `write_docx` +
  atomic replace; `QuickViewRequest` single-slot bị session map thay tự nhiên.
- **Ponytail:** lean — không có delete:/stdlib:/native:/yagni:/shrink đáng đất trong diff A3
  (mỗi commit đã qua ponytail-review theo mandate 03.10); quan sát perf KHÔNG thành task
  (ngoài scope ponytail, chưa pain đo được): `viewer.rs::families()` dedup O(n²) (doc nhỏ),
  filesrv media route re-read toàn zip mỗi request ảnh, evidence/ink scans chạy cả production
  quick view (là thức ăn của gates — log contract đóng băng; C1 tách evidence theo draw tự
  nhiên).
- **Land:** T-059 (C3, đầu PH-1) + T-V7 (C1, điều kiện mở T-A27) + deps C2 vào dòng T-A27 +
  T-055 đặt lại ⬜ (lần kế #5). Report HTML tạm
  `$TMPDIR/architecture-review-20261003-t055-r4.html` (không vào repo). Không mâu thuẫn
  QĐ1–QĐ9; không re-đề xuất T-A14..T-056/T-V1..T-V6.
- **Trạng thái:** confirmed (verify.sh chạy cuối phiên — kết quả trong báo cáo phiên).

---

## 2026-10-03 · T-V5: A3/V5 — wire UX quick-view + ĐÓNG RUNG A3

- **Bối cảnh:** slice cuối rung A3 — quick view ra khỏi dev-env thành UX thật: chọn file
  từ UI mở quick view (chỉ thị user 03.10 — **OQ-V01-3 CHỐT = toggle**, "mặc định cho
  docx" thuộc A4 Nano Light), scroll cơ bản, không tự thêm tính năng. Style mới qua
  token-slot Tailwind T-058 (utility classes trong markup, không đụng canvas scope #app —
  bẫy T-058). Chi tiết: DD V01 §4 V5 + §5.6.
- **Wire UX (`viewer_view`):** nút Quick view cạnh Open trên start page (`main.ts`,
  dùng lại `CONTROL_SKIN` — 0 CSS mới) → command `viewer_view` (`viewer.rs`): validate
  file tồn tại (lỗi trả về start page thay vì mở window chỉ để báo lỗi) → stash path vào
  state `QuickViewRequest` (Mutex, take-once) → mở window `quickview-<n>`
  (WebviewWindowBuilder, focused(false) rule 6b, KHÔNG inject shim/bridge/delivery);
  `viewer_open` consume stash, fallback env `NANO_OFFICE_QUICK_VIEW` (dev + gate 12 giữ
  nguyên). Disk path không vào URL — stash Rust-side (nguyên tắc filesrv). Đóng window =
  Cmd-W/close native — không thêm phím tắt riêng (ponytail: platform feature).
- **Scroll cơ bản:** canvas = trang — cao bằng nội dung đã layout (min = viewport; cap
  **8192px logic = WebKit 16384 device px @dpr2** — cao hơn clip, pagination là việc
  post-A3, ceiling ponytail). **Bẫy context:** resize canvas CLEAR canvas + reset context
  state (kể cả transform `scale(dpr)`) → **2-pass draw**: pass 1 measure (`contentBottom`
  từ baselines+lhs và media rects) → resize → **re-scale ctx + set lại fillStyle** →
  pass 2 redraw CÙNG input → cùng tọa độ; evidence/`viewer evidence:` lấy từ pass 2.
  body `overflow-auto` (viewer.html).
- **Gate 13** `smoke-quick-view-ui.sh` (thường trụ verify.sh, consumer smoke-lib —
  `SMOKE_RE_QV_VIEW` vào khối đóng băng): boot app THƯỜNG (không env quick-view) với
  driver **`NANO_OFFICE_UX_PROBE`** (pattern `NANO_OFFICE_SAVE_PROBE` — init JS điền
  input + click nút THẬT `button.click()`, không keystroke, không focus) → assert chuỗi
  `viewer view:` (command layer = UI wiring) → `viewer first-draw` < QUICK_VIEW_MAX_MS →
  `viewer layout:`, cấm `viewer failed:`. Coordinate/media asserts không nhân bản — vẫn
  ở gate 12; gate 13 sở hữu WIRING. Log `viewer first-draw … canvas <H>px` thêm đuôi
  (ADDITIVE — regex gate 12/13 không neo cuối dòng).
- **Bằng chứng:** gate 13 positive exit 0 (view → first-draw **30.0 ms** → layout 16
  dòng lhs 17.9 inked 27392 — số không đổi so T-V3) + negative exit 1 (.docx rác: chain
  chạy hết tới parse, `viewer failed:` cắn nhanh, không timeout) + gate 12 exit 0
  (first-draw 89 ms, layout giống hệt, RSS tree **265 MB** ≤ 300) + **parity-view exit 0
  — 6/6 PASS từng px giống bảng DD §5.5** (2-pass draw không lệch render) + scroll E2E
  doc 60 paragraph qua UI probe: `canvas 2232px` > viewport 700, first-draw 23.0 ms.
- **Đóng rung A3 (ceiling ghi thẳng):** đích RAM <150 MB là criterion cho **RELEASE
  build** (OQ-V01-2/DD V01 §10) — T-057 (release boot stall, PH-4) chặn đo release; số
  debug = tripwire gate 12 (300 MB, đo 265 MB). Không fake số RELEASE. Vision QĐ9 A3 ✅;
  ▶ = T-055 #4 (review kiến trúc post-A3) rồi seed **T-A27** (A4 "Nano Light").
- **Trạng thái:** confirmed — verify.sh FULL exit 0 (17 ▸ steps, gates 9a–d/10/11/12/13
  GREEN) trên cây sắp commit; `viewer view:` là log contract mới (smoke-lib khối đóng
  băng, emitting end `viewer.rs::viewer_view` — đổi phải cùng commit).

---

## 2026-10-03 · T-058: Tailwind v4 cho own-UI — 3 bẫy pixel-level (refactor style-infra, visual không đổi)

- **Bối cảnh:** T-058 migrate start page + viewer chrome sang Tailwind v4 CSS-first (`@tailwindcss/vite`,
  KHÔNG config file) trước T-V5; ràng buộc = visual pixel-identical. Bằng chứng: screenshot window
  trước/sau (boot nền rule 6b; `screencapture -l<CGWindowID>` chụp được cả window off-Space) + diff
  RGBA bằng script Swift CoreGraphics (/tmp tool, không dep repo).
- **Bẫy 1 — preflight phải loại:** Tailwind v4 default reset toàn cục sẽ đổi 2 thứ UI đang dựa vào:
  (a) body margin 8px UA (styles.css cũ không reset — page hiện có khung 8px), (b) box-sizing —
  preflight border-box làm `input{flex:1}` hẹp ~padding so với content-box hiện tại. Import
  `theme.css` + `utilities.css` có layer, KHÔNG import preflight.
- **Bẫy 2 — canvas `fillText` theo font-smoothing của document (bẫy lớn, gate pixel bắt được):**
  dùng chung styles.css cho 2 surface làm block `:root` typography (`-webkit-font-smoothing:
  antialiased`, `text-rendering`, …) rơi vào viewer.html — chrome inline cũ của viewer KHÔNG có
  chúng. Layout log GIỐNG HỆT từng số (16 dòng, lhs 17.9, baselines, inked 27392) nhưng screenshot
  lệch 100 310 px = contour anti-aliasing từng glyph, không phải dịch vị trí. Fix: scope typography
  vào `#app` (start page) thay `:root`; viewer về UA default → **0/2 800 000 px**. Bài học: CSS
  "vô hại" trên DOM thường đổi raster của canvas — mọi style dùng chung phải scope khỏi surface
  canvas.
- **Bẫy 3 — Tailwind auto-detect crawl cả `src/shim/*.js`:** sinh candidate rác (`m-1391`,
  `text-slate-900`, …) — CSS 20.74 kB. Clamp `@import … source(none)` + `@source` đúng 3 file
  own-UI (index.html, viewer.html, main.ts) → 5.33 kB, deterministic.
- **Ghi chú cấu trúc:** `@theme static` giữ slot chưa có ai dùng (`--color-accent`) greppable trong
  dist (contract A5); dark mode = 1 token flip trong `@layer base` — layer order khai
  `theme, base, utilities` nên override `:root` beat emission `@layer theme`, utility tham chiếu
  `var(--slot)` tự theo; titlebar text AA lệch giữa các lần chụp = window đặt khác vị trí/display
  (OS render) — so sánh vùng content (y ≥ 104 @2x); `font-mono` (ui-monospace đứng đầu) render
  cùng mặt chữ với stack cũ `ui-monospace, monospace`.
- **Trạng thái:** confirmed — verify FULL exit 0 (gates 9a-d/10/11/12 GREEN, gate 12 cùng số
  layout), pnpm build exit 0 ×2 entry, 8 token slot + 46 utility class grep dist, CSS byte-identical
  sau ponytail-review cuts.

---

## 2026-10-03 · T-V4: A3/V4 — parity render viewer ↔ sdkjs qua oracle CDP (cursor-probe walk)

- **Bối cảnh:** slice V4 viewer (A3) — chứng minh viewer render **parity** với engine sdkjs
  trên corpus. Oracle = ONLYOFFICE.app thật, CDP `:9222` (diagnostics 22.09/27.09), boot nền
  `open -g` (rule 6b) + `open -g <file>` + quit AppleEvent/TERM — không keystroke. Clean-room:
  quan sát runtime thuần (Builder API + selection bounds), KHÔNG đọc source render sdkjs.
  Chi tiết metric + bảng: DD V01 §5.5; kỹ thuật: reference.map § Viewer oracle CDP.
- **Kỹ thuật oracle (đã validate từng bước trên app thật):** (1) `callCommand` (Document
  Builder) chạy code trong editor frame — sandbox KHÔNG có `Asc.editor` nhưng CÓ global
  `editor` nội bộ; kết quả trả về qua callback VỊ TRÍ 3 (`callCommand(fn, a, b)` — data đến
  qua `b`, đo được 03.10); (2) **collapsed-caret probe**: `GetRange(p,p).Select()` +
  `editor.asc_GetSelectionBounds()` trả điểm caret (x, y đỉnh line box) — bisect y-flip =
  ranh giới dòng; (3) **text từng dòng = selection**: `GetRange(s,e).Select()` +
  `asc_GetSelectedText()`; (4) extent ảnh từ Builder `ToJSON` (`extent.cx/cy` EMU);
  (5) `zoom100()` pin đơn vị CSS px = viewer space.
- **3 bẫy lớn (confirmed, mỗi cái đều làm split sai trước khi fix):** (a) **positions ≠
  GetText index** — positions đếm content element ẩn (engine re-split runs không đều: đo
  select(0,3)="the" shift-0 nhưng select(105,108)="the" shift-1 trên CÙNG paragraph; scan
  GetText: 0 ký tự ẩn) → mọi phép suy position↔char đều sai — text PHẢI đọc qua selection;
  (b) **bounds cache cập nhật ASYNC theo draw** — đọc cùng tick với select trả giá trị
  position TRƯỚC (triệu chứng: "line break" giữa mid-word đúng 1 ký tự lệch, flaky giữa
  các run) → verify-triple (caret s on-line, lo off-line, lo−1 on-line) + re-bisect với
  baseline fresh, 4 lần rồi fail loudly (không bao giờ phát split sai im lặng); (c) **sai
  window**: desktop app mở MỖI file MỘT WINDOW, title chung "ONLYOFFICE Documents" — target
  phải match `title=<file>.docx` trong URL boot page, và `osascript quit` BỎ QUA im lặng
  trên app không bao giờ được activate → quit phải verify port đóng + fallback
  `kill -TERM` pid (lifecycle fresh mỗi run).
- **Metric chốt (OQ-V01-1):** M0 text paragraph sequence = HARD; M1 số dòng + M2 text dòng
  — lệch = wrap ceiling (phân loại); M3 spacing TRONG paragraph (j>0) ±**ε=2.0px** HARD —
  nền đo được max 0.64px (16 dòng uniform); drift ranh giới paragraph (j=0) = spacing
  ceiling (w:spacing không trong model), numbering dx/dy = numbering ceiling (mark + indent
  level); M4 extent ảnh EMU = HARD exact; offset tuyệt đối (margin 1in+16 vs pgMar + page
  placement giữa cửa sổ: y+17..+36, x−245..−330) chỉ báo — shift-corrected + re-anchor sau
  bảng/ảnh.
- **Bảng parity (6 file, ×2 deterministic giống hệt từng px):** layout 4/4 paras 16/16
  lines maxdy **0.64** dx 0.00 imgs 2/2 · full 8/8 8/8 4.07* 47.00* 1/1 · media 2/2 2/2
  0.00 0.00 1/1 · multilevel 5/5 5/5 6.47* 0.00 · style-override 3/3 3/3 2.73* 0.00 ·
  engine-save 5/5 3/7† 2.73* 0.00 — **PASS 6/6** (* = ranh giới paragraph/numbering —
  phân loại; † 2 para viewer wrap 2 dòng vs oracle 1 — wrap ceiling, M0 identical).
- **Negative (tiêm giả → cắn):** `PARITY_VIEW_NEGATIVE=1` trên layout fixture — +3px
  baseline + " XQ" text + EMU lệch 1px → **cả 3 cắn (M0 text / M3 baseline / M4 EMU) →
  exit 1**. Bài học đếm bite: marker "XQ" bị `[:50]` cắt trong message → đếm qua M0
  (concat luôn lộ) + M3 đúng target line + M4; injection phải mutate cả `vparas[].text`
  (M0 đọc từ đó); dòng inject phải tránh anchor + re-anchor line (dy bị skip).
- **Verify:** `parity-view.sh` positive ×2 exit 0 + negative exit 1 (3/3 bites) ·
  `parity-a2.sh` exit 0 (9 file, 0 hard-fail) · verify.sh FULL exit 0 (18 gates — gate 12
  không thoái hóa, `viewer evidence:` ADDITIVE không đụng contract) · `pnpm build` exit 0
  (viewer.ts evidence + align `al` field).
- **Trạng thái:** T-V4 ✅ — OQ-V01-1 ĐÓNG. ▶ = T-058 (Tailwind, đúng trigger trước T-V5);
  ceiling lớn lộ ra → seed **T-V6** (model mở w:spacing + pgMar + render numbering mark).

---

## 2026-10-03 · T-V3: A3/V3 — media bytes qua route filesrv an toàn + render ảnh theo wp:extent (EMU→px)

- **Bối cảnh:** slice V3 viewer (A3) — ảnh thật hiển thị trên canvas quick view. Clean-room:
  quy ước EMU verify qua nguồn mở (Apache POI Units.java, Apache-2.0 — `EMU_PER_INCH=914400`,
  `EMU_PER_PIXEL=9525` → px = emu/9525 @96 dpi; spec-ref ECMA-376 §20.4.2.7 `wp:extent`);
  không đọc upstream render code. Chi tiết design + số đo: DD V01 §5.4; evidence route:
  reference.map § Viewer media route.
- **Route `/media/<fileId>/<part>` (`filesrv.rs`):** key bằng fileId của FileRegistry
  (`viewer_open` đăng ký file — delivery không đụng, DD §3.1 amendment); part lookup
  **exact-name trong zip entry table** qua `try_part_bytes` (re-export 1 dòng từ `mod opc`
  private của crate) — tên traversal không thể resolve ra đĩa; `..`/`\` → 403 tường minh
  TRƯỚC lookup (security path, không phải dead code). Test cargo ×2 trên socket thật:
  byte-exact part == fixture bytes + bộ 403/404 (literal `../`, `%2e%2e` encoded, backslash,
  part lạ, fileId lạ, thiếu part).
- **Render (`viewer.ts`):** ảnh từ `Run.images` (ImageRef: part + cx/cy EMU) thành token
  **dòng riêng** (ceiling `ponytail:` — không inline wrap; ảnh vẽ sau text của run do parser
  flatten thứ tự con); `drawImage` tại emu/9525; fetch qua `fetch()`+`createImageBitmap` —
  **canvas không taint** nên `getImageData` đọc được (HTMLImageElement cross-origin sẽ taint).
  Bytes fail → reserved box + log warn, view không chết.
- **Bằng chứng pixel (không chỉ text-ink):** log `viewer media: part emu drawn at ink` —
  ink = đếm pixel non-white trong rect vừa vẽ. E2E boot nền (rule 6b): fixture layout
  (2 ảnh body extent 100×50/50×30 px, PNG 2×2/3×2 màu đậm) → drawn **100.0×50.0** at
  **398.3** (= 380.4 last-baseline + 17.9 lh — vị trí flow đúng), **ink 20000 = 100×50@dpr²
  phủ kín**; ảnh 2 kề ảnh 1 (448.3 = 398.3+50). Fixture T-A26 media (body 2×2): drawn
  **2.0×2.0** ink 16 — bytes thật ≠ placeholder trắng (placeholder sẽ ink ≈ 0). Header image
  KHÔNG render (RenderDoc.body chỉ body — ceiling ghi DD; route vẫn serve bytes part header,
  curl 200 byte-exact sha256 == fixture).
- **Gate 12 mở rộng** (log contract ADDITIVE `viewer media:`/`viewer media-failed:`, hằng
  `SMOKE_RE_QV_MEDIA` vào khối đóng băng smoke-lib CÙNG commit): assert media — drawn ==
  EMU/9525 (gate TỰ tính lại từ EMU trong log, không tin viewer), ink ≥ ½ rect, ảnh đầu đúng
  vị trí flow, ảnh kề advance = chiều cao ảnh, cấm media-failed. Bằng chứng: positive exit 0
  (first-draw 27–32 ms, tree 261–265 MB ≤ 300); negative ×2 exit 1 (MAX_MS=1 → "first-draw
  30.0 ms exceeds budget"; MAX_MB=1 → "RSS tree 257 MB exceeds budget"); **counterfactual
  fake-log ×5 đều exit 1 đúng thông điệp** (drawn sai 99×50 / ink thủng 20 / vị trí trôi +30 /
  mất media line / thêm media-failed) — assert media cắn thật mà không đụng product code.
- **Bug thật gate bắt trong phiên (confirmed):** breakLine thừa sau token ảnh → mỗi image
  paragraph sinh "dòng rỗng ma" lh 16.9 (fallback 1.15) lọt vào `stats.lhs` — assert uniform
  của gate cắn ngay run đầu (17.9 vs 16.9 > 0.15). Fix: chỉ breakLine sau ảnh khi còn token
  phía sau (wrapParagraph indexed). Bài học: assert tọa độ bắt regression layout mà "nhìn
  có vẻ đẹp" không thấy.
- **Verify:** `cargo test --workspace` exit 0 (shell lib 26 passed + 1 ignored = 27, crate 8+4=12) · clippy
  clean (3 warning mới fix: needless_borrow ×2 + unused_mut) · `cargo fmt` (lần đầu verify
  FULL FAIL do quên fmt filesrv.rs — not code, quy trình) · `pnpm build` exit 0 ·
  parity-a2.sh exit 0 (9 file, 0 hard-fail) · **verify.sh FULL exit 0 (17 gates)** ·
  ponytail-review trên diff: 1 finding (dead binding `atx` trong gate python) → fix.
- **Trạng thái:** T-V3 ✅ — ▶ = T-V4 (parity render vs oracle CDP).

---

## 2026-10-03 · T-V2: A3/V2 — layout tối thiểu (wrap pgSz + tab + line-height C4-a) + gate số liệu đầu tiên (đóng OQ-V01-2)

- **Bối cảnh:** slice V2 viewer (A3) — layout chạy thật trên canvas V0 + gate verify.sh
  đầu tiên của quick view. Clean-room: chỉ spec-ref (defaultTabStop §17.3.1.15 — 720 twips
  là quy ước Word, KHÔNG phải default spec, verify qua nguồn mở) + oracle (log layout app
  chạy thật); không đọc upstream render code.
- **Layout (`src/viewer.ts` + payload `viewer.rs`):** `viewer_open` trả thêm `fonts` (map
  family → OS/2 [ascent, descent, line_gap] scaled 1000/em qua accessor C4-a) + `page`
  (pgSz). TS layout: greedy wrap trong cột suy từ pgSz (A4 → 601.7 px trừ 2×1in margin —
  pgMar chưa vào model) · tab stop 720 twips (jump, không flat-space nữa) · line-height =
  (asc + |desc| + gap)/1000 × px — **accessor `allfonts::line_metrics` scan CHỂ bundled root
  ~20 file** (không trả giá 557 mặt hệ thống/lần mở; family ngoài bundle → fallback 1.15,
  ceiling `ponytail:` comment). Break controls `\n\f\v` giữ nguyên hành vi ép dòng.
- **Bằng chứng layout (assert theo tọa độ, không phải "nhìn thấy"):** fixture
  `make_docx_layout.py` (4 paragraph uniform Carlito 11pt, A4) — viewer log
  `viewer layout: lines=16 colw=601.7 maxw=600.5 metrics=750,-250,220 lhs=17.9×16
  baselines=112.0,129.9,…` — **mỗi delta baseline == line-height của dòng == công thức
  OS/2 tính lại từ metrics trong log** (17.9 px = 1.22 × 14.667); wrap ăn thật (16 dòng > 4
  paragraph; filler tự bẻ 12 dòng); maxw ≤ colw. Assert này sống trong gate (python đọc log),
  viewer.ts chỉ phát dữ liệu.
- **Gate 12 `smoke-quick-view.sh`** (verify.sh, pattern smoke-lib + hằng `SMOKE_RE_QV_*`
  vào khối đóng băng — emitting end `src/viewer.ts`): assert layout (trên) + first-draw
  < 500 ms + **RSS tree < 300 MB** (app + 3 WebKit XPC, method PH-0 PID-adjacency). Bằng
  chứng: **positive exit 0** ×3 (`first-draw 18–26 ms, tree 253–257 MB ≤ 300`); **negative
  ×2 exit 1 fail-fast**: `QUICK_VIEW_MAX_MS=1` → `first-draw 19.0 ms exceeds budget`;
  `QUICK_VIEW_MAX_MB=1` → `RSS tree 257 MB exceeds budget` — threshold là nội bộ gate nên
  negative = siết ngưỡng (không đụng product code); assert ms và assert RAM cắn RIÊNG lẻ.
- **OQ-V01-2 CHỐT = TREE (app + WebKit XPC, PID adjacency)** — DD V01 §10: (1) app-only mù
  đúng thành phần A3 loại (WebContent 370→46 MB); (2) đích rung so với engine 561–579 MB là
  toàn cây; (3) method PH-0 proven. **Phát hiện method: helper WebKit spawn bởi launchd,
  KHÔNG phải con app — `pgrep -P` đệ quy trả rỗng (thử thật 03.10); DD §6 mitigation sửa
  theo PID adjacency.** Ngưỡng: gate = 300 MB tripwire DEBUG (đo được 178–257, helper noisy);
  đích rung <150 MB = criterion RELEASE (debug idle đã ~203 tree), đo ở PH-4.
- **Blocker mới ghi nhận (observed, KHÔNG phải T-V2 — boot release không quick-view cũng
  stall):** release boot webview không navigate — bare binary: cửa sổ lên + WebContent spawn
  rồi idle hẳn (sample: run loop `mach_msg`, không nhận navigation) + 0 `page_load`; bundle
  `.app` qua `open -g --env` = 0 cửa sổ. Đã loại App Nap/caffeinate/đợi >90 s; debug boot
  cùng máy tức thì → nghiêng về WebKit custom-scheme handling trong release, chưa mổ. Nếu
  muốn tái lập: `cargo build --release` + `NANO_OFFICE_QUICK_VIEW=… ./target/release/nanooffice`
  (log dừng ở `loopback file server`). → **T-057 (PH-4)** — chặn đường đo đích rung RAM release.
- **Trạng thái:** T-V2 ✅ — gate 12 thường trụ; ▶ = T-V3 (media).

---

## 2026-10-03 · T-V1: A3/V1 — model render-side mở (DD V01 §3.2) + resolve chuỗi style về Rust

- **Bối cảnh:** slice V1 của viewer (A3) — crate `nanooffice-ooxml` mở model theo bảng
  construct C2 (DD V01 §3.2, spec-ref từng mục) và thay merge TS bằng resolve về Rust.
  Clean-room: chỉ spec-ref ECMA-376 + oracle (parity harness / smoke-render), không đọc
  upstream render code.
- **Model mở (parse + write đối xứng từng mục):** `Run.bold/italic` lên **3-state ST_OnOff**
  (`None` = kế thừa / `Some(false)` = tắt tường minh — limitation 2-state T-A22 hết); `w:rStyle`
  §17.5.2.3; `w:rFonts` §17.3.2.26 (ascii/hAnsi/eastAsia/cs, literal — theme attrs không model);
  `w:color` §17.3.2.6 (passthrough); `w:spacing` §17.3.2.33 (i32 twips); **`w:br`/`w:tab` →
  ký tự điều khiển trong `Run.text`** (`\n` text / `\u{c}` page / `\u{b}` column / `\t` —
  3 control chars đầu ILLEGAL trong XML content nên không thể đụng `w:t`; `w:type` lạ → Err
  theo ST_BrType §17.18.49 closed enum; cả 2 serialization Start+End lẫn Empty — bài học
  a:blip T-A23); `w:ind` §17.3.1.12 (4 attr signed); `w:tblGrid` §17.4.4 (`gridCol` thiếu
  `w:w` = auto, sai kiểu → Err); `w:pgSz` §17.6.11 từ `w:sectPr` body-level; `w:default`
  §17.7.2.4 trên `w:style` (**attribute — absent = false**, KHÔNG phải OnOff-element semantics;
  bug này test round-trip bắt ngay lần đầu) + `w:docDefaults` §17.7.5.1 (rPrDefault/pPrDefault,
  chung parser với style rPr/pPr).
- **Resolve về Rust (`resolve.rs`):** `resolve(&Document) -> RenderDoc` — thứ bậc §17.7.2:
  direct → rStyle (character style) → pStyle hoặc style `w:default` khi không pStyle →
  docDefaults; con thắng cha (basedOn đã resolve sẵn trong styles.rs); rStyle/pStyle trỏ style
  sai loại hoặc thiếu → bỏ lớp đó (quick view không chặn open); `u val="none"` và `color="auto"`
  collapse về `None`; font = ascii→hAnsi→eastAsia→cs. `viewer_open` trả `RenderDoc` —
  **viewer.ts xoá sạch merge chain** (`styleOf`/interface style hết), chỉ còn fallback terminal
  (11pt/sans-serif) khi cả docDefaults cũng không khai báo.
- **Bằng chứng:**
  - Test mới: `v1_render_constructs_parse_resolve_and_round_trip` (inline XML — assert
    RESOLVED values từng thứ bậc: default-para-style, rStyle>pStyle, direct>rStyle, explicit-off
    thắng chain, ind direct thắng style, ascii font, br/tab 2 serialization, gridCol auto,
    pgSz, round-trip bất biến) + `v1_fixture_parses_resolved_values` (file thật — Carlito từ
    docDefaults, color chain, underline none→None, ind, grid) + cycle/override có sẵn
    (malformed (f) + run-override test nâng cấp assert `Some(false)`).
  - Fuzz: 500 vòng mutation (parse) + 500 vòng (write+reparse, JUNK có `\t`/`\n` → đi qua
    w:tab/w:br element) — 0 panic, model bất biến từng vòng.
  - **parity-a2.sh exit 0** — invariant 9/9 (corpus 8 + v1-fixture mới qua glob); diff phân
    loại ĐỦ: 4 lớp vô hại — 2 cũ (empty-run drop, rel_id renumber/rename) + **2 mới do model
    surface hóa: x2t thêm `w:docDefaults` rỗng (8/9 file) + hex color lowercase** (DD K01 §5.1
    bổ sung (c)/(d)). m-x2t của v1-fixture lệch duy nhất 3 dòng màu → x2t giữ nguyên mọi
    construct V1 qua round-trip.
  - **Engine nhận XML writer mới:** crate round-trip của v1-fixture qua `smoke-render.sh`
    **PASSED exit 0** (contentReady + canvas + /fonts/) + adapter pre-normalize chạy trên đó
    (`rust-canonical → x2t, normalize 3.3 ms`).
  - **Viewer render GREEN không thoái hóa:** boot nền (rule 6b) `NANO_OFFICE_QUICK_VIEW` —
    v1-fixture: parse **2.5 ms** · first-draw **20.0 ms** · inked 27499; full-fixture: parse
    **4.8 ms** (V0 2.9 — thêm pgSz/tblGrid/sectPr + resolve) · first-draw **24.0 ms** ·
    inked 27547 (khớp V0); app RSS **~92–100 MB** (V0 117.8). Số liệu ghi DD V01 §5.2.
  - `cargo test --workspace` 23 shell + 12 crate exit 0 · `cargo clippy -D warnings` exit 0 ·
    `pnpm build` exit 0 · **verify.sh FULL exit 0** (16 gates; lần chạy đầu FAIL do port 1420
    còn listener từ boot viewer — dọn process, rerun sạch; không phải code).
  - T-056 cuts trigger đúng lúc chạm fixtures: `docxgen_common.py` (png_1x1×3 → 1 nhà, chữ ký
    `png_1x1(r,g,b)`/`png(rgb_rows)`) + xoá 2 dead line `make_docx_edge.py` __main__ — verify
    byte-identical: edge corpus 4 file byte-identical, full/media part-bytes identical.
- **Trạng thái:** T-V1 ✅ — ▶ = T-V2 (layout + gates số liệu). CẢN T-V2 được giữ: không làm
  layout/gate RAM trong phiên này.

---

## 2026-10-03 · T-A25: A3 mở màn — viewer Rust đọc-only, slice V0 chạy thật + số đo ms/MB đầu tiên (design-first như T-A20)

- **Bối cảnh:** mở màn rung A3 (QĐ9 — "quick view" không boot sdkjs; đích rung mở <500 ms,
  RAM <150 MB, parity render vs oracle). DD **V01** `docs/design/viewer-a3/DD.md` (mã nhóm V
  mới, folder giữ `viewer-a3/` theo handoff — tiền lệ K01): render = **R1 canvas 2D
  "PDF-style"** trong webview riêng (R2 DOM-flow và R3 qua-PDF bị loại — không mở đường A4);
  deps review #3 nạp hết: **C3** viewer cắm COMMAND LAYER song song delivery (`viewer.rs`
  parse-only, không sidecar seam/FileRegistry/delivery.js) · **C4 chốt (a)** accessor OS/2
  line metrics cho V2, KHÔNG parser hmtx (canvas `measureText` đo advance font thật — native
  platform; trigger mở lại (b) ghi rõ) · **C2** bảng construct render-side + spec-ref từng
  mục → task T-V1.
- **Slice V0 (chạy thật, không layout):** `src-tauri/src/viewer.rs` — command
  `viewer_open` đọc file từ env `NANO_OFFICE_QUICK_VIEW` → `nanooffice_ooxml::parse_docx`
  (in-process, không CLI) → JSON model; `viewer.html` + `src/viewer.ts` — entry vite riêng
  (không shim, không engine iframe), canvas 2D vẽ 1 dòng/paragraph (align center/right,
  underline, style chain + run override, table = cell text theo cột); window quick-view mở
  bằng return-sớm trong setup — đường boot engine nguyên văn. Log ADDITIVE
  `viewer: parsed … in N ms` + `[web] viewer first-draw … ms (…, inked N)` — log contract
  gates 9/10/11 không đụng.
- **Số đo đầu tiên** (fixture `make_docx_full.py` 3794 B, debug build, ×2 run nhất quán;
  lệnh: `NANO_OFFICE_QUICK_VIEW=/tmp/ta25-quickview.docx pnpm tauri dev`, poll log):
  - **parse 2.9 ms** (crate A2) · **invoke→first text pixel 14.0 ms** (run1 17.0 ms).
  - **Render evidence:** `inked 27547` — sample pixel tối trên canvas, text thật sự vẽ.
  - **RAM (method PH-0 diagnostics 27.09: `ps -o rss=` app + 3 WebKit XPC):** app process
    **117.8 MB** ổn định 2 run; +helpers (WC 46.5/GPU 49.3/Net 22.7) ≈ **236 MB**.
  - **Đọc:** "không boot sdkjs" hiện thực — WebContent rớt ~370 MB (engine 1-doc 561–579 MB)
    → ~46 MB; app-only 117.8 MB < 150 MB đích ✓, nhưng +helpers 236 MB > 150 MB → **T-V2
    phải chốt công thức RAM (OQ-V01-2) + đo lại release build** trước khi đặt gate. Idle
    start page debug cũng ~203 MB cùng method — đích rung không thể tính tree trên debug.
- **Bằng chứng verify:** `cargo check/test --workspace` exit 0 (23 shell + 10 crate) ·
  `pnpm build` exit 0 (2 entry vite: main + viewer) · `cargo fmt --check` clean ·
  `verify.sh` FULL exit 0 (kết quả trong báo cáo phiên) · đường boot thường không đụng
  (verify smoke gates boot app KHÔNG env quick-view vẫn GREEN).
- **Trạng thái:** T-A25 ✅ — A3 MỞ MÀN (không đóng rung: parity + gates + UX là T-V1..T-V5);
  ▶ = T-V1.

---

## 2026-10-03 · T-A26: fix canonical lossy media bytes (review #3 C1) — writer copy media part từ zip nguồn

- **Triệu chứng (C1, confirmed bởi review #3):** adapter open-side (T-A24) thay MỌI ảnh thật
  bằng placeholder PNG 1×1 trên mọi docx có ảnh qua luồng canonical — model S1 không giữ bytes,
  writer ghi placeholder cho mọi media part; invisible với gates vì corpus toàn PNG 1×1.
- **Fix (approach ponytail — ít đụng model nhất trong 2 phương án handoff):** model KHÔNG đổi.
  `write_docx`/`write_docx_file` nhận thêm `media_src: Option<&[u8]>` (package nguồn) — writer
  copy từng media part **byte-exact** từ nguồn qua `opc::try_part_bytes`; placeholder chỉ còn
  fallback khi `None` hoặc part thiếu (model synthetic/test). JSON CLI bất biến → invariant
  parity không đổi. Callers: adapter `rust_canonical` đọc src 1 lần → `parse_docx(&bytes)` →
  write với `Some(&bytes)`; CLI round-trip face tương tự.
- **Bằng chứng:**
  - fixture mới `nanooffice-media-fixture.docx` (generator `harness/fixtures/make_docx_media.py`,
    python stdlib): PNG 2×2 79 B (body) + PNG 3×1 74 B (header — phủ branch header-image
    write.rs:170-188, C5 coverage hole) — bytes ≠ placeholder 72 B; có `sectPr`/`headerReference`
    (header mồ côi KHÔNG sectPr bị x2t bỏ khi open — oracle-obs lần đầu, parity 42-line diff,
    fix bằng fixture thực tế hơn chứ không phải code). Regen deterministic byte-identical.
  - Test hồi quy `media_bytes_copied_byte_exact_from_source_package` (crate tests/write.rs):
    so BYTES từng media part giữa input zip và canonical output (không phải kích thước) +
    model bất biến qua round-trip có nguồn + `None` → placeholder khác gốc. Helper
    `media_parts()` tái dùng cho cả 3 assert.
  - `parity-a2.sh` thêm gate (d) media byte-exact (`part_sha` python3 stdlib — không dep
    unzip/jq): crate-rt phải giữ bytes mọi part mà model tham chiếu; x2t side không gate
    (serialization oracle). **exit 0 — 8/8 invariant, media gate GREEN**; diff phân loại:
    x2t renumber rel_id + **ĐỔI TÊN media part** (image1↔image2 swap theo rel mới — mapping
    rel→part tự-consistent, bytes nguyên vẹn: sha input image1 `4c8f04482971` = x2t-rt
    image2) — mở rộng lớp vô hại "rel_id renumber" của T-A23.
  - E2E adapter ON: `smoke-render.sh nanooffice-media-fixture.docx` **exit 0** — log
    `converter: rust-canonical → x2t (…, normalize 2.48 ms)`; counterfactual
    `NANO_OFFICE_NO_RUST_CONVERTER=1` **exit 0** — log `rust-canonical OFF … — x2t direct`.
  - `cargo test --workspace` 23 shell + 10 crate exit 0; `verify.sh` FULL **exit 0** (sau fmt —
    lần đầu fail duy nhất do `cargo fmt --check` trên test mới).
- **Trạng thái:** T-A26 ✅ sink Track A — canonical KHÔNG còn lossy trên media; bytes gốc
  sẵn trong package nguồn cho C3 của T-A25 (serve `word/media/*`). ▶ = T-A25 (A3 viewer).

---

## 2026-10-03 · Review kiến trúc #3 (T-055) — post-A2, focus cản A3 viewer

- **Phương pháp:** bộ đôi improve-codebase-architecture + ponytail-audit (codebase-design từ vựng),
  hot-spot `git 9b1dde5..HEAD` (T-A23 parity + T-A24 adapter, 8 commits) + bề mặt A2 nguyên vẹn;
  3 claim chịu lực kiểm chứng first-hand: (1) crate = in-process lib-dep của app
  (`src-tauri/src/sidecar/mod.rs:248-251` gọi `nanooffice_ooxml::parse_docx_file` trực tiếp — viewer
  không cần CLI/JSON) · (2) `ImageRef` không mang bytes (`crates/.../src/lib.rs:95-108` — "S1 reads
  relationships only, image bytes are not touched") · (3) đường open duy nhất = delivery-base64
  (`src-tauri/src/lib.rs:131-198` `get_document_for_delivery`, đọc toàn file delivery.js). Explore
  subagent quét styles/document/allfonts/formats/CLI/write/corpus (prompt kèm mit-purity).
- **Phát hiện chính (C1, observed — bug fidelity sống):** canonical path thay ảnh thật bằng
  placeholder PNG 1×1 trên MỌI docx có ảnh qua adapter ON (parse bỏ bytes → write placeholder →
  x2t convert canonical) — invisible với toàn bộ gates vì corpus chỉ có PNG 1×1 / không ảnh
  (make_docx_full, make_docx_edge, engine-save-t011). T-A24 đã ghi "fidelity ceiling LOSSY" như
  limitation; review nâng thành việc TRƯỚC A3 vì bytes-aw-model cũng là điều kiện render ảnh viewer.
- **Cản A3 còn lại:** C2 model S1 thiếu dữ liệu render (w:rFonts không parse, w:br/w:tab bị bỏ ở
  `document.rs:243` `Event::Empty(_) => {}`, Run bool 2-state vs StyleDef 3-state, không rStyle/
  w:default/color/spacing/tblGrid/pgSz) · C3 điểm cắm viewer = command layer song song delivery
  (không có Tauri command parse-only, không có đường serve `word/media/*` bytes) — **sidecar seam
  KHÔNG cắm viewer** (viewer bỏ qua x2t/Editor.bin; thêm shape viewer vào converter = wrong-home) ·
  C4 font metrics: allfonts.rs parse đủ OS/2 line metrics nhưng toàn pub(crate) + struct private,
  không có hmtx (advance thật) — quyết định (a) accessor rẻ vs (b) hmtx thuộc DD T-A25.
- **Ponytail (C5):** make_docx_edge.py:223-224 dead `out/print` (delete) · png_1x1/chunk lặp ×3
  generator → common (−~30 dòng) · write.rs:170-188 branch header-image chưa có corpus phủ ·
  parity-a2.sh + adapter + parse_alt_content = lean/evidence-driven, giữ.
- **Land:** T-A26 (C1, urgent, độc lập T-A25) + T-A25 nạp deps review #3 (C2/C3/C4) + T-056 (C5,
  trigger chạm fixtures kế) + T-055 đặt lại ⬜ (nhịp, lần kế post-A3). Report HTML tạm
  `$TMPDIR/architecture-review-20261003-t055.html` (không vào repo). Không mâu thuẫn QĐ1–QĐ9;
  không đề xuất lại T-A14..T-054.
- **Trạng thái:** confirmed (verify.sh chạy cuối phiên — kết quả trong báo cáo).

---

## 2026-10-03 · T-A24: A2/S4 — adapter crate vào đường runtime (open-side canonical → x2t) — rung A2 milestone

- **Chốt chỗ cắm (Bước 1, trước khi code):** save-side KHÔNG thay được — save hôm nay là x2t
  fromChanges trực tiếp trên Editor.bin (bridge/save.rs), mà crate KHÔNG sản xuất Editor.bin
  (DD §2 P2: Editor.bin giữ x2t đến A4) → đề xuất A chết tự nhiên. Chọn **đề xuất B
  (open-side pre-normalize)** cắm trong seam `sidecar::convert` nhánh `OpenBinary` (đúng chỗ
  T-A14 dự liệu "second adapter"): src `.docx` → crate `parse_docx_file` + `write_docx_file` →
  `canonical.docx` trong workdir → x2t chuyển **canonical** ra Editor.bin. xlsx/pptx/pdf không
  đụng (scope crate = docx; S5 mới mở rộng). Crate parse/write lỗi (refuse-unknown) → WARN +
  x2t nhận **file gốc** — file crate từ chối vẫn mở đúng như trước, không bao giờ chặn open.
- **Kill switch:** `NANO_OFFICE_NO_RUST_CONVERTER` set (≠"0") → đường cũ nguyên vẹn (mirror
  semantics `NANO_OFFICE_NO_APP_FALLBACK` của T-A13); mặc định ON. Counterfactual = gate 9/10
  chạy cả 2 chế độ.
- **Log ADDITIVE (không đụng contract gates 9/10/11):** `converter: rust-canonical → x2t (…,
  normalize …)` (ON) · `converter: rust-canonical failed (…) — x2t takes the original file`
  (fallback WARN) · `converter: rust-canonical OFF (…)` (kill-switch evidence). Không dòng nào
  chạm chuỗi grep đóng băng trong smoke-lib.sh.
- **Fidelity ceiling chấp nhận có chủ đích:** model S1 chưa surface image bytes (writer ghi
  placeholder PNG 1×1 — T-A22), sectPr, hyperlink, rStyle → canonical là LOSSY trên file thật
  có construct ngoài model surface; kill switch là rollback tức thời; bytes thật + construct
  còn lại là việc tự nhiên của A3 (viewer dùng model trực tiếp → image bytes trở thành bắt
  buộc). Gate 9/10 fixture nằm trọn trong model surface (invariant 7/7 T-A23).
- **Bằng chứng:** verify.sh **FULL exit 0** chế độ ON (`/tmp/verify-ta24.log` — 6 smoke PASSED;
  log app giữ tại `/tmp/nanooffice-smoke-ezoNVw` …): gate 9 `converter: rust-canonical → x2t
  (…, normalize 2.08 ms)` → `x2t converted … Editor.bin (1495 bytes) in 114.6 ms`; gate 10
  save round-trip GREEN (marker ×1, 25562 B); gate 11 clean-machine GREEN. **Counterfactual
  OFF** (`NANO_OFFICE_NO_RUST_CONVERTER=1`): gate 9 exit 0 — log `converter: rust-canonical
  OFF (…) — x2t direct`, Editor.bin **1586 B = đường cũ** (vs 1495 B canonical — hai đường
  phân biệt được bằng kích thước binary); gate 10 exit 0 (marker ×1, 25570 B). Log contract
  gates 9/10/11 byte-identical (smoke-lib.sh hằng đóng băng không đụng). `parity-a2.sh` exit 0
  (7/7 parse + invariant, 0 hard-fail — đúng baseline T-A23). `cargo test --workspace` 23
  shell + 9 crate exit 0 — test seam mở rộng: (d) docx qua adapter, x2t nhận ĐÚNG canonical
  (so bytes), (e) file hỏng → fallback không chặn open, (f) kill switch không sinh canonical.
- **Overhead đo được:** normalize ≈ **2 ms/open** (2.08/1.96/1.86 ms trên 3 gate) trên fixture
  nhỏ; x2t 37–130 ms cùng run — tổng open tăng <2%, không thấy được từ ngoài.
- **Residual risk (ghi nhận, không sửa):** nếu x2t từ chối canonical của một file crate parse
  được (chưa từng obs — T-A22 render GREEN + T-A23 parity) → open lỗi thay vì fallback tự
  động; khi recur, thêm retry-on-original trong `rust_canonical`.
- **Trạng thái:** T-A24 ✅ sink Track A — **RUNG A2 ĐÓNG** (vision QĐ9 bảng thang A2 ✅). ▶ =
  T-A25 (A3 mở màn — viewer Rust đọc-only "quick view" không boot sdkjs).

---

## 2026-10-03 · T-A23: A2/S3 — parity harness (crate ↔ x2t trên model): 15/15 construct parity, 0 diff không phân loại — GREEN

- **Phạm vi:** slice S3 của DD K01 §4/§5 — `harness/scripts/parity-a2.sh` (~115 dòng, script
  mỏng + CLI crate có sẵn, không framework compare): mỗi corpus file chạy x2t round-trip
  (docx→Editor.bin positional + params XML Editor.bin→docx — đúng 2 call shape của seam) +
  crate round-trip, rồi 3-way compare trên canonical JSON CLI: **INVARIANT m-orig == m-crate**
  (gate cứng — crate round-trip phải exact) + **PARITY m-crate vs m-x2t** (số liệu đo, phân
  loại bằng tay vào DD §5.1). x2t = DEV-ORACLE (so output hành vi, không reverse).
- **Corpus 7 file:** 2 fixture crate + 4 fixture mới `make_docx_edge.py` (multilevel-list 3
  mức, nested-table 1 cấp, style-override chain Quote→IntenseQuote + rStyle, image-multi 3
  drawing cùng rel) + **engine-save-t011.docx = file thật gate 10** (smoke-save, marker
  `T011-10466-SAVED-BY-NANOOFFICE`, package do x2t save sinh — styleIds numeric "138"…).
  Expected-model note từng file: `harness/fixtures/corpus-a2/NOTES.md`.
- **2 bug crate bắt được qua parity probing (fix + test hồi quy):** (1) `mc:AlternateContent` —
  x2t/Word bọc `w:drawing` trong `mc:Choice` + VML `mc:Fallback`; parser skip cả subtree → mất
  image trên mọi file x2t-rt (image trong package, rel r:id9 đúng — parse không thấy). Fix:
  `parse_alt_content` chọn branch `mc:Choice` ĐẦU TIÊN, xử lý làm run content; Fallback bỏ.
  Spec-ref: ECMA-376 Markup Compatibility AlternateContent — "processed as though it replaces
  the entire AlternateContent element" (quote từ spec text browsable); **số clause 29500-3
  cụ thể KHÔNG verify được từ nguồn mở** (mirror open-xml-docs Remarks trống cho mc classes;
  MS protocol docs dẫn §10.2.1 qua search — chưa độc lập xác nhận → KHÔNG ghi số, lần sau
  cần số thì fetch spec bản quyền). (2) `a:blip` chỉ đọc ở `Event::Empty` — x2t ghi
  `<a:blip r:embed=…></a:blip>` Start+End → rớt embed (bắt được vì fixture tự viết dùng
  self-closing). Fix: Start|Empty như `wp:extent`. Bài học: **x2t writer trộn 2 serialization**
  — `<wp:extent …/>` self-closing NHƯNG `<a:blip …></a:blip>` có end tag, `<pic:cNvPr …></pic:cNvPr>`
  cũng vậy → mọi parser đọc element đơn lẻ phải handle cả 2 dạng.
- **Phân loại diff (đầy đủ trong DD §5.1):** 2 lớp x2t-side vô hại — (a) rel_id renumber
  (rIdImg→rId9/rId6; part/extent/target nguyên vẹn; id quan hệ = chuỗi tùy ý Part 2);
  (b) empty-run drop (engine-save có 8/14 run `<w:r></w:r>` thật trong XML gốc — crate
  preserve 14/14, x2t roundtrip drop 8; 0 hunks đổi text/props). 0 diff không hiểu.
- **Không nâng model:** Run.bold 3-state + sectPr surface KHÔNG bị parity đòi (x2t giữ
  explicit `w:b w:val="false"` trên output — model 2-state invisible; hdr/ftr surface qua rels
  cả 2 phía). Blind-spot ghi DD: `rStyle` không nằm trong model — parity "mù" ở construct đó.
- **Bằng chứng:** harness ×2 deterministic — 7/7 parse ok, invariant 7/7, parity-clean 4/7
  (3 file diff = đúng 2 lớp vô hại trên), exit 0 (`/tmp/nanooffice-parity-a2-VNTEE6`);
  crate tests 9 passed (6 parse + 3 write — thêm `alternate_content_choice_surfaces_image`
  mirror cấu trúc x2t: Choice + Fallback VML + `<a:blip></a:blip>` Start+End + round-trip
  bất biến); oracle-obs probe thủ công: x2t full-fixture 2970 B Editor.bin, params exit 0.
- **Quan sát (không sửa):** x2t lần-chạy-đầu-tiên sau bundle có thể fail "Couldn't create
  temp folder" rồi run sau exit 0 cùng cwd (nghi Gatekeeper first-run scan macOS) — harness
  không retry; nếu recur khi CI thì xem lại. Lần chạy đầu phiên cũng dính (probe 1 fail,
  probe 2 ok).
- **Trạng thái:** T-A23 ✅ sink Track A. ▶ = T-A24 (S4 — cắm adapter `sidecar::convert`,
  open path trước → rung A2 milestone khi cả 2 path qua adapter).

---

## 2026-10-02 · T-A22: A2/S2 — docx WRITE + round-trip, file ghi ra render GREEN — GREEN

- **Phạm vi:** slice S2 của DD K01 §4 — `src/write.rs` serializer model → docx: 1 hàm/
  construct đối xứng parser (`PartWriter::serialize/blocks/paragraph/run/drawing/table` +
  `styles_xml` + `numbering_xml` + `rels_xml`/`content_types_xml`), quick-xml `escape()`,
  zip crate có sẵn (deflate), không builder/trait (ponytail ULTRA). Package tự sinh đúng
  chuẩn Part 2: part chính = canonical name `word/document.xml` (reader tra cứu qua root
  rels — writer chỉ chọn tên; package self-consistent by construction), image rel ids GIỮ
  id từ model (r:embed phải re-resolve identically hoặc round-trip gãy), header/footer rel
  ids alloc mới tránh đụng, styles/numbering rel slots cố định (rIdStyles/rIdNumbering),
  refuse image extension ngoài bảng media type (webp… → `WriteError`, không đoán).
- **Clean-room:** 19 clause DrawingML verify qua bảng children/parents trong Remarks mirror
  (raw XML từ OfficeDev/open-xml-docs-ref-dotnet): drawing §17.3.3.9 · wp:inline §20.4.2.8 ·
  wp:extent §20.4.2.7 · a:graphic §20.1.2.2.16 · a:graphicData §20.1.2.2.17 · pic:pic
  §20.2.2.5 · pic:nvPicPr §20.2.2.4 · pic:cNvPr §20.2.2.3 · pic:cNvPicPr §20.2.2.2 ·
  pic:blipFill §20.2.2.1 · a:blip §20.1.8.13 · a:stretch §20.1.8.56 · a:fillRect §20.1.8.30 ·
  pic:spPr §20.2.2.6 · a:xfrm §20.1.7.6 · a:off §20.1.7.4 · a:ext §20.1.7.3 · a:prstGeom
  §20.1.9.18 · a:avLst §20.1.9.5. Bài học T-A21 LẶP LẠI: ký ức số section sai 5/19
  (cNvPr/cNvPicPr đảo, stretch không phải .47, fillRect không phải .28, xfrm không phải
  .7.5, prstGeom không phải .8.10) — số clause của MỘT element nằm ở bảng children của
  element CHA (hoặc parents của con), không ở remark của chính nó.
- **Model change (schema bắt buộc — theo handoff):** `StyleDef` +`kind: StyleKind`
  (Paragraph/Character) — §17.7.4.17 `@w:type` là REQUIRED attribute, writer không thể sinh
  styles.xml đúng schema thiếu nó; +derive `PartialEq`/`Clone` (round-trip compare + fuzz —
  không đổi schema). Styles serialize FLAT (model đã resolve basedOn → không sinh basedOn;
  re-parse resolve về chính nó = cùng model). Option 3-state của style sinh explicit kể cả
  `Some(false)`.
- **Limitation ghi rõ (model S1, không phải bug writer):** `Run.bold/italic` là bool
  2-state → `false` serialize ABSENT (không phải `w:b w:val="false"`) — explicit-off
  override của style bold MẤT intent trong file output (corpus p2 sẽ render bold khi mở);
  model 3-state là việc khi parity/render yêu cầu. Tương tự `sectPr` không surface trong
  model → header/footer parts được ghi kèm relationships nhưng không được reference từ
  section (reader surface hdr/ftr từ rels nên round-trip vẫn bất biến).
- **Image bytes:** model S1 relationship-only → media part ghi placeholder PNG 1×1 tự
  encode std-thuần (RFC 2083 + RFC 1950/1951 stored block, crc32/adler32 tự viết, 72 B);
  relationship + part + content type là dữ liệu thật. Bytes thật là việc S3+ nếu parity cần.
- **2 bug bắt được qua chính round-trip test:** (1) writer thiếu wrapper `w:body` cho main
  part (hdr/ftr giữ blocks trực tiếp, main phải wrap — "no w:body in document" khi re-parse);
  (2) quên relationships cho styles.xml/numbering.xml (body khớp nhưng dict trống khi
  re-parse — reader tra accessory parts qua doc rels).
- **Bằng chứng:** `cargo test -p nanooffice-ooxml` 8 passed (5 read + 3 write mới: corpus
  round-trip bất biến + edge XML escape/xml:space/webp-refuse + fuzz 500 vòng
  mutation-on-model — write không panic, reparse luôn Ok, model bằng từng vòng);
  `cargo test --workspace` 23 shell (+1 ignored) + 8 crate exit 0; oracle-obs x2t đọc file
  ghi ra exit 0 (Editor.bin 2467 B); **`smoke-render.sh /tmp/ta22-roundtrip.docx` PASSED
  exit 0 ×2** (contentReady + canvas 7 canvas 2302x958 + /fonts/ — gate pattern 9);
  verify.sh **FULL exit 0** 16 gates (`/tmp/verify-ta22.log`).
- **Quan sát (không sửa):** clippy 1.93.0 phát hiện mới `manual_is_multiple_of` flag
  `src/protocol/allfonts.rs:1025` ở lệnh `--all-targets` (KHÔNG flag ở lệnh verify-style
  `cargo clippy --workspace --quiet -- -D warnings` — verify exit 0). Ghi để session sau
  không ngạc nhiên nếu toolchain verify nâng cấp bật flag này vào verify-style.
- **Trạng thái:** T-A22 ✅ sink Track A. ▶ = T-A23 (S3 parity: x2t = DEV-ORACLE, so trên
  model theo DD §5) → T-A24 (S4 adapter seam).

---

## 2026-10-02 · T-A21: A2/S1 — docx READ đầy đủ (OPC chuẩn + table/list/image + styles resolve) — GREEN

- **Phạm vi:** slice S1 của DD K01 (`docs/design/converter-a2/DD.md` §4) — crate
  `nanooffice-ooxml` từ model tối giản lên read đầy đủ văn bản cơ bản: OPC đúng chuẩn
  (Content_Types + rels — part chính tra cứu qua `_rels/.rels`, KHÔNG hardcode
  `word/document.xml`; refuse content-type sai), body blocks (paragraph + `w:tbl` 2×2,
  cell lồng được), paragraph props (pStyle/jc/numPr), run props (b/i/u/sz), image
  relationship-only (`a:blip r:embed` → part + `wp:extent` EMU, không decode PNG),
  styles.xml resolve chain `basedOn` (con thắng cha, cycle/dangling → Err),
  numbering resolve (numId→abstractNum→lvl: numFmt+lvlText), headers/footers
  (rel type header/footer, root check `w:hdr`/`w:ftr`, rels RIÊNG của từng part).
- **Clean-room:** mọi parser path ghi `spec-ref:` số section thật, verify bằng bảng
  Remarks của open-xml-docs-ref-dotnet (mirror ISO/IEC 29500-1 1st ed, fetch raw XML
  từ GitHub — rẻ hơn WebFetch HTML ~20×). Bảng đã verify: document §17.2.3 · body
  §17.2.2 · p §17.3.1.22 · pPr §17.3.1.25 · pStyle §17.3.1.27 · numPr §17.3.1.19 ·
  jc §17.3.1.13 · r §17.3.2.25 · rPr §17.3.2.27 · b §17.3.2.1 · **i §17.3.2.16 · sz
  §17.3.2.38 · u §17.3.2.40** (comment cũ T-A20 ghi i=§17.3.2.13 SAI — đã sửa) ·
  **t §17.3.3.31** (comment cũ ghi §17.3.3.30 SAI — sửa) · drawing §17.3.3.9 ·
  tbl §17.4.38 · tr §17.4.79 · tc §17.4.66 · style §17.7.4.17 · basedOn §17.7.4.3 ·
  name §17.7.4.9 · abstractNum §17.9.1 · ilvl §17.9.3 · num §17.9.16 · lvl §17.9.7 ·
  numId §17.9.19 · numFmt §17.9.18 · lvlText §17.9.12 · ftr §17.10.3 · hdr §17.10.4 ·
  **headerReference §17.10.5** · sectPr §17.6.18 · wp:inline §20.4.2.8 · wp:extent
  §20.4.2.7. Bài học: đoán số section dễ lệch (basedOn không phải .1, lvlText không
  phải .6, headerReference không phải .1) — luôn verify trước khi ghi.
- **Parser recursive-descent** thay state-machine phẳng: 1 hàm/construct
  (`parse_blocks/paragraph/ppr/numpr/run/rpr/drawing/table/row/cell` + skip_element
  depth-saturating) — không visitor/trait; `Event::Empty` root (`<w:styles/>`,
  `<w:numbering/>`, `<w:document/>`) phải được coi là root hợp lệ — quick-xml emit
  Empty cho self-closing, chỉ match Start là bỏ sót (bắt được qua test header part).
  Bug bắt được khi test: `<w:tc/>` Empty phải push cell rỗng trong parse_row.
- **Corpus:** generator mới `harness/fixtures/make_docx_full.py` (stdlib, PNG 1×1 tự
  encode struct+zlib) → `tests/fixtures/nanooffice-full-fixture.docx` 10 parts: style
  chain Heading1→Normal + run override bold=false, 5 run props, table 2×2, list
  decimal/lowerLetter/bullet, image r:embed, header/footer, tiếng Việt — fixture cũ
  giữ nguyên (vẫn render GREEN trong engine, giờ đọc qua OPC lookup thay hardcode).
- **Fuzz:** 500 vòng seed 42 (LCG tự viết, không dep): XML truncate/splice-junk/
  byte-flip + zip byte-flip + namespace-sai → mọi case Ok/Err, 0 panic.
- **Bằng chứng:** `cargo test -p nanooffice-ooxml` **5 passed** (fixture cũ exact +
  corpus đầy đủ + 9 case malformed + semantics + fuzz); `cargo test --workspace`
  23 shell (+1 ignored) + 5 crate exit 0; CLI smoke JSON đủ model mới (10 blocks,
  3 styles — TableNormal skip đúng, numbering {1:[0,1], 2:[0]}, image part+EMU);
  verify.sh **FULL exit 0** 16 gates (`/tmp/verify-ta21.log`); clippy bắt 2
  single-match thật (Empty arm 1-pattern → guard pattern).
- **Quan sát (không sửa — ngoài scope):** test tích thể `sidecar
  convert_shapes_through_the_seam` FLAKY khi chạy `cargo test --workspace` song song
  — x2t thật exit 88 "no Editor.bin" 1/2 lần, chạy tuần tự/`--test-threads=1` +
  retry parallel đều pass. Ghi nhận cho session sau nếu recur.
- **Trạng thái:** T-A21 ✅ sink Track A. ▶ = T-A22 (S2 write: model → docx round-trip,
  file ghi ra mở trong engine render GREEN pattern gate 9).

---

## 2026-10-02 · T-A20: mở màn A2 — design-first chốt P2 "OOXML core trước" + crate slice nanooffice-ooxml — GREEN

- **Phạm vi:** design-first theo handoff `ta20.md` (ponytail ULTRA — user chỉ thị): probe 2
  phương án bằng số liệu → DD mã K01 (`docs/design/converter-a2/DD.md`) → crate slice tối
  giản; KHÔNG implement converter đầy đủ. Clean-room tối đa: Editor.bin chỉ qua oracle-obs
  (chạy x2t bundled), OOXML qua spec-ref ECMA-376.
- **P1 probe (oracle-obs — `engines/converter/aarch64-apple-darwin/x2t fixture.docx out.bin`,
  fixture `make_docx.py` 1962 B):** Editor.bin 1586 B, exit 0; 2 lần convert **byte-identical**
  (differential probing khả thi). Header 35 B: magic text `DOCY;v10;0;` (11 B) + nhị phân —
  3 giá trị u32 LE tại offset 0x0d/0x17/0x21 = 0x28b/0x5bf/0x62e khớp ranh giới vùng data
  (0x28c–0x62a) — nghi bảng (offset,size) section, **semantics chưa xác định**; 42 B đầu
  payload chưa decode sau 1 probe phiên. Payload 926 B (0x28c–0x62a) **KHÔNG nén** (0 zlib
  stream), record stream `[u8 tag][u32 LE byte-len][value]`; 2 chuỗi lồng verify byte-exact
  (46→51→56→61 và 204→209→214→219→224 — mỗi mức +5 B = tag+len); chuỗi UTF-16LE inline
  (u32 len + bytes, ~20 chuỗi/5 đoạn) gồm styleId `Heading1` VÀ style name `heading 1` →
  **converter resolve style TRƯỚC khi serialize**. Cộng gỡ: delta `changes0.json` = format
  nội bộ thứ 2 (T-010: run-length, DoctRenderer áp) → P1 save phải re-implement cả
  delta-application. Kết luận: round-trip docx cơ bản = ước lượng **tháng** (từ điển tag
  hàng trăm nghĩa × ≥1 chu kỳ probe khác biệt mỗi nghĩa) + writer phải được parser engine
  chấp nhận (tag thiếu = render hỏng im lặng) + **dead code sau A4** (Nano Light làm OOXML
  trực tiếp; advanced mode dùng x2t thật — P1 hết consumer).
- **P2 probe (spec-ref ECMA-376 Part 1 §17 WordprocessingML / §20 DrawingML, Part 2 OPC):**
  fixture = 5 parts OPC, 2.5 KB XML, **21 element types + 15 attribute types**; docx cơ bản
  (heading/bold/italic/table/list/image/hyperlink/hdr-ftr/section) ước **~85 element types**
  trên ~7 parts, 4 namespace; **0% ngoài spec** → ước lượng **tuần** cho read+write cơ bản.
- **Quyết định: P2** (không 50/50): P1 chỉ rút 113 MB x2t trong khi sdkjs+web-apps 313 MB
  vẫn ship (engine vẫn cần cho edit); P2 = nền bắt buộc A3 viewer + A4 editor + tiện ích
  ngay (metadata/text-extract cho AI QĐ3). P1 giữ làm phương án B, trigger: rút x2t trước
  A4 → khi đó chỉ cần writer OOXML→Editor.bin (DD K01 §2).
- **Crate slice `src-tauri/crates/nanooffice-ooxml`** (lib: lib.rs model + opc.rs zip-part +
  document.rs XML→model; main.rs CLI in JSON): model `Document/Paragraph{style,runs}/
  Run{text,bold,italic}`; root-check `xmlns:w` binding → refuse-unknown thay parse-sai;
  OnOff `w:val` semantics (false/0/off tắt); rPr của pPr không áp lên runs; mọi lỗi zip/
  utf-8/xml → `Err(ParseError)` không panic (trust boundary). **3 test** (fixture chuẩn
  tiếng Việt exact; malformed zip/xml/part → Err; pStyle/w:val=false/w:i/entities).
  Deps mới: `quick-xml` (MIT OR Apache-2.0) + `zip` deflate-only (MIT) — stdlib không có
  XML/zip, tự viết vi phạm rule trust-boundary; `serde_json` đã có trong graph. Justification
  trong commit `a2(crate)`.
- **Wiring (bẫy mới ghi lại):** manifest src-tauri giờ là package + workspace root
  (`[workspace] members = ["crates/nanooffice-ooxml"]` — 1 member chỉ để gates phủ crate).
  Cargo ở manifest kiểu này mặc định **package-mode** → verify.sh phải thêm `--workspace`
  (check/clippy) / `--all` (fmt) vào 3 lệnh cargo, không có flag = crate không được phủ
  (im lặng!). `cargo test` vẫn chạy thủ công (`-p nanooffice-ooxml`) theo convention.
- **Bằng chứng:** `cargo test -p nanooffice-ooxml` **3 passed**; tổng cargo test 23 shell
  (+1 ignored) + 3 crate; `cargo clippy --workspace -D warnings` exit 0 (bắt 2
  useless-conversion thật ngay lần đầu); `cargo fmt --check --all` clean; verify.sh FULL
  exit 0 (`/tmp/verify-ta20.log`); CLI smoke: `target/debug/nanooffice-ooxml fixture.docx`
  in JSON đúng (tiếng Việt nguyên vẹn) exit 0.
- **Trạng thái:** T-A20 ✅ sink Track A. Seed T-A21 (S1 read) → T-A22 (S2 write) → T-A23
  (S3 parity, x2t = DEV-ORACLE) → T-A24 (S4 adapter seam); ▶ = T-A21.

---

## 2026-10-02 · T-053 + T-054 + T-044: smoke-lib 1 nhà · font-root 1 chủ · gate bridge-surface — GREEN (review #2 hygiene C3+C5+C4, 3 commit nhỏ theo thứ tự user duyệt)

- **Phạm vi:** subagent 2 chuỗi 3 task review #2 (T-053→T-054→T-044); nguyên tắc
  replace-don't-layer — gate/script rewrite phải PASSED exit 0 độc lập TRƯỚC khi
  thay bản cũ; log contract gates 9/10/11 giữ nguyên BYTE. Không đụng sidecar/
  formats/save journal/delivery.js (vừa landed T-A14/T-052).
- **T-053 (8be3224):** `harness/scripts/smoke-lib.sh` (170 dòng, SOURCE — guard
  cuối file chống execute; chống double-trap qua `_SMOKE_TRAP_SET`, mỗi script
  đúng 1 trap EXIT; style thống nhất env + mảng): `smoke_init` (pnpm/port-1420/
  fixture-gen + pre-stat) · `smoke_boot ENV… LOG` (set -m process-group +
  teardown) · `smoke_poll TIMEOUT PREDICATE [what]` (kill -0 + sleep 2 +
  SMOKE_ABORT fail-fast + SMOKE_SUMMARY mỗi vòng) · `smoke_save_evidence` (cặp
  grep save lines + unzip grep -cF + stat) · `smoke_fail_tail` (FAILED + refs
  app khi APP_HITS>0 + tail -40). LOG CONTRACT đóng băng thành hằng đầu lib
  (SMOKE_RE_* + SMOKE_APP_PATH, cross-ref 2 chiều emitter pointer comments giữ
  nguyên). 3 gate thành consumer mỏng: render 82→61 · save 93→69 · clean 107→88
  dòng; verify.sh KHÔNG đổi cấu trúc gates (đường dẫn giữ nguyên).
  **Bằng chứng:** render docx standalone PASSED exit 0 · save standalone
  PASSED exit 0 (1962→25569 B, marker ×1) · clean-machine standalone PASSED
  exit 0 · **negative** SMOKE_CLEAN_FORCE_APP_X2T=1 → exit 1 (app-path-hits=1,
  fail-fast bắt đúng dòng override `[x2t] using NANO_OFFICE_X2T`) · verify.sh
  FULL 15 gates exit 0 (`/tmp/verify-t053.log`).
- **T-054 (c797293):** font-root bảng + CẢ HAI flavor AllFonts.js về 1 nhà
  `protocol/allfonts` (producer): DI CHUYỂN từ mod.rs — `FONT_URL_ROOTS` +
  `font_root_disk` + `engines_root` (discovery chung) + `encode_uri_component`
  (1 encoder cạnh URL producer, hết nguy cơ drift) + `rewrite_font_paths`
  (PORT làm tham số — producer không phụ thuộc server) + composition
  `engine_flavor_js(port)` (thay generate_all_fonts_js). mod.rs + filesrv thành
  consumer (`allfonts::`), **hết `super::` ngược dòng từ allfonts** (0 match).
  Quick win: filesrv.rs:181-199 range-parse tự viết (0 test + latent bug 0-byte
  body: `total - 1` underflow) → XOÁ, gọi `mod.rs::parse_range_header`
  (refactor parse_range nhận `Option<&str>` — bản có test; RangeSpec pub(crate)).
  Test: +1 `bundled_fonts_enumerate_via_interface` qua interface `generated()`
  (structure js_raw + app-root contribution + Carlito family + URL flavor;
  skip-friendly khi engines thiếu — giới hạn sys-machine-dependent ghi comment);
  2 test rewrite/app-root dời theo bảng. **Bằng chứng:** cargo test **23 passed
  / 0 failed** (+1 ignored cũ) · clippy -D warnings exit 0 · fmt clean.
  Ghi nhận ngoài scope: filesrv percent_decode private vẫn trùng mod.rs (không
  đụng — không nằm trong task).
- **T-044 (d1d04fc):** gate `harness/scripts/check-bridge-surface.mjs` (node
  stdlib, hạ còn check-script — KHÔNG sinh code): đối chiếu (a) KNOWN_METHODS +
  LOCAL_TRUTHS (desktopEditor.js — const-block match, **strip comment trước khi
  tách chuỗi**: apostrophe trong comment "ack'd" từng lệch cặp quote nuốt ~50
  method) ↔ (b) bridge/mod.rs match arms (dòng bắt đầu `"X"`/`| "X"` trước
  `_ =>`, chỉ quét TRƯỚC `=>` — không lầm `env!("CARGO_PKG_VERSION")`) ↔
  (c) reference.map (implemented ⊆ doc-token normalize+expand; bảng PH-0 cột
  Method ⊆ surface shim) ↔ (d) globals.d.ts sync-heuristic (LOCAL_TRUTHS key
  trên dòng Promise → flag). execCommand allowlist có chủ đích. Wire verify.sh
  sau check-docs. **Gate bắt được 2 mismatch THẬT ngay lần chạy đầu** (giá trị
  của gate — sửa đúng chỗ, không nể): (1) bảng PH-0 hàng Property dùng
  shorthand `Property_Get/SetCryptoMode` máy không đọc ra tên chính xác → tách
  2 tên exact; (2) globals.d.ts khai báo AscDesktopEditor + execCommand trả
  **Promise-only** trong khi LOCAL_TRUTHS trả SYNC (CEF shell cũng sync —
  call-sites dùng trong `if`) → đổi trả về `unknown` (pnpm build exit 0).
  **Bằng chứng:** gate PASS exit 0 (KNOWN=89 · LOCAL_TRUTHS=28 · dispatch=30 ·
  PH0-doc=31 · sync-ok) · negative: tiêm arm `BogusT044Method` vào dispatch →
  exit 1 (2 mismatch đúng loại) → phục hồi → exit 0.
- **Trạng thái:** T-053 ✅ (sink PH-1) · T-054 ✅ + T-044 ✅ (sink META). verify.sh
  giờ 16 gates (thêm check-bridge-surface). Chuỗi review #2 hết subagent 2;
  kế tiếp **T-A20** (A2 converter Rust) hoặc PH-1 còn lại (T-046/T-047/T-049 —
  T-049 giờ có gate-lib để clone theo format).

---

## 2026-10-02 · T-A14 + T-052: converter seam + formats/workdir 1 nhà — GREEN (refactor thuần, hành vi KHÔNG đổi)

- **Phạm vi (review #2 C1+C2, 1 subagent theo thứ tự user duyệt):** T-A14 seam converter
  trước T-A20; T-052 formats + workdir 1 nhà kèm ngay sau. Pure refactor — mọi hành vi
  hôm nay giữ nguyên; bằng chứng = `cargo test` + verify.sh FULL.
- **T-A14 (2d3da4e):** module `src-tauri/src/sidecar/mod.rs` THẬT (thay stub PH-1; xoá
  `window/mod.rs` kèm — T-043 hấp thụ, 0 dòng code):
  (1) `x2t_path`/`find_bundled_x2t`/`target_dir_name`/`no_app_fallback` DI CHUYỂN nguyên
  khối từ save.rs — log strings nguyên văn ("[x2t] using bundled converter:", override,
  fallback WARN, "clean-machine mode" — gate 10/11 grep targets); consumer ngoài
  bundle-x2t.sh/smoke-clean-machine.sh chỉ sửa comment path.
  (2) interface `convert(req)→Result<ConvertResult,ConvertError>` che 3 hình dạng gọi:
  `OpenBinary{src,dst}` (positional 2-arg, success=dst tồn tại — x2t non-zero vẫn ghi
  output hợp lệ) · `SaveFromChanges{params_xml,output}` (1-arg, success=exit 0 + output
  non-empty — partial-conversion không bao giờ swap qua file gốc) · `Passthrough{src,dst}`
  (pdf-copy, không spawn). 3 call-site cũ (lib.rs open + pdf, save.rs spawn) đổi sang
  seam — spawn/output-capture 1 chỗ; lib.rs hết `bridge::save::x2t_path()` (hết locality
  đảo ngược). KHÔNG thêm timeout (giữ `.output()` blocking như hôm nay — kill sẽ đáp 1
  chỗ trong sidecar khi có bằng chứng hang).
  (3) Test pin: resolution order env-if-exists → bundled walk-up → strict-error/app-
  fallback qua `x2t_path_in(tmp roots)` + 3 shape qua interface với fake converter (env
  branch 1 — deterministic, KHÔNG đụng /Applications thật; passthrough proven no-spawn:
  env trỏ converter failing vẫn copy Ok).
- **T-052 (c5d8771):** module `src-tauri/src/formats.rs`: bảng `FORMATS` (ext ↔ doctype
  ↔ extra boot params ↔ OpenFlavor ↔ SaveCode ↔ inner_xml_path) + provenance từng hằng:
  docx=65 Wired (save contract), xlsx=257 **KnownUnwired** (oracle 29.09), pptx=
  **Unidentified** + comment bẫy "513=export-PDF KHÔNG phải pptx" (T-049 đừng nhầm),
  pdf=CopyOriginal; `m_nCsvTxtEncoding=46`/`m_nCsvDelimiter=4`/`m_nDoctParams=1` về 1 nhà
  với provenance TBD (T-049) — value observed T-010. Struct `DocWorkdir` (editor_bin/
  changes_dir+changes_file/params_xml/delivered_name) thay 3 module tự biết layout —
  folder string `nanooffice-doc-<id>` đóng băng (delivery.js consume, test pin). Save
  gate đọc bảng (`wired_save_code`) — hôm nay vẫn CHỈ docx (xlsx/pptx từ chối y hệt
  trước). Đối chiếu main.ts FILE_FORMATS: **test Rust** đọc main.ts ở RUNTIME (không
  include_str! — bẫy stale-rebuild ngoài crate root; lý do "ít moving parts" hơn script
  verify.sh: không gate mới, chạy cùng cargo test) assert đếm entry + filetype/doctype/
  isForm khớp từng dòng.
- **Bằng chứng (confirmed):** `cargo test` **22 passed / 0 failed** (18 sau T-A14: +3
  sidecar; +4 formats sau T-052); `cargo clippy -D warnings` exit 0; fmt clean; **verify.sh
  FULL 15 gates exit 0** (`/tmp/verify-ta14-t052.log`: cargo/fmt/clippy/pnpm/licenses/
  docs + smoke render ×4 + save + clean-machine — gate 10/11 grep các log line di chuyển
  vẫn bắt được = log contract nguyên vẹn).
- **Trạng thái:** T-A14 ✅ (2d3da4e) sink Track A · T-052 ✅ sink PH-1. Slot adapter T-A20
  sẵn sàng (A2crate cắm sau `convert`); T-049 giờ có bảng 1 nhà (khỏi gõ lại 257/513 từ
  prose). Kế tiếp: T-053 (subagent 2: T-053→T-054→T-044).

---

## 2026-10-02 · Architecture review #2 (post-A1) — 6 candidates landed thành TODO, user chỉ thị "làm hết" + ponytail sau

- **Phạm vi:** 38 commits/12 ngày từ review #1 (27.09); first-party ~2.0k → ~4.6k dòng.
  Sweep: agent Explore toàn hot set (save.rs 490 · allfonts.rs 1072 · delivery/desktopEditor
  ~940 · 7 harness scripts) + main-agent kiểm chứng first-hand 3 claim load-bearing
  (đều đúng): open chain mượn `bridge::save::x2t_path` (locality đảo ngược, lib.rs:158);
  filesrv.rs:181-199 range-parse tự viết trong khi `mod.rs::parse_range` CÓ TEST vẫn còn
  (parser không test phục vụ production); 3 smoke gates copy boilerplate gần nguyên văn
  (boot/teardown/poll ×3 đã lệch style env-inline vs mảng CMD).
- **6 candidates → TODO:** C1 T-A14 converter seam `sidecar::convert(req)→result` che 3
  hình dạng gọi (open positional / save params-XML / pdf passthrough) — hấp thụ T-043,
  làm TRƯỚC T-A20 (A2crate = adapter 2) · C2 T-052 formats + workdir 1 nhà (chặn T-049
  gõ lại 257/513 từ prose) · C3 T-053 smoke-lib factor trước khi T-049 clone gates ·
  C4 T-044 hạ còn check-script (~35 method ×3 nơi — tệ hơn review #1) · C5 T-054 font-root
  1 chủ + filesrv range-reuse · C6 quick wins (provenance constants, log pointer 1 chiều,
  env hấp thụ vào C1/C2). T-046 thu hẹp phần còn lại.
- **Thứ tự thực hiện (user "làm hết"):** 1 subagent làm T-A14→T-052 (cùng file, kèm
  nhau) → subagent 2 làm T-053→T-054→T-044 → ponytail review toàn source → độc lập
  verify sau mỗi subagent (mỗi gate rewrite phải PASSED exit 0 trước khi xóa bản cũ —
  replace-don't-layer; log contract gates 9/10/11 giữ nguyên byte).
- **Trạng thái:** landed TODO (dashboard Track A 6/1🟨, PH-1 10, META 12); bắt đầu T-A14.

---

## 2026-10-02 · T-A13: Gate 11 "sạch ONLYOFFICE" — ĐÓNG RUNG A1 (tự chủ phân phối) — GREEN (verify.sh FULL exit 0 với app ẨN + negative proven)

- **Kết quả (confirmed) — 3 chiều bằng chứng:**
  (1) *Gate 11 positive:* `harness/scripts/smoke-clean-machine.sh` **exit 0** — boot app strict
  mode, save round-trip marker ×1 (1962→25570 B), `grep -cF "/Applications/ONLYOFFICE"` toàn
  log = **0**, 0 dòng WARN fallback, strict mode không trip (`/tmp/nanooffice-smoke-clean-gFtBod`).
  (2) *Negative proven:* `SMOKE_CLEAN_FORCE_APP_X2T=1` (point `NANO_OFFICE_X2T` vào
  `/Applications/ONLYOFFICE.app/…/converter/x2t`) → gate **FAIL exit 1**, `app-path-hits=1` —
  dòng `[x2t] using NANO_OFFICE_X2T override: /Applications/ONLYOFFICE.app/…` (log line 76)
  bị grep bắt, fail-fast ngay khi override hiện mặt dù run "chạy được" — gate biết bắt vi phạm
  (`/tmp/nanooffice-smoke-clean-XbynTY`).
  (3) *Test tích thể (bằng chứng đóng rung):* `harness/scripts/clean-machine-proof.sh` —
  preconditions `pgrep -x ONLYOFFICE` trống → `mv /Applications/ONLYOFFICE.app
  /Applications/ONLYOFFICE.app.ta13-hidden` (same volume, atomic; KHÔNG rm; trap
  EXIT/INT/TERM luôn restore) → `verify.sh` **FULL exit 0 khi app không tồn tại**
  (`/tmp/verify-ta13-clean-machine.log`: cargo/fmt/clippy/pnpm/licenses/docs + 6 smoke gates
  PASSED, gate 11 trong đó dùng bundled converter ×2 — open + save — `save delivered: …ta13.docx
  (25569 bytes) in 526ms`) → restore xong `ls` xác nhận `/Applications/ONLYOFFICE.app` nguyên
  trạng, 0 process sót.
- **Code chạm:** `src-tauri/src/bridge/save.rs::x2t_path()` — env
  `NANO_OFFICE_NO_APP_FALLBACK` (set ≠ "0"): nhánh 3 (fallback app cài sẵn) bị TẮT, thay WARN
  bằng `log::error!` "clean-machine mode" + return rỗng → spawn fail rõ ràng (save →
  EndSave(2); open → lỗi delivery) thay vì lặng lẽ dùng app; nhánh 1 (override
  `NANO_OFFICE_X2T`) giờ **log info path** (ADDITIVE — chính cơ chế giúp gate bắt override
  trỏ nhầm vào app). Log contract gate 9/10 nguyên vẹn.
- **Gate 11 (thường trụ verify.sh, sau gate 10):** assertions = save evidence như gate 10
  (log line + marker trong word/document.xml + fixture mutated) + `grep -cF
  "/Applications/ONLYOFFICE"` toàn log == 0 + 0 dòng "falling back to the installed
  ONLYOFFICE" + 0 dòng "clean-machine mode" (strict phải không trip trên máy khoẻ).
  `SMOKE_CLEAN_FORCE_APP_X2T=1` = chế độ negative-proof manual (không chạy trong verify).
- **Bài test mv-app re-runnable:** `clean-machine-proof.sh` committed — exit 3 khi app không
  tồn tại hoặc mv không khả thi (SIP/perm) → phương án B (gate strict + bảng đối chiếu
  runtime paths, ghi rõ giới hạn bằng chứng); exit 4 = trạng thái lạ cần can thiệp tay.
  Hôm nay không cần phương án B — mv thành công ngay lần đầu.
- **Runtime dependencies ONLYOFFICE còn lại: 0 bắt buộc** (fallback x2t = WARN dev-only, bị
  strict mode chặn; hằng số format/khác = T-046; upstream submodule = reference theo D1 giữ
  tới A6).
- **Trạng thái:** T-A13 ✅ — **RUNG A1 ĐÓNG** (A0+A1; mốc: sản phẩm phân phối độc lập đầu
  tiên). Seed **T-A20** mở màn A2 (converter Rust DOCX — clean-room ECMA-376, chỉ spec-ref +
  oracle-obs; đích: parity round-trip vs x2t trên corpus + fuzz). KHÔNG implement A2 trong
  phiên này.

---

## 2026-10-02 · T-A12: font bundle vào engines/fonts — tham chiếu runtime cuối cùng tới ONLYOFFICE.app rút xong — GREEN (verify.sh exit 0, 5 gates)

- **Kết quả (confirmed):** verify.sh full **exit 0** (`/tmp/verify-ta12.log`) — 4 render gates +
  save gate chạy với root `app` = `engines/fonts/`. Log mới ADDITIVE `font serve: /fonts/app/…`
  chỉ rõ nguồn: xlsx + pptx fetch Carlito ×4 từ `…/engines/fonts/crosextra/Carlito-*.ttf`
  (`/tmp/nanooffice-smoke-8wt8Hj`, `…-NvAmKK`); docx/pdf/save chỉ dùng root sys như trước.
  0 dòng `font serve … ONLYOFFICE` trong mọi log. Save round-trip marker ×1
  (`…-save-0uOyXh`, 1962→25570 B); AllFonts.js ghi cho x2t: 355 path — **0 path
  ONLYOFFICE.app, 15 path engines/fonts** (2 consumer khớp by construction như T-A11).
- **Discovery font cần bundle (oracle-obs, 02.10):** gom `GET /fonts/app/…` từ 44 smoke log
  còn sống (29.09–02.10, đủ 4 format + save): duy nhất `crosextra/Carlito-{Regular,Italic,
  Bold,BoldItalic}.ttf` (xlsx + pptx — theme default Calibri → substitute Carlito);
  docx/pdf fixture fetch 0 font app (fixture dùng font hệ thống qua root sys). ASC.ttf
  cũng 0 fetch — consumer thay family "ASCW3" bằng symbol font inline base64
  (reference.map § AllFonts generator).
- **Bundle decision:** bundle **toàn bộ root font app** (15 file + 5 license text =
  4.9 MB du) TRỪ ASC.ttf — không phải chỉ Carlito fixture-fetch: giữ bộ substitute family
  của app cho tài liệu thật trên máy sạch (Cambria→Caladea, bullet→OpenSymbol,
  equation→Asana Math, UI→OpenSans). ASC.ttf KHÔNG bundle: license không rõ (name table
  không có manufacturer/license fields, app không kèm LICENSE riêng) + 0 fetch — S6
  "không rõ license thì không bundle". Script: `harness/scripts/bundle-fonts.sh`
  (manifest pinned 20 file, lệch → thoát lỗi; nguồn copy = app cài sẵn, handoff cho phép
  — font giữ license riêng, không phải AGPL).
- **License gate S6 từng set (name table + LICENSE.txt kèm):** OpenSans ×5 = Apache 2.0
  (manufacturer Ascender Corporation, licenseURL apache.org/LICENSE-2.0, LICENSE.txt root);
  Carlito ×4 = SIL OFL 1.1 (tyPoland Lukasz Dziedzic, scripts.sil.org/OFL); Caladea ×4 =
  SIL OFL 1.1 (Huerta Tipográfica; name table còn licenseURL Apache 2.0 — dual, bundle
  theo LICENSE.txt OFL kèm); asana/ASANA.ttc = SIL OFL 1.1; openoffice/opens___.ttf
  (OpenSymbol) = SIL OFL 1.0. Mỗi set copy kèm LICENSE.txt verbatim. Bảng đầy đủ:
  reference.map § Font bundle.
- **Code chạm:** `protocol/mod.rs` — `FONT_ROOTS` const (2 cặp đĩa/URL) → `FONT_URL_ROOTS`
  ["app","sys"] + `font_root_disk()` (app = `engines/fonts/` qua `engines_root()`, sys =
  `/System/Library/Fonts`); `rewrite_font_paths` theo pair động (needle thêm trailing
  slash — không match sibling dir); `handle_engine` font branch qua resolver. `allfonts.rs`
  — enumerate qua roots động + WARN riêng khi thiếu `engines/fonts` ("run
  bundle-fonts.sh", mirror triết lý WARN x2t). `filesrv.rs` — `resolve_font_path` qua
  resolver + 1 dòng log ADDITIVE `font serve:` (log contract gate 9/10 không đổi).
  2 test mới: rewrite giữ namespace `/fonts/app/` + assert root app không chứa
  "ONLYOFFICE". Enumeration mới: **355 files / 557 faces / 387 families / 4370 ranges**
  — đúng bộ số của app sinh (T-A11 gen: 356/558/388; -1 mỗi loại = ASC.ttf bỏ) —
  deviation mới: family "ASCW3" rời AllFonts (consumer tự thay inline, behavior-tolerant).
- **Trim best-effort (phụ, không chặn):** `engines/editors/sdkjs-plugins` = **38 MB**
  (sdkjs 161 + web-apps 114 + plugins 38 = editors 313; + converter 113 + fonts 4.9
  ≈ engines 431 MB). 0 request HTTP tới `/editors/sdkjs-plugins/…` trong MỌI log gate
  (4 format + save; serviceworker tham chiếu prefix `sdkjs-plugins/` cũng 0 lần được
  fetch) → **loại khỏi bản copy engines/editors sau** (ghi `engines/README.md` — local,
  uncommitted; kiến thức committed ở reference.map), KHÔNG xoá local. Per-app-type build
  không làm (PH-4). Budget ≤100 MB: soft target đã user phê 30.09 — không re-baseline thêm.
- **Trạng thái:** T-A12 ✅ — đủ điều kiện T-A13 (gate 11 "sạch ONLYOFFICE" — máy không
  cài app chạy trọn open→edit→save). Tham chiếu runtime tới ONLYOFFICE.app còn lại:
  (1) fallback path x2t + hằng số khác (T-046), (2) upstream submodule (D1 — giữ tới A6).
  Font pipeline: sạch.

---

## 2026-09-30 · T-A11: tự sinh AllFonts.js bằng Rust — clean-room ĐẦU TIÊN của Track A — GREEN (verify.sh exit 0 + counterfactual data-dir)

- **Kết quả (confirmed):** verify.sh full **exit 0** (5 render gates + save gate) với AllFonts.js do
  generator Rust sinh. Log save run (`/tmp/nanooffice-smoke-save-FCGB40`):
  `[x2t] using bundled converter: …/engines/converter/aarch64-apple-darwin/x2t` +
  `AllFonts: generated 356 files / 558 faces / 388 families / 4370 ranges → …/T/nanooffice-allfonts/AllFonts.js`;
  params XML thực tế của x2t: `<m_sAllFontsPath>…/T/nanooffice-allfonts/AllFonts.js</m_sAllFontsPath>`
  + `<m_sFontDir>…/nanooffice-allfonts</m_sFontDir>` (workdir `$TMPDIR/nanooffice-doc-1/params_to.xml`).
  Save round-trip marker ×1 → changes áp dụng thật bằng file tự sinh.
- **Counterfactual data-dir (confirmed):** `mv "$HOME/Library/Application Support/asc.onlyoffice.ONLYOFFICE/data/fonts"`
  → `.ta11-counterfactual` (AllFonts.js của app KHÔNG tồn tại) → `smoke-render.sh` **PASSED exit 0**
  (`/tmp/nanooffice-smoke-ZDwkP8`) + `smoke-save.sh` **PASSED exit 0** marker ×1 (`…-XTYDQx`,
  1962→25569 B) → khôi phục nguyên trạng. Runtime không còn đọc data-dir app cho font index.
- **Khảo artifact (oracle-obs, trước khi code):** AllFonts.js của app = 5 global:
  `__all_fonts_js_version__=2` · `__fonts_files` (355 path) · `__fonts_infos` (387 dòng
  `[family, rFile, rFace, iFile, iFace, bFile, bFace, biFile, biFace]` — field cặp = FILE index
  + FACE index trong file TTC, KHÔNG phải style code; xác nhận qua `Externals.js:264 CFontInfo`
  + `fontManager.LoadFont(fontfile, info.faceIndex, …)`) · `__fonts_ranges` (4686 triplet
  `start,end,infoIdx`, index vào RAW infos — consumer `character.js:74`, optional: `if (!fonts) return`)
  · `g_fonts_selection_bin` (base64; **decode sạch 91283/91283 byte, 557 record**). Schema record v2:
  `[recordLen (tính cả 4 byte của chính nó), nameLen+name(UTF-8), altCount+alts, pathLen+path,
  faceIdx, italic, bold, fixed, panoseLen(10)+panose, ur1..4, cp1..2, weight, width, famClass,
  fmt, avg, asc, desc, gap, xh, cap, fsType]` — i32/i16 LE. Đối chéo: mọi record.name ∈ infos,
  mọi record.path ∈ files, `utype == fsType` 551/551, metrics = OS/2 scale ×1000/upm trunc
  (TNR: typo 1420/2048→693, −442→−215, gap 307→149, sxHeight 916→447, sCapHeight 1356→662,
  xAvgCharWidth 821→400; offset sxHeight@86, sCapHeight@88 — v2+ = 96 byte, KHÔNG phải @84 như ghi nhầm spec).
- **Rules sinh xác nhận từ data (không đọc source generator — core C++ chưa clone):**
  (1) primary family = nameID1: pid3-e1-l0x409 → pid1-e0 → pid3-e1 → first (**557/557**);
  (2) alt name = nameID1 pid3-e1 đầu tiên khác primary (**555/557** — lệch 2 face .SF NS);
  (3) slot R/I/B/BI = fsSelection bits 0x20/0x01; thiếu OS/2 → head.macStyle 0x01/0x02
  (Courier.ttc: 4 face macStyle 0-3 → đủ 4 slot, mọi metric = 0, weight = 0);
  (4) primary bắt đầu `.` → skip (hidden — `.Apple SD Gothic NeoI`);
  (5) file chỉ vào list khi ≥1 face thắng slot; (6) winner per (family, slot) = |weight−400| min.
- **So sánh font cũ (app) vs mới (generator):** 528/528 face GIAO TẬP khớp **100% mọi field**
  (name, alt, it, bo, weight, format); files 355→356, faces 557→558, families 387→388.
  Deviations ghi nhận: (a) tie-break lệch 27/387 family — app chọn face cùng-slot KHÔNG theo
  weight (nghi CoreText weight-trait float; VD `Sukhumvit Set` R = 'Semi Bold' w600 thay vì
  'Text' w400) — của tôi chọn face gần 400 nhất (đúng typographic hơn), behavior-tolerant vì
  slot vẫn trỏ face hợp lệ của family; (b) alt 2 face `.SF NS`; (c) superset +1: Apple Color
  Emoji (bitmap-only, app skip); (d) `__fonts_ranges` tự sinh từ cmap f4/f12 của font mình
  (widest-span-wins; 4370 vs 4686 triplet — mechanism tương đương); (e) 31 file đĩa không có
  trong artifact app (enumerate 21.09 + skip rule riêng của app — không replica được, superset vô hại).
- **Index filesrv vs x2t (câu hỏi handoff):** 2 consumer không cần khớp thứ tự với app — cần
  khớp NHAU. Resolve **by construction**: filesrv serve `rewrite_font_paths(js_raw)` (URL
  loopback), save.rs nhận chính `js_raw` (disk path) từ CÙNG chuỗi nguồn → index trùng khớp.
  Behavioral proof: gate 10 marker ×1 bằng file tự sinh (x2t parse được — kể cả selection bin).
- **Code chạm:** `src-tauri/src/protocol/allfonts.rs` (MỚI — generator 1 nhà: walk FONT_ROOTS,
  parse sfnt std-thuần name/OS2/head/post/cmap bằng seek-read từng table (không đọc nguyên
  file — TTC lớn), OnceLock 1 lần/run, `ensure_written()` ghi 1 file temp; 8 unit test gồm
  synthetic sfnt in-memory + selection-bin round-trip + base64 RFC vectors) ·
  `protocol/mod.rs` (`all_fonts_source()` XÓA — hết đọc data-dir app; `generate_all_fonts_js()`
  từ generator + `rewrite_font_paths` giữ nguyên) · `bridge/save.rs` (`m_sAllFontsPath`/
  `m_sFontDir` trỏ `ensure_written()`). KHÔNG thêm crate (base64 + UTF-16BE tự viết;
  Mac-Roman high-byte decode Latin-1 — deviation nhỏ, tên primary thực tế đều ASCII).
  Log contract gate 9/10 không đổi.
- **Trạng thái:** T-A11 ✅; kế tiếp T-A12 (font subset + trim ≤100 MB) rồi T-A13 (gate "sạch
  ONLYOFFICE" trên máy không cài app). Tham chiếu runtime tới ONLYOFFICE.app còn lại: (1) FILE
  fonts trong `login/fonts` (T-A12), (2) fallback path x2t + hằng số khác (T-046), (3) upstream
  submodule (D1 — giữ tới A6).

---

## 2026-09-30 · User quyết budget A1: chấp nhận phình size — tự chủ ưu tiên, tối ưu sau

- **Quyết định (user, 30.09):** sau khi T-A10 ghi thẳng x2t subset 113 MB (17 framework
  strong-link, không cắt thêm được), user chốt: **quan trọng nhất là tính độc lập và tự chủ
  của dự án; phình size chấp nhận; tối ưu tính sau.**
- **Land:** vision QĐ2 — "Tổng payload có engine" chuyển **soft target** (≤100 MB giữ làm
  hướng tối ưu, không phải gate; chốt lại số khi PH-4) · vision QĐ9 rung A1 — điều kiện
  XONG bỏ "payload ≤100 MB", giữ điều kiện tự chủ (máy sạch chạy trọn open→edit→save) ·
  TODO T-A12 — trim/font subset = best-effort, số liệu ghi thẳng.
- **Hệ quả:** T-A13 (gate "sạch ONLYOFFICE") không còn bị chặn bởi con số — chỉ chặn bởi
  độc lập runtime thực tế. Download-first (T-041 phần còn lại) vẫn là câu hỏi PH-4.

---

## 2026-09-30 · T-A10: bundle x2t vào engines/converter/ — GREEN, 113 MB vượt budget 100 MB (ghi thẳng)

- **Kết quả (confirmed):** mở + save DOCX chạy bằng x2t BUNDLED không cần env — gate 10
  PASSED exit 0 (`/tmp/nanooffice-smoke-save-TnT1Ki`, marker `T011-30194`, 1962→25619 B,
  marker ×1), log `[x2t] using bundled converter: …/engines/converter/aarch64-apple-darwin/x2t`
  ×2 (open convert + save). Counterfactual: `NANO_OFFICE_X2T=/nonexistent/bogus-x2t` →
  WARN "points to a missing file — ignoring" → vẫn PASSED bằng bundled
  (`-Z1vBWr`). Nhánh fallback: dời `engines/converter` tạm → gate 9 docx vẫn GREEN với
  WARN "falling back to the installed ONLYOFFICE.app" (`/tmp/nanooffice-smoke-11YLqW`).
- **Discovery framework (otool -L, 30.09):** x2t strong-link CẢ 17 framework
  (LC_LOAD_DYLIB, 0 weak); graph khép kín → không có on-demand ở mức dylib (phủ định
  hypothesis handoff). Subset bỏ được bằng chạy thật: `templates/` 207 MB + `empty/`
  2.9 MB + `dictionaries/` — **bundle = x2t + DoctRenderer.config + 17 fw = 113 MB**
  (bảng size từng phần: reference.map § Converter bundle). Vượt budget ≤100 MB của
  A1 — **không fake số**: trim thuộc T-A12 (UnicodeConverter 31 MB + x2t 42 MB là mục
  tiêu lớn); nếu subset tối tiểu mà vẫn >100 MB thì budget A1 cần RE-BASELINE ở vision
  QĐ2 — ĐỀ XUẤT để user quyết, không tự sửa.
- **Root cause save fail đầu tiên (reproduced → fixed):** bundled x2t save from-changes
  exit 86 `TypeError: window.InitNativeEditors is not a function` — DoctRenderer trong
  x2t load script theo `DoctRenderer.config` cạnh binary với relative `../editors/…`
  (`native.js` định nghĩa `InitNativeEditors`); app cài sẵn resolve → `Resources/editors`,
  engines/ thì editors build ở `engines/editors` (sibling của converter/, không phải
  `<target>/`) → bundle script tạo symlink `engines/converter/editors → ../editors`
  (guard: thiếu `engines/editors/sdkjs` → script thoát lỗi). Sau symlink: save trực tiếp
  exit 0, output 25619 B hợp lệ, marker ×1 trong `word/document.xml` (changes áp dụng
  thật, không drop).
- **Code chạm:** `harness/scripts/bundle-x2t.sh` (MỚI — script committed, nội dung
  engines/ không commit) · `src-tauri/src/bridge/save.rs` (resolution 3 nhánh: env→
  bundled→app WARN; env chỉ nhận khi file tồn tại; `target_dir_name()` compose từ
  `std::env::consts::ARCH/OS` — đồng bộ với `rustc -vV host` của script). 2 điểm dùng
  x2t (lib.rs open-delivery + save.rs LocalFileSave) đều đi qua `x2t_path()` — 1 nhà
  duy nhất, T-046 env.rs không bị mở scope.
- **Licensing:** bundle x2t = phân phối binary AGPL → khi phát hành (PH-4) theo
  `harness/rules/licensing.md` (NOTICE + source offer + logo §7); nhắc đã thêm vào TODO
  PH-4. Fonts đi kèm app KHÔNG được bundle theo (T-A11/A12 tự sinh/subset từ hệ thống).
- **Bằng chứng subset temp-dir (bước 1 discovery):** copy subset ra `/tmp/nanooffice-ta10-subset`
  (x2t + config + 17 fw, không templates/empty) → x2t trực tiếp convert fixture DOCX →
  Editor.bin exit 0 (1586 B) → save from-changes bằng params XML (copy format save.rs:
  `<m_nFormatTo>65` + fromChanges + AllFonts/font-dir từ font cache app) exit 0, output
  25619 B, marker `T011-18961` ×1 trong `word/document.xml` — TRƯỚC khi đụng app.
- **Code review 2 trục (30.09, sau commits):** không hard violation (MIT purity PASS —
  engines/ không commit, không dep mới, log contract nguyên vẹn); 1 bug thật đã fix:
  `startRenderProbes()` early-return khi timer còn chạy ⇒ EndLoad thứ 2 (reopen trong
  vòng 120s) có 0 dòng probe — đúng bệnh T-050 ở scenario multi-open → đổi thành
  restart (clear + chạy lại). Judgement calls ghi nhận: naming `<target>` 2 nhà
  (Rust/script) → chi tiết test đối chiếu đã nhét vào dòng T-046; fallback path
  app cài sẵn hardcode → T-046; env stale giờ WARN+rơi xuống bundled thay vì fail loudly
  (điều kiện cần cho counterfactual của chính T-A10).
- **Trạng thái:** T-A10 ✅; kế tiếp T-A11 (AllFonts tự sinh bằng Rust) — sau đó save
  params không còn phụ thuộc font cache của ONLYOFFICE.app nữa, T-A13 gate "sạch
  ONLYOFFICE" mới đủ điều kiện.

---

## 2026-09-30 · Review T-048 (session chính) — công việc GREEN, nhưng bắt được gate pptx FLAKY + facts x2t cho T-A10

- **Review T-048 (verified):** code diff sạch (FILE_FORMATS 1 nhà `src/main.ts:37`; PDF deliver
  raw không x2t; FONT_VARIANTS 3 build; evidence reference.map § MULTI-FORMAT). verify.sh
  chạy độc lập **exit 1 duy nhất vì gate pptx** — standalone rerun PASSED exit 0 (canvas
  490x1010) → **flake, không phải regression**.
- **Root cause flake (observed):** run fail có `contentReady=1` + `fonts=18` nhưng **0 dòng
  probe** nào (canvas probe 6s/8s/15s + font-oracle đều im) trong 3,5 phút còn lại —
  probe one-shot setTimeout không chạy, nghi WKWebView throttle timer khi cửa sổ bị
  che/lost-focus giữa chuỗi 5 boot liên tiếp của verify.sh. Gate đang trông cậy vào
  one-shot timer → thi thoảng mất tín hiệu `viewer>0`. → **T-050**: probe lặp interval
  đến height>0, format log giữ nguyên (log contract gate 9 không đổi).
- **Facts x2t cho T-A10 (observed 30.09, `ls` converter dir):** x2t KHÔNG phải binary đơn
  lẻ — Mach-O arm64 + **17 framework** (kernel, graphics, doctrenderer, UnicodeConverter,
  PdfFile, DjVuFile…) + templates + empty, tổng **323 MB**. Hệ quả: "bundle x2t" = bundle
  cả converter dir; payload ≤100 MB của A1 không thể giữ nguyên nếu bundle trọn — T-A10
  phải khám phá subset framework (`otool -L` hard-link vs dlopen on-demand), và nếu vẫn
  vượt budget thì ghi thẳng + điều chỉnh scope T-A12, không fake số.
- **Trạng thái:** T-048 giữ ✅ (4 format mở thật); T-050 sinh ra làm trước/kèm T-A10.
- **Fix landed (T-050, 30.09, commit eec769d):** canvas probe + font-oracle + canvas
  probe-2 chuyển từ one-shot 6/8/15s sang **interval lặp 2s (tối đa 60 tick ≈ 120s —
  nằm trong timeout gate 240s), dừng sớm khi canvas/viewer height>0** (`startRenderProbes`
  trong `src/shim/delivery.js`); interval bị throttle chỉ chậm lại, không bị drop như
  one-shot. Chuỗi log giữ nguyên verbatim (pattern grep gate 9 không đụng — thêm
  occurrence vô hại). Chứng minh: `smoke-render.sh` pptx **×3 liên tiếp PASSED exit 0**
  (11:24–11:25 UTC, logs `/tmp/nanooffice-smoke-WD2VtQ`/`-vdIDVm`/`-tMKgxE`, canvas
  490×1010) + docx regression PASSED (`-R8K1lL`, 2302×958).

---

## 2026-09-29 · T-048: mở XLSX/PPTX/PDF — shell wire multi-format + smoke gates 9b/9c/9d

- **Kết quả (confirmed):** mở được cả 4 định dạng từ path input + `NANO_OFFICE_OPEN`.
  Smoke gates mới trong verify.sh: xlsx/pptx/pdf đều exit 0 (3 tín hiệu gate 9 giữ
  nguyên); gate DOCX mặc định + gate 10 save round-trip vẫn GREEN sau refactor (smoke
  save 1962→25619 B, marker ×1). Negative test: fixture xlsx hỏng → `document delivery
  failed: x2t produced no Editor.bin (exit Some(88))` + gate exit 1. Bằng chứng
  multi-format: `reference.map.md` § "Desktop open contract — MULTI-FORMAT".
- **Oracle x2t (29.09):** DOCX→`DOCY;v10` · XLSX→`XLSY;v10` · PPTX→`PPTY;v10`;
  **PDF: x2t KHÔNG convert** (exit 0, không output file) — đường PDF đi qua app thật
  (CDP oracle, không keystroke): recovery folder của ONLYOFFICE.app chỉ chứa bản COPY
  file gốc (`f.pdf` 1003 B) + `changes/` + `media/`, KHÔNG Editor.bin → pdf editor nhận
  bytes GỐC qua cùng `DesktopOfflineAppDocumentEndLoad`, render GREEN (5 canvases
  2772×1364). Khớp fallback `folder+"/"+GetFileName(sFileSrc)` khi Editor.bin thiếu trong
  `client_renderer_wrapper.cpp` "onlocaldocument_loadend" (~6410-6416). lib.rs implement
  nhánh pdf đúng hành vi này.
- **Oracle CDP kỹ thuật:** frames của CEF không truy cập được từ frame cha
  (`SecurityError` cross-origin kể cả file://) — eval trong từng frame phải qua
  `Runtime.enable` + `Runtime.executionContextCreated` (mỗi frame = 1 execution context,
  evaluate với `contextId`). /json chỉ list page targets; iframe không hiện riêng.
- **Bẫy tên minified theo build (root cause tiềm tàng nếu không généralise):** font
  loader + glyph manager có tên khác nhau trong mỗi sdkjs build (word `sje.BSa`/`Wg.vue`,
  cell `mye.cZa`/`hj.RIe`, slide `K8d.VMa`/`mi.Zie` — bảng đầy đủ trong reference.map).
  Shim cũ hardcode tên word build → cell/slide sẽ treo font. Đã generalise:
  `detectFontVariant()` chọn biến thể theo build (publish qua `__nanooffice_shim`);
  `engineReadiness()` union 3 tên wasm; font-oracle probe build-aware. pdfeditor dùng
  build word nên kế thừa names word.
- **Oracle format code cho save (quan sát cho T-049, không implement):**
  `<m_nFormatTo>` 257 = XLSX (real xlsx out); 513 = EXPORT PDF từ editor binary bất kỳ
  (cả PPTY lẫn XLSY đều ra %PDF — KHÔNG phải code PPTX); PPTX chưa định danh; PPTY→PPTX
  cần font params (thiếu → `DoctRenderer error code=open`, exit 80 — cùng signature
  T-010); không truyền m_nFormatTo mà dựa extension output = hoạt động (PK hợp lệ).
  Ghi trong reference.map § multi-format; task T-049 sinh trong TODO (← T-048).
- **Design docs:** màn mới đăng ký theo QĐ6 — E01/P01 (bảng cấp mã có sẵn) + **F01**
  (chữ F mới cấp cho PDF — thay đề xuất "PD01" 2 chữ trong handoff vì rule mã là
  `[Chữ][2 số]`); DD + TEST tối giản cho cả 3; registry + docs/README index cập nhật.
- **Fixtures:** `make_xlsx.py` (inlineStr, stdlib) · `make_pptx.py` (bộ part tối giản
  master+layout+theme) · `make_pdf.py` (ISO 32000 viết tay, tiếng Việt ở Title metadata
  UTF-16BE vì font base-14 WinAnsi không encode dấu — OQ-F01-2). KHÔNG copy fixture từ
  upstream.
- **Code chạm:** `src/main.ts` (FILE_FORMATS map 1 nhà + isForm pdf) ·
  `src/shim/desktopEditor.js` (FONT_VARIANTS + detectFontVariant + patch đại số) ·
  `src/shim/delivery.js` (engineReadiness union + font-oracle build-aware; log contract
  gate 9 giữ nguyên verbatim) · `src-tauri/src/lib.rs` (nhánh pdf trong
  get_document_for_delivery) · `src-tauri/src/bridge/save.rs` (message guard trỏ T-049) ·
  `harness/scripts/smoke-render.sh` (fixture param + generator theo extension) ·
  `harness/scripts/verify.sh` (3 gates mới).
- **Bổ sung vòng review (code-review 2 trục):** (1) UNIMPL sweep — 0 UNIMPL trong cả 3
  log boot mới (`/tmp/nanooffice-smoke-9BpaBf` xlsx · `-xkpaC4` pptx · `-NgB0ST` pdf):
  bridge surface cho cell/slide/pdf boot = các method DONE-PH0/PH1 có sẵn, không cần
  method mới (reference.map không đổi STUB→DONE). (2) Screenshot DoD: một lần chụp
  `screencapture -x` bắt NHẦM màn hình user đang dùng (bài học T-003 lặp lại) — file đã
  XOÁ ngay, bỏ yêu cầu screenshot, bằng chứng render = 3 tín hiệu log của gate
  (contentReady + canvas + fonts) — khuyến cáo phiên sau: KHÔNG chụp màn hình khi user
  đang hoạt động. (3) Mở từ UI path input: form submit và `NANO_OFFICE_OPEN` đi chung
  command `open_document` + `mountEditor` (code-identical, `src/main.ts`); form listener
  không đổi từ PH-0; không có cách tự động hoá gõ path không-keystroke (ràng buộc
  T-003) — chấp nhận bằng equivalence + negative test đã đi cùng đường open_document.
  (4) verify.sh: gate fmt từng nuốt output lỗi (`>/dev/null`) khiến CHECKS FAILED im
  lặng không chỉ ra gate nào — đã bỏ redirect; rerun exit 0.
- **Font-oracle build-aware xác nhận:** boot xlsx in `font-oracle: variant=mye mgr=yes
  streams=9` (biến thể cell được detect đúng) + 8 canvas `ws-canvas` 2340×926;
  `viewer=none` ở cell app (không có `id_viewer`) → signal ② gate khớp qua nhánh
  `canvas probe` generic — thiết kế signal đã phòng đúng.

---

## 2026-09-29 · QĐ9: roadmap tự chủ mã nguồn 100% — Track A thay engine theo thang clean-room

- **Bối cảnh:** user nâng mục tiêu sản phẩm lên "tự chủ hoàn toàn mã nguồn, cạnh tranh được
  với ONLYOFFICE/MS Office" — ★ trong Horizons được promoting thành lộ trình chính.
- **Chiến lược (chốt):** strangler fig trên seam sẵn có — thay từng dependency bằng module
  clean-room phía sau contract đã reverse (bridge, params-XML x2t, Editor.bin, fonts), ship
  sản phẩm ở mọi rung A1→A6 (A1 phân phối · A2 converter Rust · A3 viewer · A4 editor
  "Nano Light" + engine AGPL làm "chế độ nâng cao" · A5 UI riêng · A6 thoát sdkjs).
- **3 quyết định con user phê 29.09:** D1 không hard-cut (engine AGPL giữ đến A6 — note trong
  TODO Track A để nhớ) · D2 TODO hai track song song (P sản phẩm + A tự chủ, dashboard theo
  dõi) · D3 rule clean-room ràng buộc mọi agent.
- **Rule mới có hiệu lực 29.09:** `harness/rules/clean-room.md` — task Track A cấm ĐỌC code
  upstream của module đang thay (siết hơn mit-purity vốn cho đọc để hiểu); nguồn vào hợp lệ
  chỉ spec mở (`spec-ref:`) + quan sát black-box (`oracle-obs:`); knowledge các phiên reverse
  22–28.09 dùng được qua `reference.map.md` (hợp đồng hành vi = bản ghi clean-room an toàn).
  mit-purity S9 sửa tương ứng; AGENTS.md identity đổi "không bao giờ port" → "cấm port,
  chỉ thay clean-room"; conventions phân biệt "Lộ trình A" (kiến trúc shell) ≠ "Track A" (tự chủ).
- **TODO:** section Track A sinh 4 task rung A1 (T-A10 bundle x2t → T-A11 AllFonts tự sinh →
  T-A12 font subset + trim ≤100 MB → T-A13 gate "sạch ONLYOFFICE"); T-040/T-041 🚫 gộp vào.
- **Trạng thái:** quyết định ghi vision QĐ9 + Horizons 2 track; thang + ước lượng effort
  (A1 1–2 tuần · A2 1–2 tháng · A3 2–4 tháng · A4 6–18 tháng subset) trong vision.

---

## 2026-09-28 · T-011: gate 10 — save round-trip tự động

- **Cái gì:** `harness/scripts/smoke-save.sh` (verify.sh gate 10, commit `7ba9150`) tự động hoá
  bằng chứng thủ công T-010 bên dưới thành gate hành vi: fixture COPY fresh trong `mktemp -d`
  (KHÔNG save đè file repo, KHÔNG reuse path — file bị mutate theo design) → boot app với
  `NANO_OFFICE_SAVE_PROBE="T011-<random>-SAVED-BY-NANOOFFICE"` → chờ log round-trip trên stdout
  tích luỹ → assert TRÊN DISK: size+mtime đổi + `unzip -p … word/document.xml | grep -c` marker ≥ 1
  (grep trực tiếp đủ — không cần strip tag, khớp T-010, đã chạy thật). PASS in 3 bằng chứng (log
  line, byte delta, grep count); FAIL/timeout → log tail 40, exit 1; full log giữ ở mktemp như gate 9.
- **Marker/override:** env `NANO_OFFICE_SAVE_PROBE` vốn CHÍNH LÀ text probe chèn (`lib.rs` inject
  verbatim thành `__NANO_OFFICE_SAVE_PROBE_TEXT__`) — override có sẵn từ T-010, không đụng code;
  gate random token theo run (`T011-$RANDOM-…`) nên các run độc lập. Knob `SMOKE_SAVE_NO_PROBE=1`
  = run counterfactual (probe tắt → gate PHẢI FAIL — negative test của chính gate).
- **Log contract gate 10:** `save-end delivered to engine` (`delivery.js` consumeSaveEnd) +
  `save delivered:` (`bridge/save.rs`, phát sau rename đè file gốc) — pointer comment 2 đầu,
  đổi 1 đầu phải đổi cùng commit (cùng cơ chế với contract frozen gate 9).
- **Verify:** standalone ×2 exit 0 (marker `T011-6027-…` / `T011-3379-…`, 1962→25618 B, grep ×1,
  `save-end delivered to engine: code=0`); negative `SMOKE_SAVE_NO_PROBE=1 SMOKE_SAVE_TIMEOUT=90`
  exit 1 đúng kỳ vọng (app mở OK, `onDocumentModifiedChanged([false])`, không save, timeout);
  full `verify.sh` 10 gates exit 0.

---

## 2026-09-28 · T-010: SAVE DOCX hoạt động end-to-end (open → edit → save → disk)

- **Kết quả (confirmed — counterfactual có thật, xem root cause):** mở fixture DOCX → probe
  chèn text `asc_AddText("T010 SAVED-BY-NANOOFFICE")` + `asc_Save()` (không keystroke — bẫy
  T-003) → `save delivered: /private/tmp/t010-fixture.docx (25613 bytes)` (1962 B ban đầu) →
  `unzip -p … word/document.xml | grep -c "T010 SAVED-BY-NANOOFFICE"` = **1** (grep trực tiếp
  chạy ngay, không cần strip tag) → mở LẠI file đã lưu: contentReady + `viewer=1151x479` +
  14 `GET /fonts/` (render GREEN). Full verify.sh **exit 0**. TC-D01-10: GREEN.
- **Hợp đồng save (word desktop 9.4.0) — chi tiết + file:line ở `reference.map.md` § "Desktop
  save contract":** engine `asc_Save` → `LocalFileSaveChanges(deltas, deleteIndex, count)`
  (deltas append vào `<work>/changes/changes0.json`, format byte `"<…>","` của shell) →
  `OnSave()` → `LocalFileSave(param, pwd, docinfo, fileType, json, oldPwd)` → shell chạy **x2t
  với params XML** (`<m_sFileFrom>=<work>/Editor.bin` + `<m_bFromChanges>` + `<m_sFileTo>`=file
  gốc, `<m_nFormatTo>`65=DOCX) → shell eval lại: `LocalFileSetSaved(true)` +
  `LocalFileSetSourcePath(path)` + `DesktopOfflineAppDocumentEndSave(0)`. UI đẩy trạng thái nút
  LÊN shell qua `execCommand('title:button', {disabled:{save:…}})` (Desktop.js:259–281) — shell
  KHÔNG phải đẩy xuống để enable Save; nút toolbar là api-driven.
- **Root cause lần đầu THẤT BẠI im lặng (confirmed, counterfactual = fix):** save #1 ra file
  8811 B KHÔNG có marker dù mọi bridge call đúng thứ tự. Điều tra: decode `changes0.json` bằng
  regex utf-16 liền mạch → kết luận sai "deltas thiếu text" (text trong delta được mã hoá
  run-length, không scan được kiểu đó). Bằng chứng quyết định: chạy x2t của ta trên recovery
  copy của app THẬT (dump qua CDP full-trace) → vẫn thiếu marker + `DoctRenderer error
  code=open`, exit 86. Fix: thêm `<m_sFontDir>` + `<m_sAllFontsPath>` (= thư mục data/fonts của
  app — cùng nguồn AllFonts.js đã dùng) vào params XML → x2t exit 0, marker xuất hiện ngay cả
  trên recovery của app thật. Kèm hardening: x2t exit ≠ 0 giờ coi là lỗi (không ghi đè file gốc
  bằng bản chuyển đổi đã mất changes).
- **Sự cố oracle (ghi lại để tránh lặp):** CDP wrap toàn bộ `window` (walk prototype chain)
  phá các global builtin — hàm bị thay thế mất toàn bộ static property (`Object.getPrototypeOf
  is not a function`), phải restart app sạch. Quy tắc: chỉ wrap object bridge cụ thể.
  `asc_enterText` KHÔNG có trong bảng export của word build (`prototype['asc_enterText']` không
  tồn tại) — dùng `asc_AddText` (word/api.js:15634, `AddTextWithPr`).
- **Bẫy PATH:** `std::env::temp_dir()` trên macOS = `$TMPDIR` (/var/folders/…), KHÔNG phải
  `/tmp` — workdir x2t nằm ở đó; khi soi workdir phải dùng `$TMPDIR`.
- **Code:** `src-tauri/src/bridge/save.rs` (append_changes + save_document + unit test byte
  format), `bridge/mod.rs` (dispatch mới + signature có AppHandle/registry), `lib.rs`
  (`DeliverableDocument.src` = path gốc; env `NANO_OFFICE_SAVE_PROBE`), `src/shim/desktopEditor.js`
  (KNOWN_METHODS + LOCAL_TRUTHS mới; relay FULL args — trước đây truncate 500 ký tự sẽ hỏng
  deltas; FIFO queue cho invoke Rust), `src/shim/delivery.js` (DOWN lane nhận `saveEnd`;
  probe save sau contentReady). Dọn thêm: unit test `rewrite_font_paths` trong protocol
  test URL loopback đã stale từ commit ef4ebe5 (verify.sh không chạy cargo test nên không
  thấy) — sửa expectation cho khớp hành vi committed.
- **Còn mở:** save-as dialog (PH-2), format khác docx (T-048), tự động hoá round-trip trong
  verify.sh (T-011), `LocalFileSaveDialog`/`ResaveFile`/`loadLocalFile` vẫn STUB.

---

## 2026-09-28 · T-045: refactor delivery chain (review C3)

- **Module hoá (chưa đụng behavior):** chuỗi open-delivery tách thành `src/shim/delivery.js`
  (deliverDocument/consumeDeliveredDocument/postDown + DOWN-lane listener riêng; desktopEditor.js
  giữ bridge/relay/font-patch, publish `window.__nanooffice_shim` = tauriInvoke+forwardLog cho
  delivery consume — surface 2 chiều tối thiểu). Constants đặt tên: DELIVERY_DELAY_MS=1500 ·
  RETRY_INTERVAL_MS=500 · RETRY_MAX=120 · HOP_GUARD=6; readiness predicate thành
  `engineReadiness()` (api + wasm `vue` + EndLoad fn). `src/main.ts`: engine path + `_offline_`
  về 1 nhà `ENGINE_BOOT`. lib.rs inject thêm DELIVERY_JS ngay sau BRIDGE_JS (init script + vite
  import đều giữ thứ tự bridge→delivery). x2t spawn KHÔNG đụng (T-043/T-010 sở hữu).
- **Probe gate:** mọi probe (font-oracle, canvas ×2, DocInfo, cc-wrapper, probe decode) sau predicate
  duy nhất `probesEnabled()` đọc `window.__NANO_OFFICE_PROBE__` — flag boolean do Rust init script
  set từ env `NANO_OFFICE_PROBE` (default ON debug / OFF release; `=0` tắt, `=1` bật). Mechanism
  ghi trong header delivery.js; open-signal hooks (asc_onError/contentReady) KHÔNG gate — là evidence
  của chính delivery.
- **Log contract gate 9 nguyên vẹn verbatim** (`asc_onDocumentContentReady` · `viewer=WxH` ·
  `canvas probe` · `GET /fonts/`) + comment trỏ chéo 2 chiều shim↔smoke-render.sh (đổi 1 đầu phải
  đổi đầu kia cùng commit).
- **Verify:** `./harness/scripts/verify.sh` exit 0 (gate 9 boot app thật, đủ 3 tín hiệu);
  `smoke-render.sh` standalone exit 0; test thêm `NANO_OFFICE_PROBE=0`: 0 dòng probe, delivery
  production vẫn chạy (contentReady + EndLoad executed); `git status` sạch file lạ. Commit `0d8f7a6`.

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
