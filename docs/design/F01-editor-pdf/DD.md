---
type: design
---
# Detail Design (DD) — F01 · Cửa sổ xem/soạn PDF (editor PDF)

## 0. Metadata

| Trường | Giá trị |
|---|---|
| Mã màn hình | `F01` (nhóm F — các màn PDF; chữ mới cấp 29.09, bảng cấp mã ở `docs/design/README.md`) |
| Tên màn hình | `editor-pdf` |
| Mã cha | — (màn chính) |
| Trạng thái | in-development |
| Phase | PH-1 |
| Owner | Eric Nguyen |
| Cập nhật gần nhất | 2026-09-29 |
| TEST tương ứng | [`TEST.md`](TEST.md) |

## 1. Tổng quan

Màn mở PDF của WasenOffice: WKWebView hosting **pdfeditor** — app này tái dùng
sdkjs build `word` (observed: `pdfeditor` app require `sdkjs/word/sdk-all-min`) cộng lớp
PDF riêng của engine (`sdkjs/pdf/src/engine/drawingfile.wasm`). Khác D01/E01/P01 ở điểm
quan trọng nhất: **PDF KHÔNG qua x2t convert** — shell copy file gốc vào thư mục work và
deliver bytes gốc qua `DesktopOfflineAppDocumentEndLoad` (oracle CDP trên ONLYOFFICE.app
29.09: recovery folder chứa `f.pdf` 1003 B nguyên bản, KHÔNG có `Editor.bin`; khớp
fallback `folder + "/" + file_name(src)` khi Editor.bin thiếu trong
`client_renderer_wrapper.cpp` "onlocaldocument_loadend"). Boot params
`doctype=pdf&filetype=pdf&isForm=false` (bản đồ `FILE_FORMATS` trong `src/main.ts`).

## 2. Bối cảnh & phạm vi

- **Goals:** mở PDF local render đầy đủ trong shell WasenOffice (đích T-048); smoke gate
  behavioral tự động trong verify.sh (TC-F01-01).
- **Non-goals:** save PDF (pipeline khác — `localSaveToDrawingFormat`, ghi nhận trong
  T-049) · form filling OCR (cắt) · sửa `upstream/`/`engines/`.
- **Quyết định liên quan:** QĐ6 (DD+TEST theo màn) · contract mở: `reference.map.md`.

## 3. Luồng mở file (khác biệt so với D01)

Như D01 §3, với:
- ENGINE_BOOT: boot page **dùng chung** — `doctype=pdf` chọn app `pdfeditor`; kèm
  `isForm=false` như shell thật.
- OPENING: **không x2t** — `get_document_for_delivery` nhánh pdf copy file gốc vào work
  dir rồi deliver bytes đó (raw `%PDF`), EndLoad của build word (pdfeditor) xử lý nhánh
  non-DOCY.
- Font pipeline: như D01 (build word — `sje.BSa`/`Wg.vue`).

## 4. Bố cục & hành vi trực quan

UI = pdfeditor nguyên bản, shell không đổi gì (rule read-only). Canvas signal của gate:
`canvas probe` generic (5 canvas quan sát 29.09 — 1 canvas trang + 4 overlay).

## 5–9. N/A (màn kế thừa hành vi D01; điểm khác đã ghi §1/§3)

## 10. Câu hỏi mở

- OQ-F01-1: Save/export PDF đi pipeline riêng (không phải x2t from-changes như DOCX) —
  chưa điều tra; thuộc T-049.
- OQ-F01-2: Tiếng Việt trong PDF yêu cầu font nhúng (fixture chỉ đặt tiếng Việt ở Title
  metadata vì font base-14 WinAnsi không encode được dấu) — kiểm tra khi làm PH-2 fonts.
