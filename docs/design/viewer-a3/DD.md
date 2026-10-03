---
type: design
---
# Detail Design (DD) — V01 · Viewer Rust đọc-only "quick view" (Track A, rung A3)

> Trạng thái: **draft** (mở màn A3 — T-A25, 03.10.2026, design-first như T-A20). Tài liệu
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
`NANO_OFFICE_QUICK_VIEW=<path>` (dev-only; UX wiring thật = V5) — window viewer mở bằng
entry `viewer.html` riêng, shim bridge/delivery KHÔNG inject (điểm của quick view: không
engine, không bridge).

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
| V3 | C3 trọn: serve `word/media/*` bytes (filesrv route an toàn — bytes đã byte-exact nhờ T-A26) + render ảnh theo `wp:extent` EMU | ảnh hiện đúng vị trí/kích thước trên corpus media |
| V4 | parity render vs oracle: so viewer render với render sdkjs (oracle CDP app thật) — text content + vị trí dòng | metric parity đạt trên corpus (định nghĩa metric ở slice này, OQ-V01-1) |
| V5 | wire UX: open docx → quick view (mặc định hoặc toggle — user quyết), rút trigger env | **rung A3 đóng** khi V4 parity + V2 gates đạt |

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

## 6. Rủi ro + mitigations

| Rủi ro | Mitigation |
|---|---|
| Mở model (C2) phá parity A2 | 2 invariant cứng mỗi slice: round-trip bất biến + parity-a2 0 diff; test hồi quy per-construct |
| `w:rFonts` → family không có trong font roots | map qua bộ bundled (Carlito ≈ Calibri, A12); thiếu → fallback + log đếm được (V1); không dừng open |
| OS/2 metrics scaled 1000/em truncate | sai số chỉ nằm ở line-height; advance là `measureText` (font thật); đo lệch ở V4 rồi mới quyết |
| Đo RSS thiếu process WebContent | đo cả cây (pgrep -P đệ quy) từ slice V0; công thức chốt ở V2 thành gate |
| Scope creep sang layout engine đầy đủ | V0 không layout; từng V-slice một phạm vi; §4 là biên |

## 7. Kế hoạch kiểm thử (thay TEST.md — component-level)

- V0: bằng chứng chạy thật + số đo ghi diagnostics; không gate mới (slice).
- V1+: crate tests per-construct (pattern A2) + parity-a2 là regression thường trụ.
- V2: gate behavioral verify.sh — boot quick-view + poll first-draw + đo RSS tree
  (pattern smoke-lib, log ADDITIVE riêng viewer, không đụng contract 9/10/11).
- V4: parity harness riêng (pattern parity-a2.sh: oracle CDP, so content + vị trí).

## 8. Câu hỏi mở

- **OQ-V01-1:** metric parity render — ε cho baseline/width bao nhiêu là "đạt"? Chốt ở V4
  sau khi đo sai số nền giữa 2 canvas (viewer vs sdkjs cùng WKWebView).
- **OQ-V01-2:** công thức RAM — RSS app process đơn hay cả cây (WebContent/GPU helper)?
  V0 đo cả 2 để có số liệu; V2 chốt thành gate.
- **OQ-V01-3:** UX quick view (V5) — viewer là mặc định cho docx hay toggle user chọn?
  User quyết khi V4 đạt (sản phẩm mở <500 ms mới đáng mặc định).
