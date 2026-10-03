---
type: notes
---

# corpus-a2 — corpus parity T-A23 (A2/S3)

Input của `harness/scripts/parity-a2.sh`. 4 fixture sinh bằng `make_docx_edge.py`
(stdlib, spec-derived ECMA-376) + 1 file engine-save thật từ gate 10 (smoke-save).
3 fixture gốc của crate (`tests/fixtures/nanooffice-fixture.docx`,
`nanooffice-full-fixture.docx`, `nanooffice-media-fixture.docx` — T-A26) cũng
thuộc corpus — harness lấy cả 2 thư mục.

| File | Expected model (tóm tắt) |
|---|---|
| `edge-multilevel-list.docx` | numbering `{1: {0: decimal "%1.", 1: lowerRoman "%2.", 2: lowerLetter "%3)"}}`; 5 paragraph ilvl 0/0/1/1/2, numId=1 |
| `edge-nested-table.docx` | body: para "Bảng lồng:" + table 1×2 — cell(0,1) chứa table 2×1 ("ô trong","hàng hai") + para "sau bảng trong"; kết thúc para "Kết thúc bảng lồng." |
| `edge-style-override.docx` | styles: IntenseQuote {bold Some(true), italic Some(true)} (basedOn Quote→Normal 2 hops), Quote {italic Some(true)}, Normal {}, Strong (character) {bold Some(true)}; run overrides `i=false` (trên Quote), `b=false` (trên IntenseQuote); run 3 tham chiếu `rStyle Strong` (blind-spot: model không surface rStyle) |
| `edge-image-multi.docx` | 3 paragraph, mỗi run 1 image CÙNG rel `rIdImg` → `word/media/image1.png`; extents 9525×9525 / 19050×9525 / 28575×9525 EMU |
| `nanooffice-media-fixture.docx` (crate tests/fixtures, T-A26) | body 1 image `rIdImg` → `word/media/image1.png` (PNG 2×2, 79 B) + header 1 image `rIdHImg` → `word/media/image2.png` (PNG 3×1, 74 B) — bytes ĐA KÍCH THƯỚC ≠ placeholder 1×1; gate media byte-exact của parity chạy đúng trên file này (crate-rt phải copy nguyên bytes) |
| `engine-save-t011.docx` | output thật của pipeline save gate 10 (open → asc_AddText probe → asc_Save → x2t from-changes): package do x2t sinh khi save, chứa marker `T011-*-SAVED-BY-NANOOFFICE` trong `word/document.xml` |

Provenance: `make_docx_edge.py` regenerate deterministic; engine-save tái lập bằng
`harness/scripts/smoke-save.sh` (marker random mỗi lần — file committed là bằng chứng
một lần chạy, không byte-stable).
