---
type: design
---
# Detail Design (DD) — V01 · Viewer Rust đọc-only "quick view" (Track A, rung A3)

> Trạng thái: **đóng theo rung A3** (T-V5, 03.10.2026 — V0→V5 đủ; ceiling parity → T-V6).
> Mở màn T-A25 design-first 03.10.2026. Tài liệu
> kiến trúc component-level (không phải màn UI): TEST không tách file — kế hoạch kiểm thử
> ở §7, sống trong code + gates + harness. Mã nhóm **V** cấp cho viewer (quy tắc
> `[Chữ][2 số]`; folder giữ `viewer-a3/` theo handoff T-A25 — tiền lệ K01/converter-a2).

## 1. Bối cảnh + mục tiêu

Rung A3 (QĐ9 `docs/vision.md`): **viewer Rust đọc-only "quick view"** — mở docx KHÔNG boot
sdkjs. Đích rung: mở **< 500 ms**, RAM **< 150 MB**, **parity render vs oracle**. Đây là
bước đầu rút sdkjs (313 MB) khỏi đường mở tài liệu; sdkjs/web-apps vẫn giữ "chế độ nâng
cao" tới A6 (D1, không hard-cut).

Nền có sẵn, không viết mới:
- crate `nanooffice-ooxml` (A2, K01): parse docx → model đã parity 15/15 construct với
  x2t; **in-process lib-dep** của shell (review #3 first-hand: `sidecar/mod.rs:248-251`
  gọi trực tiếp — viewer không cần CLI/JSON trung gian).
- `protocol/allfonts.rs` (A11): enumerate font + parse OS/2 line metrics (ascent/descent/
  line_gap, scaled 1000/em) — hiện `pub(crate)` + struct private.
- `protocol/filesrv.rs`: loopback origin (fonts `/fonts/`, tài liệu `/file/<id>/`).
- Canvas 2D proven trong WKWebView (gate 9: sdkjs vẽ canvas GREEN).

Deps review kiến trúc #3 (03.10, diagnostics) nạp vào DD này: **C2** model render-side,
**C3** điểm cắm command layer, **C4** font metrics (chốt §3.3).

## 2. Phương án render

| Phương án | Nội dung | Đánh giá |
|---|---|---|
| **R1 — canvas 2D "PDF-style"** | webview riêng + canvas 2D; viewer TỰ layout (line height từ font metrics, line break theo pgSz) và vẽ text positioned như PDF renderer | cùng kỹ thuật sdkjs đang dùng (canvas 2D) → parity đo được trên cùng môi trường; layout model là CỦA MÌNH → A4 editor (cursor/selection/edit) dựng tiếp trên đó; 0 engine boot |
| R2 — DOM/HTML text flow | nhúng text vào DOM, browser tự wrap bằng CSS | rẻ nhất cho đọc, nhưng layout là hộp đen của WebKit → không dựng được editor A4 lên đó, parity vị trí không đo được; chỉ xứng khi quick view là đích cuối (không phải) |
| R3 — docx → PDF → xem PDF | tái dùng x2t export + một PDF viewer | phụ thuộc x2t (AGPL vẫn chạy mọi lần mở, chậm hơn parse trực tiếp) và "dừng ở PDF" — không mở đường A4, trái mục tiêu A3 (bước đầu rút sdkjs) |

**Chọn R1.** R2/R3 bị loại không phải vì xấu mà vì không mở đường A4 — quick view là
slice đầu của editor riêng, không phải sản phẩm dừng.

## 3. Kiến trúc

```
src-tauri/src/viewer.rs              ← command layer viewer (C3): viewer_open — parse-only
src/viewer.html + src/viewer.ts      ← webview RIÊNG (không shim, không engine iframe); canvas 2D
crates/nanooffice-ooxml (K01)        ← model A2; V1 mở render-side (C2), resolve về Rust
protocol/allfonts.rs                 ← accessor line metrics (C4-a), V2
```

### 3.1 C3 — điểm cắm: command layer song song delivery (review #3)

