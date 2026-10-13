# WasenOffice — AGENTS.md

> Harness cho dự án clone ONLYOFFICE Desktop Editors sang Rust + Tauri.
> Agent làm việc trong repo này BẮT BUỘC đọc và tuân thủ file này.

## identity & mục tiêu

- **Tên dự án:** WasenOffice — rebrand trọn vẹn từ "NanoOffice" ngày 13.10 (hai mẻ cùng ngày: mặt
  tiền rồi identifier nội bộ; repo `github.com/EricNguyen1206/wasenoffice`).
  Identifier chuẩn hiện tại: crate `wasenoffice-ooxml`, binary cargo `wasenoffice`,
  env `WASEN_OFFICE_*`, lệnh `wasen_light_*` (lệnh của tính năng **Nano Light** — tên hiển thị
  tính năng giữ nguyên), scheme `wasenoffice-engine://` + `wasenoffice-file://`, window stamp
  `__wasenoffice_*`. Đổi identifier là task riêng, phải chạy verify FULL (đã áp dụng lần rebrand 13.10).
- **Mục tiêu:** app desktop office suite (DOCX/XLSX/PPTX/PDF) — shell Rust + Tauri,
  giữ engine editing JS gốc của ONLYOFFICE (sdkjs + web-apps), x2t chạy sidecar.
- **Thay engine chỉ bằng clean-room, không port/translate** (QĐ9, 29.09.2026): cấm mang code
  AGPL sang code mình dưới mọi hình thức; thay thế từng module theo Track A chỉ từ
  **spec mở + oracle quan sát** — đọc `harness/rules/clean-room.md` trước khi đụng task Track A.
  Engine AGPL giữ vai trò reference + "chế độ nâng cao" đến tận Milestone A6 (user chốt 29.09).

## Layout (nguồn chân lý về cấu trúc)

```
wasenoffice/
├── AGENTS.md            ← file này — contract cho agent
├── docs/                ← kiến thức bằng chứng: reference.map, diagnostics, conventions
│   ├── design/          ← DD + TEST theo màn hình (source of truth UI, public — QĐ6)
│   └── handoff/         ← prompt bàn giao phiên agent (viết/ghi tiếp/xoá theo README ở đó)
├── harness/
│   ├── rules/           ← quy tắc bắt buộc (upstream-read-only, licensing, mit-purity, clean-room, evidence-discipline; migration/release sinh theo phase)
│   └── skills/          ← quy trình từng việc cụ thể (open-docx, bridge-shim, ...)
├── upstream/            ← MẢNG CHỈ ĐỌC — chỉ để tra cứu bằng git/submodule, không sửa, không commit vào
│   ├── sdkjs/           (gitlink → ONLYOFFICE/sdkjs)
│   └── web-apps/        (gitlink → ONLYOFFICE/web-apps)
├── src-tauri/           ← MÃ NGUỒN CHÍNH của dự án (Rust: shell, bridge, sidecar)
├── src/                 ← frontend Tauri (start page, wrapper UI — KHÔNG sửa engine JS ở đây)
└── engines/             ← artifact build của sdkjs/web-apps (sinh ra, không edit tay)
```

## Quy tắc gốc (tóm tắt — chi tiết trong harness/rules/)

1. **Upstream chỉ đọc.** Mọi file trong `upstream/` không được sửa, không được copy-and-patch.
   Cần thay đổi hành vi engine → làm ở lớp wrapper/adapter phía mình (shim, monkey-patch qua config API).
   Lý do: giữ khả năng pull upstream theo quý. Xem `harness/rules/upstream-read-only.md`.
2. **Không sửa upstream → không vi phạm AGPL cho phần code của mình.** Phần code WasenOffice tự viết
   dùng **MIT** (`LICENSE` ở root); engine AGPL giữ nguyên provenance qua submodule. Khi PHÁT HÀNH binary có nhúng
   AGPL component → vẫn phải tuân thủ AGPL cho component đó (mở source component + ghi credit).
   Xem `harness/rules/licensing.md` và `docs/vision.md`.
