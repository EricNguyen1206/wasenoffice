---
type: design
---

# Detail Design (DD) — U01 · UI editor riêng A5 (Track A, Milestone A5)

> Trạng thái: **draft** (mở màn Milestone A5 — T-A33, 11.10.2026). Tài liệu kiến trúc
> component-level (thay thế web-apps cho các surface của mình): TEST không tách file —
> kế hoạch kiểm thử ở §7, sống trong code + gate 15 + harness. Mã nhóm **U** cấp cho
> UI editor riêng (quy tắc `[Chữ][2 số]`; folder đặt `editor-ui-a5/` theo tiền lệ K01/V01/L01).
> Criterion đóng MILESTONE A5 = web-apps rút khỏi phân phối mặc định (vision.md QĐ9).

## 1. Bối cảnh & mục tiêu

Milestone A5 (QĐ9 `docs/vision.md`): **UI editor riêng** thay thế `web-apps` cho các
surface của NanoOffice (start page, quick view chrome, Nano Light chrome).
Engine ONLYOFFICE giữ vai trò "chế độ nâng cao" (D1 — không hard-cut tới A6);
khi mở chế độ nâng cao, web-apps vẫn được nạp.

Task T-A33 (Slice 0):
- Chốt quyết định sản phẩm về Framework (**RATIFY**).
- Chốt bảng giá trị màu sắc thực (**Graphite neutral-minimal**).
- Thiết kế kiến trúc chrome bọc quanh `DocumentSurface` (vừa tách ở T-064).
- Dựng và chạy thật **Slice 0** (Nano Light chrome: header + tên file + dirty dot + nút Save thật).

---

## 2. Quyết định kỹ thuật: Framework RATIFY (Design-it-twice)

| Tiêu chí | Phương án 1: Vanilla TS + Tailwind v4 (CHỌN cho Slice 0) | Phương án 2: Preact ~4.5KB (RATIFY cho Slice 1+) | Phương án 3: Svelte 5 (LOẠI) |
| :--- | :--- | :--- | :--- |
| **Runtime overhead** | **0 KB** (không thêm runtime dependency) | ~4.5 KB gzipped | ~10 KB compiler runtime |
| **Độ phức tạp Slice 0** | **Rất thấp** (~25 dòng DOM sync: `#doc-name`, `#dirty-dot`, `#save-btn`) | Cần config JSX factory, mount hook | Cần Svelte compiler plugin cho Vite |
| **Agent Fluency** | Rất cao (DOM API chuẩn) | **Cao nhất** (chuẩn JSX / React pattern) | Trung bình (cú pháp template riêng) |
| **License (MIT gate)** | Đạt (100% code nội bộ) | Đạt (MIT license, allowlist) | Đạt (MIT license) |
| **Khả năng mở rộng** | Nguy cơ phình code khi làm toolbar phức tạp | **Lý tưởng** cho component cây trạng thái sâu | Tốt cho reactivity |

### Quyết định sản phẩm:
1. **Slice 0 (Task T-A33):** Giữ **Vanilla TypeScript + Tailwind v4**. 
   *Lý do theo Ponytail ULTRA:* Header của Slice 0 chỉ có 3 element tương tác (`#doc-name`, `#dirty-dot`, `#save-btn`). Toàn bộ logic DOM sync chỉ tốn 25 dòng code. Thêm dependency framework ngay lúc này là over-engineering.
2. **Trigger chuyển đổi Preact:** Sẽ tích hợp Preact (~4.5KB) khi bắt đầu làm **Tab bar đa tài liệu (PH-2)** hoặc **Ribbon/Toolbar định dạng văn bản (Slice 2+ của A5)**, nơi số dòng manual DOM sync vượt ngưỡng 100 dòng.
3. **Loại Svelte 5:** Mặc dù cú pháp gọn, mức độ thông thạo (fluency) của các AI coding agent trên JSX/Preact cao hơn đáng kể so với Svelte runtime template.

---

## 3. Bảng Theme Tokens: Graphite Neutral-Minimal

Theo phân tích `docs/techstack-analysis.md`, hướng thiết kế chốt là **Graphite neutral-minimal** (tông màu zinc/slate trung tính, document-first, chrome mỏng chìm vào nền, tuyệt đối không sao chép ONLYOFFICE hay MS Office):