Viewer cắm ở **Tauri command layer**, KHÔNG ở seam `sidecar::convert`: viewer bỏ qua x2t và
Editor.bin hoàn toàn (parse trực tiếp file gốc bằng crate — đúng nghĩa "không boot sdkjs"
và không đụng converter). `viewer_open` = parse-only: đọc file → `nanooffice_ooxml::parse_docx`
→ JSON model trả frontend. Không đụng FileRegistry (nhà của delivery), không đụng
delivery.js, log contract gates 9/10/11 nguyên vẹn. Trigger slice: env
`NANO_OFFICE_QUICK_VIEW=<path>` (dev-only + gate 12; UX wiring thật = V5 —
`viewer_view` từ start page, §4 V5/§5.6) — window viewer mở bằng
entry `viewer.html` riêng, shim bridge/delivery KHÔNG inject (điểm của quick view: không
engine, không bridge). *(V3 amendment: `viewer_open` ĐĂNG KÝ file vào FileRegistry để
keying route media `/media/<id>/<part>` — handoff T-V3 đòi fileId; luồng delivery không
hề đụng tới, quick-view mode không bao giờ gọi delivery.)*

### 3.2 C2 — model render-side mở theo spec, đúng nhu cầu viewer (V1)

Model A2 là model **parity** (đủ cho round-trip); render đòi thêm dữ liệu mà parser hiện bỏ
(`document.rs:243 Event::Empty(_) => {}`). Mở theo spec, từng mục có spec-ref, giữ 2
invariant: round-trip `parse→write→parse` bất biến + `parity-a2.sh` vẫn 0 diff không
phân loại (mở model không được phá parity):

| Construct | Spec-ref | Vì sao viewer cần |
|---|---|---|
| `w:rFonts` | §17.3.2.26 | chọn font cho canvas (ascii/hAnsi/eastAsia/cs) — foundational, mọi dòng đều qua |
| `w:br` / `w:tab` | §17.3.3.3 / §17.3.3.34 | line break / tab advance — hiện bị bỏ, render mất xuống dòng |
| Run bold/italic **3-state** | §17.3.3 ST_OnOff | phân biệt "không set" (kế thừa style chain) vs "tắt tường minh" — limitation 2-state T-A22 |
| `rStyle` | §17.5.2.3 | character style trên run — blind-spot parity T-A23 |
| `w:color`, `w:spacing` | §17.3.2.6 / §17.3.2.33 | màu text, character spacing |
| `w:ind`, `w:tblGrid`, `w:pgSz` | §17.3.1.12 / §17.4.4 / §17.6.11 | indent, độ rộng cột, khổ trang cho layout |
| `w:default` + `docDefaults` | §17.7.2.4 / §17.7.5.1 | default font/size khi không ai set |

Resolve (style chain + run props về giá trị render cuối) chuyển về **Rust** ở V1 — frontend
nhận giá trị đã resolve, không merge JSON trên TS (merge của slice V0 chỉ là chứng minh).

### 3.3 C4 — font metrics: chốt **(a) accessor OS/2, không parser hmtx**

- **(a) Chọn:** accessor rẻ đọc OS/2 line metrics `allfonts.rs` ĐÃ parse sẵn (ascent/
  descent/line_gap, scaled 1000/em — `Face` private, cần 1 accessor `pub(crate)`). Line
  height render = ascent + |descent| + line_gap theo family đã map.
  **Đã land ở V2 (T-V2):** `allfonts::line_metrics(family)` — scan CHỂ bundled root
  (~20 file, millisecond; không trả giá 557 mặt hệ thống cho mỗi lần mở), family trùng
  không phân biệt hoa thường; family không có trong bundle → `None` → renderer dùng
  fallback phẳng 1.15 (ceiling ghi `ponytail:` comment tại chỗ; V4 parity đo sai số rồi
  mới quyết nâng).
- **(b) Bỏ:** parser `hmtx` lấy advance thật — KHÔNG làm: canvas `measureText` đo advance
  của font thật (text stack WebKit, rung 4 ladder "native platform feature"); viết hmtx
  parser là duplicate đúng thứ platform đã làm.
- **Trigger mở lại (b):** parity render (V4) đòi advance identical với oracle mà
  `measureText` không đủ chính xác/khả nhất giữa môi trường. Khi đó hmtx là nguồn duy nhất
  khả-dịch (advance từ bytes font, không phụ thuộc WebKit).

### 3.4 Render model (frontend contract)

`viewer_open` trả JSON: `{ parse_ms, doc }` — `doc` = model crate (slice V0). V1 thay bằng
render model đã resolve (blocks → dòng đã layout-ready: text + font + size + bold/italic/
underline + màu + align + indent). Vẽ PDF-style: mỗi dòng một `fillText` tại baseline y;
line height từ C4-a; advances từ `measureText`.

## 4. Lộ trình slice (V0 chạy phiên này, V1+ là task TODO)