3. **Mọi nhận định về upstream phải có đường dẫn file làm bằng chứng.** Ghi vào `docs/reference.map.md`.
   Cấm đoán mò. Xem `harness/rules/evidence-discipline.md`.
4. **Bridge là hợp đồng.** Mỗi method `window.AscDesktopEditor` phải có stub khai báo ở
   `src-tauri/src/bridge/` kèm marker `// UNIMPL` nếu chưa implement thật. Xem
   `harness/skills/bridge-shim/SKILL.md`.
5. **Verification trước khi tuyên bố xong.** Lệnh kiểm chứng + exit code phải được nêu trong
   báo cáo. "ZCode/chrome devtools nói chạy được" ≠ đã verify.
6b. **App boot nền.** Mọi lần agent khởi động app (dev/binary/smoke gates/verify) phải
   chạy background — không đưa cửa sổ lên frontmost, không cướp focus màn user, không
   đè lên màn hình làm việc (user chỉ thị 03.10, mở rộng 04.10; enforce:
   `WASEN_OFFICE_BACKGROUND=1` → `ActivationPolicy::Accessory` set TRƯỚC `run()` +
   offscreen sliver 8px góc dưới-phải (`bg_sliver_position` — giữ `occlusionState
   .visible` cho engine probes) — `smoke_boot` tự bắn cho mọi gate boot; kèm Tauri
   `focused:false`). Diagnostics 04.10: policy set trong `setup` là MUỘN — phải set
   trước run.
6. **MIT purity.** Không copy code/text từ `upstream/` vào code của mình (kể cả gián tiếp qua
   subagent); dependency mới phải qua license gate trong verify.sh; `engines/**` không commit.
   Xem `harness/rules/mit-purity.md`.
7. **Thuật ngữ chuyên nghiệp:** Thang Track A gọi là **Milestone A0–A6** (hoặc Chặng/Nấc A0–A6),
   tuyệt đối không dùng từ 'rung'. Giữ nguyên các thuật ngữ kỹ thuật tiếng Anh chuẩn (`Commit`,
   `Refactor`, `Seam`, `Patch`, `Build`, `Runtime`, `Interface`).

## Progress snapshot (cập nhật khi hoàn thành phase)

