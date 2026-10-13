---
type: design
---
# Detail Design (DD) — E01 · Cửa sổ soạn thảo bảng tính (editor XLSX)

## 0. Metadata

| Trường | Giá trị |
|---|---|
| Mã màn hình | `E01` (nhóm E — các màn XLSX) |
| Tên màn hình | `editor-xlsx` |
| Mã cha | — (màn chính) |
| Trạng thái | in-development |
| Phase | PH-1 |
| Owner | Eric Nguyen |
| Cập nhật gần nhất | 2026-09-29 |
| TEST tương ứng | [`TEST.md`](TEST.md) |

## 1. Tổng quan

Màn mở/soạn file XLSX của WasenOffice: cùng kiến trúc với D01 (editor DOCX) — WKWebView
hosting **spreadsheeteditor** + sdkjs build `cell` — khác D01 ở các điểm do shell điều
khiển: (1) boot params `doctype=cell&filetype=xlsx` (bản đồ tại `src/main.ts`
`FILE_FORMATS`, bằng chứng upstream: `docs/reference.map.md` § multi-format), (2) engine
build `cell` dùng bộ tên minified riêng cho font loader / glyph manager — shim đã
generalise theo biến thể build (T-048), (3) x2t convert `fixture.xlsx` → `Editor.bin`
magic `XLSY` (oracle-obs 29.09).

## 2. Bối cảnh & phạm vi

- **Goals:** mở XLSX local render đầy đủ trong shell WasenOffice (đích T-048); smoke gate
  behavioral tự động trong verify.sh (TC-E01-01).
- **Non-goals:** save XLSX (T-049 — format code + font params) · print/clipboard (PH-2) ·
  sửa `upstream/`/`engines/`.
- **Quyết định liên quan:** QĐ6 (DD+TEST theo màn) · contract mở/save: `reference.map.md`.

## 3. Luồng mở file (khác biệt so với D01)

Như D01 §3 (INIT → ENGINE_BOOT → READY_NO_DOC → OPENING → RENDERING → READY), với:
- ENGINE_BOOT: boot page **dùng chung** `api/documents/index.html` — nó đọc
  `doctype`/`filetype` rồi `DocsAPI.DocEditor` tự chọn app `spreadsheeteditor`.
- OPENING: x2t convert ra `Editor.bin` (magic `XLSY;v10`, oracle 29.09) → deliver qua
  `DesktopOfflineAppDocumentEndLoad` như D01 — EndLoad của build `cell` parse magic
  `XLSY` (observed trong build).
- Font pipeline: loader `AscFonts.mye.prototype.cZa` + registry `AscFonts.Nmc` +
  glyph manager `AscFonts.hj` (wasm handle `.RIe`) — shim patch loopback XHR như D01,
  biến thể chọn tự động (`detectFontVariant` trong `src/shim/desktopEditor.js`).

## 4. Bố cục & hành vi trực quan

UI = spreadsheeteditor nguyên bản, shell không đổi gì (rule read-only). Canvas signal của
gate: `canvas probe` trong `src/shim/delivery.js` (generic, 8 canvas quan sát 29.09).

## 5–9. N/A (màn kế thừa toàn bộ hành vi D01 — chi tiết không lặp lại ở đây; chỉ ghi điểm khác)

## 10. Câu hỏi mở

- OQ-E01-1: Đủ font hệ thống cho tiếng Việt trong cell build chưa được đo riêng (fixture
  có dấu, gate chỉ assert render). Theo dõi khi làm T-049 (save) hoặc PH-2 fonts.