| Slice | Nội dung | XONG khi |
|---|---|---|
| **V0** (T-A25, phiên này) | slice chứng minh kiến trúc: `viewer_open` parse-only + `viewer.html` canvas vẽ text paragraph (không layout đầy đủ) + **số đo ms/MB đầu tiên** | app chạy thật: mở fixture qua env → text hiện trên canvas, log parse/first-draw ms + RSS |
| **V1** (T-V1, 03.10) ✅ | C2: model render-side mở (bảng §3.2) + resolve về Rust (`resolve.rs`); write.rs đối xứng | round-trip bất biến + parity-a2 exit 0 + render dùng model mới |
| V2 | layout tối thiểu: line break theo pgSz, tab stop, line-height C4-a; **gates số liệu** mở <500 ms + RAM <150 MB trong verify.sh (gate mới, không đụng 9/10/11) | gate GREEN trên fixture corpus; metric RAM chốt (OQ-V01-2) |
| **V2** (T-V2, 03.10) ✅ | wrap greedy theo cột từ `pgSz` + tab stop + line-height C4-a; **gate 12** `smoke-quick-view.sh` | gate GREEN (positive exit 0 + negative ×2 exit 1); OQ-V01-2 chốt §10 |
| **V3** (T-V3, 03.10) ✅ | route filesrv `/media/<fileId>/<part>` (exact-name zip lookup, 403 traversal — bytes byte-exact nhờ T-A26) + render ảnh dòng riêng theo `wp:extent` EMU→px | gate 12 assert drawn == EMU→px (gate tự tính lại) + ink thật trên fixture media; E2E fixture T-A26 |
| **V4** (T-V4, 03.10) ✅ | parity render vs oracle: so viewer render với render sdkjs (oracle CDP app thật) — text content + vị trí dòng | **`harness/scripts/parity-view.sh` 6/6 corpus PASS ×2 deterministic exit 0 + negative (tiêm 3 diff giả) 3/3 cắn exit 1; OQ-V01-1 chốt §5.5** |
| **V5** (T-V5, 03.10) ✅ | wire UX: nút **Quick view** trên start page → `viewer_view` (stash path Rust-side → window `quickview-<n>`, focused(false), không shim) + **scroll cơ bản** (canvas cao bằng nội dung, 2-pass draw); env giữ làm trigger dev/gate 12 | **rung A3 ĐÓNG**: gate 13 UI E2E positive exit 0 + negative exit 1; gate 12 không thoái hóa; parity-view 6/6 không đổi — §5.6 |

## 5. Tiêu chí đo

- **ms:** t0 = invoke `viewer_open` (tương đương user action), t1 = first text pixel trên
  canvas. Parse (Rust) + transfer + layout + draw nằm trong đó. Reference thêm: cold start
  (spawn app → first draw) đo ngoài. Đích: **< 500 ms**.
- **MB:** RSS **toàn cây process app** (app + WebContent + helpers — WKWebView chạy process
  riêng, đo thiếu tree là sai; OQ-V01-2 chốt công thức ở V2). Đích: **< 150 MB**. Baseline
  đối chiếu: engine open ~561–579 MB (PH-0 diagnostics 27.09).
- **parity render (V4):** so trên corpus với oracle CDP (sdkjs render thật): text content
  exact + số dòng + baseline mỗi dòng ±ε — ε chốt ở V4 sau khi đo sai số nền.

### 5.1 Kết quả slice V0 (03.10.2026 — T-A25, số đầu tiên)

Fixture `make_docx_full.py` (3794 B — style chain + table + list + tiếng Việt), debug build,
boots bằng `NANO_OFFICE_QUICK_VIEW=<file> pnpm tauri dev`, 2 lần chạy nhất quán:

| Đo | Số liệu | Đối chiếu |
|---|---|---|
| Parse (Rust, crate A2) | **2.9 ms** | adapter pre-normalize cũng ~2 ms (T-A24) — cùng crate |
| invoke → first text pixel | **14.0 ms** (run2) / 17.0 ms (run1) | gồm parse + JSON + canvas draw; run1 là lần đầu cấp page |
| Render evidence | `inked 27547` (sample đếm pixel tối trên canvas) | text thật sự vẽ, không phải draw no-op |
| RAM app process | **117.8 MB** (ổn định 2 run) | < 150 MB đích rung ✓ (nếu đích tính app-process đơn) |
| RAM + 3 WebKit XPC (method PH-0) | ≈ **236 MB** (WC 46.5 / GPU 49.3 / Net 22.7) | engine 1-doc: ≈ 561–579 MB (diagnostics 27.09) — WebContent rớt ~370 → ~46 MB; nhưng > 150 MB → đích rung phải chốt công thức + release build (OQ-V01-2) |