| Phase | Trạng thái | Ghi chú |
|---|---|---|
| PH-0 Spike (load editor + render DOCX trong WKWebView) | ✅ ĐÓNG 27.09 — verdict **GO WITH CAVEAT** | render GREEN tự động (verify.sh gate 9, TC-D01-06); baseline debug 27.09: RAM 1 doc ~561–579 MB vs ONLYOFFICE 570 MB (start page); caveat duy nhất: scroll/edit keyboard chờ user xác nhận thủ công ~30s — `docs/diagnostics.md` 27.09 T-003 |
| Track A Milestone A0–A4 (QĐ9 — tự chủ shell + phân phối + converter + viewer + editor core) | ✅ A1 ĐÓNG 02.10 · A2 ĐÓNG 03.10 · A3 ĐÓNG 03.10 · **A4 ĐÓNG 04.10 (T-A27 L0 → T-A28 selection/arrows · T-A29 IME/clipboard · T-A30 dblclick/blink/bảng · T-A31 save-fidelity · T-A32 tblPr/tcPr warn)** | **A1 — phân phối độc lập**: chạy trọn open→edit→save KHÔNG cần ONLYOFFICE.app cài sẵn — x2t bundled (T-A10, 113 MB) + AllFonts tự sinh (T-A11) + fonts bundled có license (T-A12) + gate 11 "sạch ONLYOFFICE" thường trụ verify.sh (T-A13: strict clean-machine + negative proven + test tích thể mv app → verify FULL exit 0 khi app ẨN); **A2 — converter Rust**: T-A20 chốt P2 "OOXML core trước" bằng probe số liệu + crate slice `wasenoffice-ooxml`; T-A21 S1 docx READ đầy đủ (OPC chuẩn + table/list/image + styles resolve, fuzz 0 panic); T-A22 S2 docx WRITE + round-trip model bất biến 100% + file ghi ra render GREEN; T-A23 S3 parity harness 15/15 construct + 0 diff không phân loại; T-A24 S4 adapter vào runtime — open-side pre-normalize (docx → canonical.docx qua crate → x2t) trong seam `sidecar::convert`, kill switch `WASEN_OFFICE_NO_RUST_CONVERTER`, gates 9/10/11 GREEN cả ON lẫn OFF; **A3 — viewer Rust đọc-only ĐÓNG 03.10 (T-A25 design-first + T-V1→T-V5)**: DD V01 `docs/design/viewer-a3/DD.md` (R1 canvas 2D webview riêng không shim/engine; C3 command layer song song delivery; C4 OS/2 không hmtx); T-V1 model render-side + resolve về Rust (round-trip + parity-a2 bất biến); T-V2 layout pgSz + tab + OS/2 line metrics + **gate 12** số liệu (OQ-V01-2 chốt RAM = TREE, tripwire debug 300 MB); T-V3 media route `/media/<id>/<part>` + render ảnh EMU→px; **T-V4 parity render vs oracle CDP — 6/6 corpus ×2 deterministic + negative cắn, ε=2.0px (OQ-V01-1 chốt)**; T-V5 wire UX: nút Quick view trên start page (`viewer_view` — OQ-V01-3 chốt toggle) + scroll cơ bản (canvas cao bằng nội dung, 2-pass draw) + **gate 13 UI E2E**; mở <500 ms GREEN debug (18–89 ms); RAM tree debug 265 MB — **đích <150 MB = criterion RELEASE, T-057 (PH-4) chặn đo, ceiling ghi DD V01 §10**; ceiling parity → seed T-V6; **A4 — Nano Light T-A27 03.10** (chưa đóng rung — criterion đóng = parity matrix theo lớp, DD L01 §5.1): vòng edit đầu KHÔNG sdkjs/x2t — DocSession window-scoped + mutation commands hệ hitTest + caret canvas + undo snapshot + save `write_docx` atomic + gate 15, verify FULL 19 ▸ steps |
| PH-1 Vòng đời file (open/save/convert qua x2t sidecar) | ⬜ | |
| PH-2 Trải nghiệm desktop (print/clipboard/fonts/tabs) | ⬜ | |
| PH-3 Tùy chọn (crypto/signature/cloud) | ⬜ | có thể cắt vĩnh viễn |
| PH-4 Đóng gói + updater | ⬜ | |

## Việc còn treo

