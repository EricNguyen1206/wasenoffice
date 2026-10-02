---
type: design
---
# Detail Design (DD) — K01 · Converter core A2 (Rust thay x2t — Track A)

> Trạng thái: **draft** (mở màn A2 — T-A20, 02.10.2026). Tài liệu kiến trúc component-level
> (không phải màn UI): TEST không tách file — kế hoạch kiểm thử ở §7, sống trong crate + gates.
> Mã nhóm **K** cấp cho converter core (handoff T-A20 đề xuất "AC" — 2 chữ vi phạm quy tắc
> `[Chữ][2 số]`, tiền lệ F01 29.09; folder giữ `converter-a2/` theo handoff).

## 1. Bối cảnh + mục tiêu

x2t (113 MB, AGPL C++) làm 2 việc cho NanoOffice: **open** (docx/xlsx/pptx → `Editor.bin`,
magic `DOCY/XLSY/PPTY;v10`) và **save** (`Editor.bin` + `changes0.json` → file gốc, params XML —
hợp đồng đã reverse: `docs/reference.map.md` § save contract). Cả 2 hướng chạm **Editor.bin —
định dạng binary NỘI BỘ, không spec công khai**. Đích đóng rung A2 (QĐ9 `docs/vision.md`):
round-trip parity vs x2t trên corpus + fuzz, x2t rút khỏi runtime.

Seam sẵn sàng: `sidecar::convert` (T-A14) chờ adapter thứ 2; `formats.rs` (T-052) là bảng format
1 nhà. Crate A2 cắm vào seam, KHÔNG đụng luồng save production (x2t còn production đến khi parity).

## 2. Hai phương án — số liệu probe (02.10.2026)

### P1 — "sdkjs-seam": converter Rust đọc/ghi Editor.bin thay x2t trong luồng hiện tại

Probe oracle-obs (x2t bundled `engines/converter/aarch64-apple-darwin/x2t`, fixture
`harness/fixtures/make_docx.py` 1962 B; tái lập: `x2t fixture.docx out.bin`):

| Quan sát | Số liệu | Hệ quả |
|---|---|---|
| Kích thước | 1962 B docx → **1586 B** Editor.bin, exit 0 | nhỏ hơn input — không nén bọc ngoài |
| Determinism | 2 lần convert **byte-identical** | differential probing khả thi |
| Header | magic `DOCY;v10;0;` (11 B) + ~24 B nhị phân: 3 giá trị u32 LE khớp ranh giới payload (651=0x28b, 1471=0x5bf, 1582=0x62e) — nghi bảng offset section, **semantics chưa xác định** | ngay phần đầu 35 B đã còn điều chưa decode |
| Payload | 926 B (0x28c–0x62a) **không nén** (0 zlib stream); record stream `[u8 tag][u32 LE len][value]` | cấu trúc đảo được, xem tiếp |
| Lồng nhau | 2 chuỗi verify byte-exact: chuỗi text 46 B → cha 51 → 51+5=56 → +5=61; chuỗi 204 → 209 → 214 → 219 → 224 — **mỗi mức +5 B** | rule TLV xác nhận ở 2 nhánh sâu 3–4 mức |
| Chuỗi | UTF-16LE inline (u32 len + bytes), ~20 chuỗi/5 đoạn văn | có thể extract văn bản nhanh |
| Resolve | styleId `Heading1` **và** style name `heading 1` cùng nằm trong binary | converter resolve style trước serialize — muốn thay x2t phải re-implement luật resolve (style/theme/numbering) mới ra binary đúng |
| Preamble | 42 B đầu payload + header chưa decode sau 1 probe phiên | chưa chiếm được 5% file 1.6 KB tối giản |

Hai format nội bộ phải reverse, đều không spec: (1) Editor.bin — từ điển tag `[u8][u32]` với
semantics phụ ngữ cảnh (ước lượng hàng trăm nghĩa cho docx cơ bản — model sdkjs mirror; mỗi nghĩa
≥1 chu kỳ probe khác biệt: mutate docx → convert → diff bytes); (2) **delta records trong
changes0.json** — T-010 đã obs "text trong delta mã hoá run-length, áp bởi DoctRenderer JS
trong x2t" → save-from-changes phải re-implement cả delta-application. Đổ thêm: writer phải
được parser của sdkjs (`f$c` → `WKa.At`) chấp nhận — mỗi tag thiếu = render hỏng im lặng.