Đọc thẳng: **"không boot sdkjs" hiện thực hoá đúng** — engine WebContent (370 MB) biến mất
khỏi đường mở; phần RAM còn lại là shell + WebKit nền (idle start page debug cũng ~203 MB debug,
cùng method). V2 chốt gate: công thức RAM (app-only vs tree) + build release.

### 5.2 Kết quả slice V1 (03.10.2026 — T-V1)

Model mở đủ bảng §3.2 (Run 3-state/rStyle/rFonts/color/spacing, br/tab → ký tự điều khiển
trong `Run.text`, Ind, tblGrid, pgSz/sectPr, w:default, docDefaults) + `resolve.rs` trả
`RenderDoc` đã resolve (§17.7.2: direct → rStyle → pStyle/w:default → docDefaults) —
`viewer_open` trả model đã resolve, viewer.ts không merge gì nữa.

| Đo | Số liệu | Đối chiếu |
|---|---|---|
| Parse + resolve (full fixture 3794 B) | **4.8 ms** | V0: 2.9 ms — tăng vì pgSz/tblGrid/sectPr giờ được parse + thêm bước resolve |
| invoke → first text pixel (full fixture) | **24.0 ms** | V0: 14–17 ms — thêm line-split cho br/tab; cùng bậc, đích 500 ms xa |
| v1 fixture (2359 B) | parse 2.5 ms · first-draw 20.0 ms · inked 27499 | construct V1 render thật qua canvas |
| RAM app process | **~92–100 MB** debug | V0: 117.8 MB — không thoái hoá; công thức tree vẫn là T-V2 (OQ-V01-2) |

Parity sau khi mở model (invariant 9/9, exit 0) — 4 lớp diff ĐỀU vô hại, oracle-obs:
(1) empty-run drop + (2) rel_id renumber/rename (T-A23/T-A26 cũ) + **(3) x2t thêm
`w:docDefaults` rỗng** trên file gốc không có (surface hoá nhờ model mới) + **(4) hex color
lowercase** (ST_HexColor case-insensitive). Điểm nhấn: m-x2t của v1-fixture chỉ lệch 3 dòng
màu → **x2t giữ nguyên mọi construct V1 qua round-trip** (rStyle/rFonts/br page+column/
spacing âm/docDefaults/w:default/grid/pgSz). File round-trip crate mở trong engine render
GREEN (smoke-render exit 0 — writer XML được engine chấp nhận).

### 5.3 Kết quả slice V2 (03.10.2026 — T-V2)

Layout thật trên canvas V0: greedy wrap theo cột suy từ `pgSz` (A4 11906 twips → px, trừ
2×1in margin cố định — `pgMar` chưa vào model); tab stop 720 twips (`w:defaultTabStop`
§17.3.1.15 nằm ở settings.xml — model chưa parse; **720 = quy ước Word, không phải default
của spec** — verify số § qua nguồn mở, spec không khai default); line-height =
(ascent + |descent| + line_gap)/1000 × px từ accessor OS/2 C4-a. Advances vẫn `measureText`
(C4-b không đổi). Fixture `make_docx_layout.py` (4 paragraph: ngắn + filler dài wrap + tab +
medium; Carlito 11pt uniform; A4 pgSz) — boot dev nền, ×3 run nhất quán:

| Đo | Số liệu | V1 | V0 |
|---|---|---|---|
| Parse (+ resolve + scan line_metrics lần đầu) | **7.0–7.5 ms** | 4.8 | 2.9 |
| invoke → first-draw | **18–30 ms** | 20–24 | 14–17 |
| Layout evidence (log `viewer layout:`) | **16 dòng từ 4 paragraph** (filler wrap 12 dòng); colw 601.7 px; maxw 600.5 ≤ colw; lh **17.9 px** = (750+250+220)/1000 × 14.667 — metrics Carlito [750,−250,220] từ log | 1 dòng/paragraph | 1 dòng/paragraph |
| RSS app-only | **~103–113 MB** | ~92–100 | 117.8 |
| RSS tree (app + 3 WebKit XPC, method PH-0) | **177.8–257 MB** — helper RSS noisy (WC/GPU/Net 75–143 MB giữa các run) | — | ≈236 |