| Token Slot | Ý nghĩa | Giá trị Light (chuẩn) | Giá trị Dark (tự động) |
| :--- | :--- | :--- | :--- |
| `--color-bg` | Nền ứng dụng / khung ngoài | `#f4f4f5` (zinc-100) | `#18181b` (zinc-900) |
| `--color-fg` | Màu chữ mặc định | `#18181b` (zinc-900) | `#f4f4f5` (zinc-100) |
| `--color-accent` | Điểm nhấn tương tác / dirty dot | `#2563eb` (blue-600) | `#3b82f6` (blue-500) |
| `--color-panel` | Bề mặt điều khiển (button/input) | `#ffffff` | `#27272a` (zinc-800) |
| `--color-panel-active` | Nút khi nhấn / hover | `#e4e4e7` (zinc-200) | `#3f3f46` (zinc-700) |
| `--color-edge` | Đường viền ngăn cách nhẹ | `#d4d4d8` (zinc-300) | `#3f3f46` (zinc-700) |
| `--color-paper` | Màu nền trang tài liệu | `#ffffff` | `#ffffff` (trang giấy luôn trắng) |
| `--radius-panel` | Bo góc điều khiển | `6px` | `6px` |

---

## 4. Kiến trúc Chrome bọc Canvas

```
┌────────────────────────────────────────────────────────────────────────┐
│ Header Chrome: [Logo] [Tên tài liệu] [Dirty Dot]        [Nút Save]     │
├────────────────────────────────────────────────────────────────────────┤
│ Container: DocumentSurface Viewport (overflow: auto)                   │
│                                                                        │
│        ┌──────────────────────────────────────────────┐                │
│        │   Canvas 2D (#cv) — Document Rendering       │                │
│        │   (Inked text, carets, selections, media)    │                │
│        └──────────────────────────────────────────────┘                │
│                                                                        │
└────────────────────────────────────────────────────────────────────────┘
```

1. **Header DOM (`editor.html`):**
   - Nằm phía trên viewport canvas, không co giãn (`shrink-0`).
   - `#doc-name`: Hiển thị tên file từ payload `NanoLightDoc.name`.
   - `#dirty-dot`: Chấm tròn accent, hiển thị khi `dirty = true` (khi `doc != saved`).
   - `#save-btn`: Nút bấm lưu file gọi lệnh `nano_light_save` (hoặc phím tắt `Cmd-S`).
2. **Tương tác với `DocumentSurface`:**
   - Viewport canvas nằm bên trong `<main id="canvas-box" class="flex-1 overflow-auto">`.
   - `DocumentSurface` (từ `src/viewer/surface.ts`) tự động đo đạc kích thước vùng hiển thị (`vw`/`vh`) theo container này.
   - Thao tác gõ chữ, điều hướng caret, selection tiếp tục diễn ra trên canvas mà không bị cản trở bởi header.

---

## 5. Lộ trình Slices (Milestone A5)

- **Slice 0 (Task T-A33 hiện tại):** 
  - Hoàn thiện chrome tối thiểu cho Nano Light trong `editor.html` / `src/editor.ts`.
  - Nút Save gọi lệnh Rust `nano_light_save` thật.
  - Cập nhật bộ theme tokens Graphite trong `src/styles.css`.
  - Đảm bảo Gate 15 (`smoke-nano-light.sh`) tiếp tục đạt PASSED.
- **Slice 1 (Seed):** Chuyển đổi start page sang layout và theme Graphite.
- **Slice 2 (Seed):** Thêm thanh chrome mỏng cho Quick View (`viewer.html`).
- **Slice 3 (Seed):** Tab bar quản lý đa tài liệu (tích hợp Preact).

---

## 6. Kế hoạch Kiểm thử & An toàn

1. **Gate 15 (Smoke Nano Light):**
   - Probe `NANO_OFFICE_NL_PROBE` tự động gõ chữ $\to$ dirty dot hiện $\to$ bấm Save $\to$ disk thay đổi $\to$ reopen trong cửa sổ thứ hai.
   - Selector `#save-btn` và `#dirty-dot` phải được probe tìm thấy chính xác.
2. **Gate 14 (Layout core unit tests):**
   - Tuyệt đối không đụng vào `layout.ts` $\implies$ 114 checks giữ nguyên.
3. **Parity Render:**
   - Không đụng vào logic font/line height của `render.ts` $\implies$ giữ nguyên kết quả đo.

---

## 7. Changelog

| Ngày | Nội dung | Commit |
| :--- | :--- | :--- |
| 2026-10-11 | Khởi tạo DD U01 mở màn Milestone A5 (T-A33) | *commit DD* |