### P2 — "OOXML core trước": crate parse/serialize docx theo ECMA-376, bỏ qua Editor.bin

Probe spec-ref + đo fixture (ECMA-376 Part 1 WordprocessingML §17 + DrawingML §20; Part 2 OPC;
Part 3 Markup Compatibility — spec chuẩn mở, sạch clean-room):

| Đo | Số liệu |
|---|---|
| Fixture hiện có (5 phần văn, render GREEN trong engine) | **5 parts OPC, 2.5 KB XML, 21 element types + 15 attribute types** — surface thật của một doc đủ mở/đọc |
| Mở rộng "docx cơ bản" (heading✓/bold✓/italic/table/list/image/hyperlink/headers-footers/section) | ước lượng **~85 element types** (table +9–14, numbering part +11–13, image wp:/a:/pic: +15–20, rPr +6, hdr/ftr +5, docProps +6) trên ~7 parts, 4 namespace (`w`, `wp/a/pic`, opc, rels) |
| Phần không spec | **0** — mọi element/attribute/quan hệ rels đều spec'd công khai |

### Đối chiếu — khuyến nghị **P2**

| Tiêu chí | P1 (Editor.bin) | P2 (OOXML core) |
|---|---|---|
| Spec đầu vào | 0% — 2 binary nội bộ, oracle-only | 100% ECMA-376 (clean-room hoàn hảo) |
| Effort "docx cơ bản" round-trip | **tháng** — từ điển tag + luật resolve + writer chấp nhận được bởi parser engine + delta-application; long-tail parity (tag thiếu = hỏng im lặng, chỉ thấy qua render) | **tuần** — ~85 element types, parser+model+writer ~2.5–3.5 kLOC, hành vi spec'd |
| Rủi ro parity | cao — corpus+fuzz phải đuổi từng tag ẩn | thấp — parity đo trên model, x2t làm DEV-ORACLE (so output, không reverse) |
| Tiền tự chủ | chỉ rút **113 MB x2t** — sdkjs+web-apps **313 MB vẫn ship** (engine vẫn cần cho edit), app vẫn boot engine mỗi doc | mở đường rút TOÀN BỘ engine: A3 viewer (không boot sdkjs), A4 editor tự đọc/ghi OOXML → x2t rút thật |
| Tái sử dụng | **dead code sau A4**: Nano Light làm việc trên OOXML trực tiếp; "chế độ nâng cao" vẫn dùng x2t thật (không ai tiêu thụ writer Editor.bin của mình nữa) | nền bắt buộc của A3+A4 — 0 throwaway; dùng được ngay cho tiện ích (metadata, text-extract cho AI QĐ3, diff, sanitize) |
| Ship ở mọi rung (QĐ9) | không ship được gì giữa chừng | S1 là sẵn công cụ đọc docx của shell |

**Khuyến nghị: P2 trước.** P1 chỉ đúng khi muốn rút x2t TRƯỚC khi có editor riêng — và điểm
hết hàng của nó là A4, nơi P1 trở thành dead code. Trigger mở lại P1 (ghi để không quên):
user muốn rút 113 MB x2t trước A4 vì lý do phân phối/license → khi đó chỉ cần writer
OOXML→Editor.bin (1 consumer thật: build Editor.bin cho sdkjs advanced-mode). Cân lại ở thời điểm đó.

## 3. Kiến trúc (P2) — 1 crate, vài file

```
src-tauri/crates/nanooffice-ooxml/     ← crate duy nhất của A2 (KHÔNG workspace đa crate;
                                        member của workspace src-tauri chỉ để gates kiểm soát)
  src/lib.rs        ← model tối giản + entry parse
  src/opc.rs        ← OPC: zip → parts (ECMA-376 Part 2)
  src/document.rs   ← document.xml → model (ECMA-376 Part 1 §17)
  src/main.rs       ← CLI: docx → JSON (stdout)
  tests/…           ← 3 test (fixture chuẩn + edge XML + tiếng Việt đã gộp trong fixture)
```

- Model tối giản slice: `Document{paragraphs}`, `Paragraph{style, runs}`, `Run{text, bold, italic}`.
  Không trait hierarchy, không error-type tower — `ParseError(String)`.
- Ranh giới với shell: crate ĐỨNG NGOÀI `nanooffice_lib` — future adapter cắm sau
  `sidecar::convert` (T-A14) dưới dạng nhánh gọi crate, không đụng seam khi chưa parity.