**Gate 12** `harness/scripts/smoke-quick-view.sh` (thường trụ verify.sh, pattern smoke-lib,
log ADDITIVE `viewer first-draw/layout/failed` — hằng `SMOKE_RE_QV_*` trong khối đóng băng
smoke-lib): assert **layout** (mỗi delta baseline == line-height của dòng == công thức OS/2
tính LẠI từ metrics đọc trong log; lhs uniform; maxw ≤ colw; lines ≥ 12 > 4 paragraph =
wrap ăn thật) + **first-draw < 500 ms** + **RSS tree < 300 MB**. Positive exit 0 ×3 chạy;
negative ×2 exit 1 fail-fast (không chờ timeout): `QUICK_VIEW_MAX_MS=1` → "first-draw 19 ms
exceeds budget"; `QUICK_VIEW_MAX_MB=1` → "RSS tree 257 MB exceeds budget" — 2 assert cắn
riêng lẻ (threshold là nội bộ gate nên negative = siết ngưỡng, không đụng product code).

### 5.4 Kết quả slice V3 (03.10.2026 — T-V3)

Media route + render ảnh. Clean-room: EMU quy ước verify nguồn mở — **Apache POI
`Units.java` (Apache-2.0): `EMU_PER_INCH = 914400`, `EMU_PER_PIXEL = 9525`** + spec-ref
ECMA-376 §20.4.2.7 (`wp:extent` cx/cy kiểu ST_PositiveCoordinate, đơn vị EMU); px = emu/9525
@96 dpi — cùng đơn vị px của cả page (PT_PX = 96/72).

| Mảnh | Nội dung | Bằng chứng |
|---|---|---|
| Route `/media/<fileId>/<part>` | filesrv, key bằng fileId (registry); part lookup **exact-name trong zip** (`try_part_bytes` — traversal không thể thoát ra đĩa); `..`/backslash → 403 tường minh TRƯỚC lookup | cargo test ×2 (byte-exact part + 403/404 bộ traversal/unknown) · curl route sống: image2 (part của HEADER) 200, bytes == fixture (sha256), `../` 404, `%2e%2e` 403, fileId lạ 404 |
| Render ảnh | ảnh = token **dòng riêng** (ceiling `ponytail:` — không inline wrap); vẽ `drawImage` tại `emu/9525`; fetch qua `fetch()`+`createImageBitmap` (không taint canvas — getImageData đọc được) | — |
| Evidence pixel | log `viewer media: part emu drawn at ink` (ink = pixel non-white trong rect vẽ) | fixture layout (2 ảnh body extent 100×50 / 50×30 px): drawn **100.0×50.0 / 50.0×30.0**, vị trí 398.3 = last-baseline + lh; **ink 20000/6000 = phủ kín @dpr2** — placeholder trắng 1×1 của writer sẽ ink ≈ 0 |
| Gate 12 mở rộng | assert media: drawn == EMU/9525 (gate tự tính lại từ EMU trong log), ink ≥ ½ rect, ảnh đầu đúng vị trí flow, ảnh kề nhau advance = chiều cao ảnh, cấm `viewer media-failed` | positive exit 0 (first-draw 27–32 ms, tree 261–265 MB); negative ms/MB exit 1 ×2; **counterfactual fake-log ×5 đều cắn** (drawn sai / ink thủng / vị trí trôi / mất media line / media-failed) |

Ceilings ghi rõ (ponytail): ảnh chỉ render trên dòng riêng; ảnh vẽ SAU text của run (parser
flatten thứ tự con trong run); **header image không render** (RenderDoc.body chỉ mang body —
route vẫn phục vụ bytes của part header, E2E chứng minh; render header = slice sau nếu parity
V4 đòi). Bug thật gate bắt được trong phiên: breakLine thừa sau ảnh → "dòng rỗng ma" 16.9 px
lọt vào lhs, assert uniform của gate cắn ngay run đầu — fix bằng chỉ breakLine khi còn token
phía sau.

---

### 5.5 Kết quả slice V4 (03.10.2026 — T-V4, parity render vs oracle CDP) — CHỐT OQ-V01-1

