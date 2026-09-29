---
type: design
---
# Detail Design (DD) — P01 · Cửa sổ soạn thảo bài trình bày (editor PPTX)

## 0. Metadata

| Trường | Giá trị |
|---|---|
| Mã màn hình | `P01` (nhóm P — các màn PPTX) |
| Tên màn hình | `editor-pptx` |
| Mã cha | — (màn chính) |
| Trạng thái | in-development |
| Phase | PH-1 |
| Owner | Eric Nguyen |
| Cập nhật gần nhất | 2026-09-29 |
| TEST tương ứng | [`TEST.md`](TEST.md) |

## 1. Tổng quan

Màn mở/soạn file PPTX của NanoOffice: cùng kiến trúc với D01 (editor DOCX) — WKWebView
hosting **presentationeditor** + sdkjs build `slide` — khác D01 ở: (1) boot params
`doctype=slide&filetype=pptx` (bản đồ `FILE_FORMATS` trong `src/main.ts`, bằng chứng
upstream: `docs/reference.map.md` § multi-format), (2) engine build `slide` dùng bộ tên
minified riêng cho font loader / glyph manager — shim generalise theo biến thể build
(T-048), (3) x2t convert `fixture.pptx` → `Editor.bin` magic `PPTY` (oracle-obs 29.09).

## 2. Bối cảnh & phạm vi

- **Goals:** mở PPTX local render đầy đủ trong shell NanoOffice (đích T-048); smoke gate
  behavioral tự động trong verify.sh (TC-P01-01).
- **Non-goals:** save PPTX (T-049) · print/clipboard (PH-2) · sửa `upstream/`/`engines/`.
- **Quyết định liên quan:** QĐ6 (DD+TEST theo màn) · contract mở/save: `reference.map.md`.

## 3. Luồng mở file (khác biệt so với D01)

Như D01 §3, với:
- ENGINE_BOOT: boot page **dùng chung** — `doctype=slide` chọn app `presentationeditor`.
- OPENING: x2t convert ra `Editor.bin` (magic `PPTY;v10`, oracle 29.09) → EndLoad của
  build `slide` parse magic `PPTY` (observed trong build).
- Font pipeline: loader `AscFonts.K8d.prototype.VMa` + registry `AscFonts.D3b` +
  glyph manager `AscFonts.mi` (wasm handle `.Zie`) — shim chọn biến thể tự động.

## 4. Bố cục & hành vi trực quan

UI = presentationeditor nguyên bản, shell không đổi gì (rule read-only). Canvas signal
của gate: `canvas probe` generic (15 canvas quan sát 29.09).

## 5–9. N/A (màn kế thừa toàn bộ hành vi D01 — chi tiết không lặp lại ở đây; chỉ ghi điểm khác)

## 10. Câu hỏi mở

- OQ-P01-1: Theme tối giản trong fixture đủ cho mọi renderer path chưa (không assert
  màu/shape trong gate) — mở rộng fixture nếu PH-2 cần visual check.