- Dependency (2, qua license gate — justification trong commit): `quick-xml` (MIT OR Apache-2.0 —
  stdlib không có XML; parser tự viết vi phạm rule trust-boundary "bắt XML lỗi thay vì panic")
  + `zip` (MIT — OPC là zip container theo Part 2; stdlib không có inflate) + `serde_json`/`serde`
  (đã có trong graph — JSON out cho CLI).

## 4. Lộ trình slice (sau T-A20)

| Slice | Nội dung | XONG khi |
|---|---|---|
| S1 | docx **read**: OPC + document.xml + styles.xml → model (para/run/props/table/list/image) | corpus fixture parse 100% construct, cargo test |
| S2 | docx **write**: model → docx (round-trip) | mở file S2-ghi được trong engine render GREEN (gate pattern 9) |
| S3 | parity harness: docx→model→docx so vs x2t trên corpus + fuzz | parity metric đạt (§5), corpus mở rộng từ corpus thật |
| S4 | cắm adapter `sidecar::convert` (open path trước, save path sau) | gate 9/10 qua bằng adapter — rung A2 milestone |
| S5+ | xlsx (SpreadsheetML §18), pptx (PresentationML §19) | theo đúng thứ tự QĐ9 |

## 5. Tiêu chí parity đo được

- **Model parity (S1):** với mỗi fixture corpus: parse bằng crate ↔ parse thủ công theo spec
  (bảng construct) — 100% khớp paragraph/run/props; 0 panic trên corpus fuzz (mutation: XML lỗi,
  zip lỗi, entity, namespace lạ → `Err`, không panic).
- **Round-trip parity (S3):** docx → model → docx: text/style/structure bất biến (canonical
  compare); đối chiếu **x2t làm oracle**: cùng input, x2t round-trip output ↔ mình round-trip
  output → so trên model (không so byte — x2t cũng không byte-stable so với input).
- **Behavioral parity (S4):** file do S2 ghi mở trong engine: contentReady + canvas + text marker
  (đúng pattern gate 9/10, log contract giữ nguyên).

## 6. Rủi ro + mitigations

| Rủi ro | Mitigation |
|---|---|
| Bề rộng OOXML thực tế (docx thật có mc:AlternateContent, rPr mặc định ngầm, theme fonts) | S1 bắt đầu từ corpus fixture + corpus thật nhỏ; mọi construct lạ → OQ + thêm test, KHÔNG đoán; fallback: refuse-unknown-part rõ ràng thay parse sai |
| `w:val` semantics đa hình (on/off/enum/measure) | spec-ref từng nhóm; test per-construct trong crate |
| x2t output không byte-stable giữa version | oracle so trên model + pin version x2t trong corpus harness |
| Scope creep sang engine | crate chỉ model + IO; render/apply-changes là A4 — không làm trong A2 |
| License nhiễm | clean-room rule toàn phiên; 2 dep mới qua `check-licenses.mjs`; similarity review trong code-review |

## 7. Kế hoạch kiểm thử (thay TEST.md — component-level)

- Slice T-A20: 3 test trong crate — (1) fixture chuẩn `make_docx.py` (bao tiếng Việt có dấu +
  smart quotes + tab): đếm paragraph/run + assert text exact; (2) edge XML hỏng → `Err` không
  panic; (3) construct pStyle/bold/italic + `w:val="false"` semantics.
- Parity: §5 — S3 sinh harness riêng, là điều kiện mở S4.
- Gates: verify.sh thêm `--workspace` vào 3 lệnh cargo (check/fmt/clippy — manifest src-tauri
  là package + workspace root, không có flag cargo chỉ chạy package gốc) + `check-licenses.mjs`
  phủ graph crate tự động (không gate mới). `cargo test` chạy `-p nanooffice-ooxml` thủ công
  theo convention hiện có (shell cũng vậy — 26 test = 23 shell + 3 crate).

## 8. Câu hỏi mở

- **OQ-K01-1 (user veto):** khuyến nghị P2 — user chốt? Nếu muốn P1 (rút x2t trước A4) → mở lại
  theo trigger §2, effort tháng, ghi rõ trong TODO.
- **OQ-K01-2:** corpus cho S3 — dùng corpus docx thật (license?) hay tự sinh + fuzz (proven,
  pattern make_*.py)? Quyết lúc S3.
- **OQ-K01-3:** `docProps/core.xml` (metadata) có vào S1 không — AI layer QĐ3 cần metadata sớm;
  default: có, vì chi phí ~0 (1 part nhỏ).