**Oracle (clean-room oracle-obs — không đọc source sdkjs):** ONLYOFFICE.app thật boot nền
`--remote-debugging-port=9222` (kỹ thuật diagnostics 22.09/27.09); harness
`harness/scripts/parity-view.sh` walk từng paragraph của doc MỞ THẬT qua CDP
`callCommand` (Document Builder API) bằng **collapsed-caret probe**:
`GetRange(p, p).Select()` + `asc_GetSelectionBounds()` trả điểm caret (x, y đỉnh line box)
— y nhảy tại ranh giới dòng → số dòng + tọa độ x/y từng dòng lấy thẳng từ layout của
engine; **text từng dòng đọc bằng selection** (`GetRange(s, e)` + `asc_GetSelectedText()`);
extent ảnh lấy từ Builder `ToJSON` (`extent.cx/cy` EMU). Zoom pin 100% → đơn vị CSS px =
không gian vẽ của viewer. Ba bẫy đã đo và xử lý trong harness (chi tiết diagnostics
03.10): (1) positions ĐẾM content element ẩn (engine re-split runs) → KHÔNG map được
position ↔ chỉ mục GetText — text line phải đọc qua selection; (2) bounds cache cập nhật
async theo draw → đọc stale → verify-triple (s, lo, lo−1) + re-bisect, fail loudly nếu
không chốt được; (3) caret cuối dòng trước space: engine vẽ caret sau space ở CUỐI dòng
trước — y-flip vẫn đúng ranh giới dòng.

**Metric (OQ-V01-1 CHỐT):** M0 sequence text paragraph (normalized) = HARD; M1 số dòng —
lệch phải PHÂN LOẠI (wrap ceiling); M2 text theo dòng (para cùng số dòng) — drift break
point = wrap ceiling (bất biến nội dung do M0 bảo đảm); M3 **spacing trong paragraph**
(j>0) **±ε HARD với ε = 2.0 px** (đo nền: max |dy| trong-paragraph = **0.64 px** trên 16
dòng uniform corpus; ε_x = 2.0 px, dx đo = 0.00 px), drift **j=0** = spacing ceiling
(w:spacing không trong model) — cả hai phía neo vào dòng đầu của mình (shift-corrected),
re-anchor sau bảng/ảnh bị skip; offset tuyệt đối (margin model + page placement) chỉ
BÁO, không gate; M4 extent ảnh EMU = HARD (đúng từng EMU), vị trí ảnh = ceiling own-line
(§5.4). Tab luân phiên giữ nguyên `\t` hai phía. Bảng = ceiling 2 phía (oracle không đi
vào cell; viewer `c:1`).

| Corpus (6) | paras | lines | maxdy | maxdx | imgs | verdict |
|---|---|---|---|---|---|---|
| layout-fixture (wrap + tab + 2 ảnh) | 4/4 | 16/16 | **0.64** | 0.00 | 2/2 | PASS |
| full-fixture (tiếng Việt + style + list + bảng + hdr/ftr) | 8/8 | 8/8 | 4.07* | 47.00* | 1/1 | PASS |
| media-fixture (ảnh + header image) | 2/2 | 2/2 | 0.00 | 0.00 | 1/1 | PASS |
| edge-multilevel-list (đánh số 3 mức) | 5/5 | 5/5 | 6.47* | 0.00 | 0/0 | PASS |
| edge-style-override (override trực tiếp) | 3/3 | 3/3 | 2.73* | 0.00 | 0/0 | PASS |
| engine-save-t011 (engine-save thật) | 5/5 | 3/7† | 2.73* | 0.00 | 0/0 | PASS |

\* drift tại ranh giới paragraph / dưới numbering — phân loại (spacing/numbering
ceiling), không phải trong-paragraph. † 2 paragraph viewer wrap 2 dòng vs oracle 1 —
phân loại wrap ceiling (M0 text identical).

Bằng chứng: positive **×2 chạy, exit 0, bảng số giống hệt từng px** (deterministic);
negative `PARITY_VIEW_NEGATIVE=1` tiêm 3 diff giả vào viewer evidence (+3px baseline,
+" XQ" text, EMU lệch 1px) → **cả 3 assert cắn (M0 text / M3 baseline / M4 EMU) → exit
1**. `parity-a2.sh` exit 0 (không thoái hóa).

**Ceilings lộ ra qua parity (đã phân loại, KHÔNG fix trong phiên — seed task):** (1)
wrap-point drift + line-count khác (font metric: viewer `measureText` WebKit vs font
engine riêng của sdkjs); (2) **numbering**: engine GetText/render mang mark ("1. ", "•")
+ indent cấp level, viewer chưa vẽ mark; (3) **w:spacing before/after** + indent từ
numbering level không trong model → khoảng cách paragraph lệch 2.7–6.5px; (4) margin:
viewer 1in cố định (pgMar chưa vào model) + engine vẽ trang giữa cửa sổ → offset tuyệt
đối (chỉ báo, đã shift-correct); (5) ảnh own-line (§5.4); (6) header/footer/bảng không
thuộc domain so (2 phía cùng skip). → **T-V6** (model mở pPr spacing + pgMar + render
numbering mark) khi parity đòi sắc hơn.