Hàng đợi công việc sống ở **`docs/TODO.md`** (QĐ7 — quy tắc + dashboard ở đầu file đó).
Điểm nổi bật hiện tại: **RUNG A4 ĐÓNG 04.10 (T-A27→T-A32) — editor core "Nano Light" KHÔNG
sdkjs/x2t trọn vòng edit: selection/arrows (T-A28) · IME composite tiếng Việt + clipboard +
Cmd-A (T-A29) · dblclick/triple + blink + edit bảng single-cell (T-A30) · save-fidelity
hyperlink/sectPr/settings/theme/docProps (T-A31) · tblPr/tcPr detect+warn (T-A32) — matrix
§5.1: 9 lớp GIỮ, dòng mất còn lại TOÀN BỘ detect+warn (hết mất im lặng); OQ-L01-2/3 ĐÓNG;
OQ-L01-1 default-switch DỜI A5 (user chốt 04.10)**. Vòng đầu mở màn T-A27 ĐÓNG 03.10
(8b5356e + 6e63399) — A4 MỞ MÀN, editor core
"Nano Light" vòng edit đầu KHÔNG sdkjs/x2t: design-first DD **L01**
`docs/design/editor-a4/DD.md` (commit trước code) → `src-tauri/src/editor.rs`
**DocSession window-scoped** (`EditorSessions` map window-label → session: `Document`
CHƯA-resolve + bytes gốc làm `media_src` + undo/redo snapshot-clone kèm caret cap 100 +
dirty = `doc != saved`) + **mutation command layer** song song delivery (insert/backspace/
delete/enter/undo/redo, toạ độ = hệ hitTest T-V7, offset utf-16; edit bảng refuse) +
caret forward-map thuần `src/editor.ts` consume `layout.ts` **nguyên vẹn 0 diff**
(parity-view không cần chạy lại) + entry `editor.html` ROOT + nút Nano Light start page +
env `WASEN_OFFICE_NANO_LIGHT` (**mặc định mở vẫn engine — D1**) + **save = `write_docx` +
tmp/rename atomic, KHÔNG Editor.bin/x2t** + **parity matrix ngoài-model trung thực**
(DD §5.1: mất hyperlink-cả-text/sectPr giữa/pgMar/settings/theme/docProps — mỗi mục
criterion nâng) + **gate 15** `smoke-nano-light.sh` thường trụ verify.sh (probe chạy cùng
edit() của phím → save → crate parse chứa marker + media sha256 byte-exact + reopen window
2 render marker; positive ×3 exit 0, first-draw 27–39 ms, negative garbage/doctor exit 1) —
verify FULL exit 0 **19 ▸ steps** ×2. Kế tiếp: **T-055 #6** review kiến trúc post-rung A4
(nhịp định kỳ sau rung đóng); T-V6 khi parity đòi.
Trước đó 03.10: **T-059 + T-V7 ĐÓNG** (4a12d00 + f7a887f — registry keying 4 consumer
bỏ `latest()` + layout module thuần/hit-test: parity 6/6 ×2 từng px, gate 14 + probe
gate 12, serde camelCase). Trước đó: **RUNG A1 ĐÓNG 02.10 (T-A10→T-A13) — phân phối
độc lập**: chạy trọn open→edit→save KHÔNG cần ONLYOFFICE.app cài sẵn — x2t bundled (113 MB) +
AllFonts.js tự sinh + fonts bundled có license + **gate 11 "sạch ONLYOFFICE" thường trụ**
trong verify.sh (strict clean-machine mode `WASEN_OFFICE_NO_APP_FALLBACK`; positive exit 0 +
negative proven exit 1) — bằng chứng đóng rung: test tích thể mv app → verify.sh FULL exit 0
khi app ẨN (`harness/scripts/clean-machine-proof.sh`, diagnostics 02.10). Runtime dependency
bắt buộc lên app: **0** (fallback x2t = WARN dev-only, bị strict mode chặn; upstream submodule
= reference theo D1 tới A6).
Mới nhất 03.10 (**T-V5 — wire UX quick-view, ĐÓNG RUNG A3**): quick view ra khỏi dev-env
thành UX thật — nút **Quick view** trên start page → command `viewer_view` (stash path
Rust-side `QuickViewRequest`, window `quickview-<n>` focused(false) rule 6b không shim;
env `WASEN_OFFICE_QUICK_VIEW` giữ cho dev/gate 12) + **scroll cơ bản** (canvas cao bằng nội
dung, 2-pass draw, cap 8192px logic — pagination post-A3); OQ-V01-3 CHỐT = toggle (chỉ thị
user). **Gate 13** `smoke-quick-view-ui.sh` thường trụ verify.sh — driver
`WASEN_OFFICE_UX_PROBE` (pattern SAVE_PROBE: điền input + click nút thật, không keystroke)
→ positive exit 0 (view → first-draw 30 ms → layout 16 dòng) + negative exit 1 (.docx rác
cắn). Bằng chứng đóng rung: parity-view (T-V4) 6/6 PASS không đổi + gate 12 không thoái hóa
(265 MB tree debug ≤ 300 tripwire; first-draw 89 ms) + scroll E2E canvas 2232 px; đích mở
<500 ms GREEN; **đích RAM <150 MB = criterion RELEASE — T-057 (release boot stall, PH-4)
chặn đo release, ceiling ghi thẳng không fake số (DD V01 §10)**. Mới nhất 03.10 (**T-059 +
T-V7 — 2 điều kiện trước A4, ĐÓNG**): **T-059** (4a12d00) xoá `FileRegistry::latest()` —
keying tường minh theo fileId: `register(owner=window-label)` + `unregister_owner` khi
window Destroyed; 4 consumer keying rõ (review #4 liệt kê 3 — consumer thứ 4
`LocalFileSetOpenChangesCount` tự lộ khi mổ); shim stamp `window.__wasenoffice_file_id__`
từ `open_document` → relay + delivery truyền `{fileId}`; thiếu/unknown = lỗi to; test
race ×2 + gates 9/10/12/13 GREEN; bài học: serde không tự camelCase (gate 9 bắt
`doc.fileId` undefined). **T-V7** tách layout viewer thành module thuần
`src/viewer/layout.ts` (không DOM/canvas, measurer tiêm vào, `LineModel`) + **hit-test
ngược** `hitTest(x,y) → {para,line,run,offset}` cho caret A4 — parity KHÔNG fork
(parity-view 6/6 ×2 từng px); 2 tầng test: **gate 14** `test-layout.sh` (42 checks tọa độ
tay) + probe E2E gate 12 (`WASEN_OFFICE_QV_HITTEST` → `viewer hit-test: 16/16`, negative
proven) + resize re-layout. Chi tiết DD V01 §5.7. Kế tiếp: **T-A27** mở màn A4 (editor
core "Nano Light", design-first khi claim — nạp deps review #4).
Trước đó cùng ngày (**T-055 #4 —
review kiến trúc post-A3**): candidates land — **T-059** (registry keying bỏ `latest()`: quick-view
đăng ký vào cùng registry với delivery/save = wrong-open/save-fail trong cửa sổ race; bug nhỏ,
trước A4) + **T-V7** (layout core tách module + hit-test ngược — điều kiện A4: layout parity
ε=2.0px không được fork) + **deps C2 vào T-A27** (DocSession giữ Document chưa-resolve + undo +
mutation commands + save = write_docx không qua Editor.bin). Kế tiếp khi đó: **T-059 + T-V7** rồi
**T-A27** mở màn A4 (editor core "Nano Light", design-first khi claim).
Trước đó 03.10 (**T-A24 — A2/S4 adapter, ĐÓNG RUNG A2**, ponytail ULTRA chuỗi A2 — verify FULL
exit 0): crate `wasenoffice-ooxml` vào đường runtime qua seam `sidecar::convert` — **open-side
pre-normalize**: open docx → crate parse+write `canonical.docx` trong workdir → x2t chuyển
canonical ra Editor.bin (save-side KHÔNG thay được — x2t fromChanges làm việc trực tiếp trên
Editor.bin, giữ x2t đến A4 theo DD K01 §2 P2). Crate refuse → WARN + x2t nhận file gốc (không
bao giờ chặn open); kill switch `WASEN_OFFICE_NO_RUST_CONVERTER=1` = đường cũ nguyên vẹn.
Log ADDITIVE `converter: rust-canonical → x2t (…, normalize …)` — không đụng log contract
gates 9/10/11. Bằng chứng: verify.sh FULL exit 0 (ON) + gate 9/10 counterfactual OFF exit 0
(Editor.bin 1586 B đường cũ vs 1495 B canonical) + parity-a2.sh exit 0 (7/7 invariant) +
cargo test 23 shell + 9 crate (test seam thêm case ON/refuse/kill-switch); overhead ~2 ms/open.
Trước đó 03.10 (**T-A23 — A2/S3 parity harness**, ponytail ULTRA chuỗi A2 — verify FULL exit 0):
`harness/scripts/parity-a2.sh` mỏng + CLI có sẵn — mỗi corpus file chạy x2t roundtrip (2 call
shape của seam: docx→Editor.bin positional + params XML→docx) + crate roundtrip, 3-way model
compare canonical JSON (INVARIANT m-orig==m-crate gate cứng 7/7; PARITY m-crate vs m-x2t =
**15/15 construct**, DD §5.1). Corpus 7 file: 2 fixture crate + 4 mới `make_docx_edge.py`
(multilevel-list/nested-table/style-override/image-multi) + **engine-save thật gate 10**
(engine-save-t011.docx). **2 bug crate fix + test hồi quy:** `mc:AlternateContent` (x2t/Word
bọc drawing trong mc:Choice + VML Fallback — trước fix mất image trên mọi file x2t-rt; fix =
branch Choice đầu làm run content, ECMA-376 MCE, số clause 29500-3 không verify được từ nguồn
mở → không ghi số) + `a:blip` Start+End (x2t writer trộn self-closing và có-end-tag). Diff còn
lại 2 lớp vô hại oracle-obs: rel_id renumber + empty-run drop. Không nâng model (3-state/
sectPr không bị đòi; blind-spot rStyle ghi DD). Harness ×2 deterministic exit 0.
Trước đó 02.10 (**T-A22 — A2/S2 docx WRITE + round-trip**, ponytail ULTRA chuỗi A2 —
verify 16 gates exit 0): crate `wasenoffice-ooxml` ghi model → docx hợp lệ — writer đối
xứng parser 1 hàm/construct (`src/write.rs`, quick-xml escape + zip có sẵn, không
builder/trait); package OPC tự sinh đúng chuẩn (Content_Types/rels — part chính = canonical
name `word/document.xml` mà reader tra cứu qua root rels, không hardcode path ngược;
refuse image extension lạ thay đoán content type); **round-trip `parse→write→parse` model
bất biến 100%** trên corpus đầy đủ (table/list/image/hdr-ftr + tiếng Việt) + fuzz-write
500 vòng mutation-on-model 0 panic; **file ghi ra mở trong engine render GREEN** — x2t
parse exit 0 (oracle-obs) + `smoke-render.sh /tmp/ta22-roundtrip.docx` PASSED exit 0 ×2.
Model đổi tối thiểu theo schema: StyleDef `+kind` (§17.7.4.17 @w:type required) + derive
PartialEq/Clone; image bytes không trong model → placeholder PNG 1×1 std-thuần (media
rel/part/content-type thật). 19 clause DrawingML verify qua mirror Remarks — ký ức sai
5/19 (bài học T-A21 lặp lại: số clause của element nằm ở bảng children của CHA). Bẫy
round-trip bắt được: main part phải wrap `w:body`; styles/numbering cần relationships
riêng. Limitation ghi diagnostics: Run.bold/italic 2-state → false serialize absent.
Kế tiếp: **T-A23** (S3 parity, x2t = DEV-ORACLE) → T-A24 adapter seam.
Trước đó 02.10 (**T-A21 — A2/S1 docx READ đầy đủ**, ponytail ULTRA chuỗi A2 — verify 16 gates
exit 0): crate `wasenoffice-ooxml` đọc đủ văn bản cơ bản — **OPC đúng chuẩn** (Content_Types +
rels, part chính tra cứu qua `_rels/.rels` không hardcode, refuse content-type sai),
recursive-descent parser 1 hàm/construct (không visitor/trait), **table** `w:tbl`/tr/tc
(cell lồng được), **list** (`w:numPr` raw + `numbering.xml` resolve numFmt/lvlText),
**image relationship-only** (`a:blip r:embed` → part + `wp:extent` EMU, refuse rel sai
loại/external), **styles.xml resolve** basedOn chain (con thắng cha, cycle → Err),
**headers/footers** (root check `w:hdr`/`w:ftr` + rels riêng từng part). Corpus mới
`harness/fixtures/make_docx_full.py` (10 parts: style chain Heading1→Normal + run override
+ table 2×2 + 3 list + PNG 1×1 + hdr/ftr + tiếng Việt); fuzz 500 vòng seed 42 (LCG tự
viết, không dep fuzz) 0 panic; spec-ref verify từng số section qua bảng Remarks
open-xml-docs (mirror ISO/IEC 29500-1 1st ed) — sửa 2 comment cũ sai (w:i §17.3.2.16,
w:t §17.3.3.31). Kế tiếp: **T-A22** (S2 write: model → docx round-trip, mở trong engine
render GREEN) → T-A23 parity (x2t = DEV-ORACLE) → T-A24 adapter seam.
Trước đó 02.10 (**T-A20 — A2 mở màn**, design-first ponytail ULTRA — verify 16 gates exit 0):
**P2 "OOXML core trước" CHỐT bằng probe số liệu** — P1 (Editor.bin reverse): 1962 B docx →
1586 B Editor.bin deterministic, TLV `[u8 tag][u32 len]` không nén, style resolve trong
binary + delta records = format thứ 2 → ước lượng tháng + dead code sau A4; P2: 100% spec
ECMA-376, ~85 element types cho docx cơ bản → tuần (DD mã K01 `docs/design/converter-a2/DD.md`,
kèm lộ trình S1–S5 + parity criteria). Crate slice `src-tauri/crates/wasenoffice-ooxml`:
parse docx → model tối giản + CLI JSON, 3 test, deps quick-xml/zip qua license gate;
verify.sh cargo `--workspace` (bẫy: manifest package+workspace root mặc định package-mode).
T-A21 S1 đã chạy (xem đoạn trên); hàng đợi A2: T-A22 write → T-A23 parity (x2t = DEV-ORACLE) →
T-A24 adapter seam; PH-1 còn lại T-046/T-047/T-049 (T-049 có gate-lib để clone).
Trước đó 02.10 (review #2 subagent 2, hygiene — verify 16 gates exit 0):
**T-053** smoke-lib.sh 1 nhà cho 3 gate 9/10/11 (log contract đóng băng hằng, consumer mỏng,
replace-don't-layer proven standalone + negative) · **T-054** font-root + 2 flavor AllFonts.js
1 chủ ở `protocol/allfonts` (mod.rs/filesrv consumer; filesrv range-parse tự viết bị xoá thay
bằng `parse_range_header` có test) · **T-044** gate `check-bridge-surface.mjs` thường trụ
verify.sh (đối chiếu shim ↔ rust dispatch ↔ reference.map ↔ globals.d.ts — bắt được 2 mismatch
thật ngay lần đầu). Trước đó cùng ngày (subagent 1): **T-A14** converter seam +
**T-052** formats 1 nhà — slot adapter T-A20 sẵn.
Trước đó: T-A12 02.10 — font bundle `engines/fonts` + trim sdkjs-plugins; T-A11 30.09 —
AllFonts tự sinh Rust (clean-room đầu tiên); T-A10 30.09 — x2t bundled; T-048 29.09 mở cả 4
định dạng + T-050 30.09 fix gate pptx flaky.
- Stack đã chốt: **giữ Rust + Tauri** (re-validate 26.09 — `docs/techstack-analysis.md`).

## Khi bắt đầu một phiên làm việc

1. Đọc `docs/vision.md` (tầm nhìn + quyết định sản phẩm) và `docs/conventions.md` (quy ước code + kiến trúc mục tiêu).
2. Đọc `docs/TODO.md` → **claim đúng 1 task** (🟨 + ngày); không đụng task 🟨 của session khác. Task có handoff → đọc theo Bước 0 của handoff đó (`docs/handoff/`). Cuối phiên: cập nhật trạng thái + dashboard + dòng ▶.
3. Việc mới → kiểm tra `harness/skills/` có skill tương ứng không trước khi tự làm.
4. Sửa code xong → chạy `harness/scripts/verify.sh` (nếu tồn tại) trước khi báo complete.
5. Phát hiện điều mới về upstream → ghi bằng chứng vào `docs/reference.map.md`, đừng để chỉ trong chat.