---

### 5.6 Kết quả slice V5 (03.10.2026 — T-V5, wire UX + ĐÓNG RUNG A3)

UX (**OQ-V01-3 CHỐT = toggle** — chỉ thị user 03.10: "chọn file từ UI mở quick-view thay
env"; quick-view KHÔNG thành mặc định cho docx — quyết định "mặc định" thuộc A4 Nano
Light): start page thêm nút **Quick view** cạnh Open → command `viewer_view`: validate
file tồn tại → stash path vào state `QuickViewRequest` (Mutex, take-once) → mở window
`quickview-<n>` (focused(false) rule 6b, KHÔNG inject shim/bridge — §3.1); `viewer_open`
consume stash, **env `NANO_OFFICE_QUICK_VIEW` giữ nguyên làm trigger dev/gate 12**. Disk
path không bao giờ vào URL (stash Rust-side — nguyên tắc filesrv). Đóng cửa sổ = close
native (Cmd-W) — không thêm phím tắt riêng (ponytail: platform feature). **Scroll cơ
bản:** canvas = trang — cao bằng nội dung đã layout (min = viewport; cap 8192px logic =
WebKit 16384 device px @dpr2 — cao hơn clip, pagination là việc post-A3, ceiling ponytail);
resize clears canvas → **2-pass draw**: measure → resize → re-scale ctx (context state
reset — bẫy) → redraw CÙNG input, cùng tọa độ; evidence lấy từ pass 2.

**Gate 13** `harness/scripts/smoke-quick-view-ui.sh` (thường trụ verify.sh, consumer
smoke-lib): boot app THƯỜNG (không env quick-view) với driver **`NANO_OFFICE_UX_PROBE`**
(pattern `NANO_OFFICE_SAVE_PROBE` — init JS điền input + click nút THẬT, không keystroke,
không focus) → assert `viewer view:` (command = UI wiring) → `viewer first-draw` <
QUICK_VIEW_MAX_MS → `viewer layout:`, cấm `viewer failed:`. Hằng `SMOKE_RE_QV_VIEW` vào
khối đóng băng smoke-lib; coordinate/media asserts không nhân bản — vẫn ở gate 12, gate
13 sở hữu WIRING.

| Bằng chứng | Kết quả | Ghi |
|---|---|---|
| Gate 13 positive | exit 0 — `viewer view:` → first-draw **30.0 ms** → layout 16 dòng (lhs 17.9, inked 27392) | chuỗi UI E2E thật, số không đổi so T-V3 |
| Gate 13 negative | exit 1 — .docx rác: chain chạy hết tới parse, `viewer failed:` cắn nhanh | assert proven, không timeout |
| Gate 12 (regression env) | exit 0 — first-draw 89 ms, layout giống hệt, RSS tree **265 MB** ≤ 300 tripwire | đường dev/gate nguyên |
| parity-view (T-V4) | exit 0 — 6/6 PASS, từng px giống bảng §5.5 | scroll 2-pass không lệch render |
| Scroll E2E (doc 60 paragraph, qua UI probe) | first-draw 23.0 ms, **canvas 2232 px** > viewport 700 | body scroll trọn document |

**Đóng rung A3 — ceiling RAM ghi thẳng (đúng §10, không fake số):** đích rung **< 150 MB
là criterion cho RELEASE build** — T-057 (release boot stall, PH-4) chặn đo release; số
debug = tripwire gate 12 300 MB (đo 265 MB). Đích mở < 500 ms GREEN trên debug (18–89 ms).
Parity (T-V4) 6/6 corpus PASS — criterion thứ ba của rung đạt.

## 6. Rủi ro + mitigations

| Rủi ro | Mitigation |
|---|---|
| Mở model (C2) phá parity A2 | 2 invariant cứng mỗi slice: round-trip bất biến + parity-a2 0 diff; test hồi quy per-construct |
| `w:rFonts` → family không có trong font roots | map qua bộ bundled (Carlito ≈ Calibri, A12); thiếu → fallback + log đếm được (V1); không dừng open |
| OS/2 metrics scaled 1000/em truncate | sai số chỉ nằm ở line-height; advance là `measureText` (font thật); đo lệch ở V4 rồi mới quyết |
| Đo RSS thiếu process WebContent | đo cả cây từ slice V0 — **method đã sửa ở V2 (thử thật): helper WebKit spawn bởi launchd, KHÔNG phải con app (`pgrep -P` đệ quy trả rỗng) → nhận diện theo PID adjacency (method PH-0); công thức chốt §10** |
| Scope creep sang layout engine đầy đủ | V0 không layout; từng V-slice một phạm vi; §4 là biên |

## 7. Kế hoạch kiểm thử (thay TEST.md — component-level)

- V0: bằng chứng chạy thật + số đo ghi diagnostics; không gate mới (slice).
- V1+: crate tests per-construct (pattern A2) + parity-a2 là regression thường trụ.
- V2: gate behavioral verify.sh — boot quick-view + poll first-draw + đo RSS tree
  (pattern smoke-lib, log ADDITIVE riêng viewer, không đụng contract 9/10/11).
- V4: parity harness riêng (pattern parity-a2.sh: oracle CDP, so content + vị trí).

## 8. Câu hỏi mở

- **OQ-V01-1: CHỐT ở V4 — xem §5.5.** ε_baseline = 2.0 px (within-paragraph, đo nền max
  0.64 px); ε_x = 2.0 px (đo 0.00 px); drift ranh-giới-paragraph / numbering = phân loại
  ceiling, không gate.
- **OQ-V01-2: CHỐT ở V2 — xem §10.**
- **OQ-V01-3: CHỐT ở V5 — toggle.** Chỉ thị user 03.10: chọn file từ UI mở quick view
  (nút Quick view trên start page → `viewer_view`); quick view KHÔNG thành mặc định cho
  docx — quyết định "mặc định" thuộc A4 (Nano Light). Env `NANO_OFFICE_QUICK_VIEW` giữ
  làm trigger dev/gate 12.

## 10. OQ-V01-2 — công thức RAM đã chốt (T-V2, 03.10)

**Công thức = TREE: RSS app process + toàn bộ WebKit XPC kề nó (WebContent / GPU /
Networking), nhận diện theo PID adjacency — đúng method PH-0 diagnostics 27.09.**

Lý do (theo số đo, không theo cảm tính):

1. **App-only mù đúng thành phần mà A3 tuyên bố đã loại.** Cả câu chuyện rung là WebContent
   370 MB (engine) → ~46 MB (quick view) — gate chỉ đo app process sẽ KHÔNG bao giờ bắt được
   thoái hoá WebContent; tree là vạch đo duy nhất nhìn thấy nó.
2. **Đích rung đối chiếu theo toàn cây.** Baseline engine 561–579 MB (PH-0) là tổng cây
   (app + 3 helper); đích <150 MB phải cùng đơn vị mới so được.
3. **Method đã proven + có sẵn.** PID adjacency đo được bằng `ps`/`pgrep` thuần, đã dùng từ
   PH-0; helper spawn bởi launchd (KHÔNG phải con của app — pgrep -P đệ quy trả rỗng, thử
   thật 03.10) nên adjacency là cách gán đúng đã ghi.

Ngưỡng theo build (số đo §5.1/§5.3):

- **verify.sh gate 12 = tripwire DEBUG 300 MB** (số đo tree 178–257 MB, dao động do helper
  RSS; thoái hoá kiểu WebContent engine +370 MB sẽ bị bắt tức thì). Đích 500 ms giữ nguyên.
- **Đích rung < 150 MB = criterion cho RELEASE build** — debug không thể là vạch: idle start
  page debug đã ~203 MB tree (V0 §5.1). Đo khi PH-4 packaging (build phân phối thật).

**Limitation ghi thẳng:** số RELEASE chưa đo được trong phiên T-V2 — release boot stall
pre-existing (không phải T-V2 gây ra: boot release KHÔNG quick-view cũng stall; diff T-V2
không đụng boot): bare binary = cửa sổ lên + WebContent spawn nhưng idle hoàn toàn (sample:
run loop chờ `mach_msg`, không nhận navigation) + 0 dòng `page_load`; bundle `.app` mở bằng
`open -g --env` = 0 cửa sổ. Đã loại App Nap (`NSAppSleepDisabled`), caffeinate, đợi >90 s;
debug boot cùng máy tức thì. Đây là việc PH-4 packaging phải xử lý trước khi đo đích rung —
→ **T-057 (PH-4)**.
