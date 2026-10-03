---
type: registry
---
# docs/ — chỉ mục & định tuyến tài liệu

> File này là chỗ **duy nhất** liệt kê mọi tài liệu của dự án: doc không có dòng ở đây = chưa chính thức.
> Quy tắc tổ chức: `docs/conventions.md`. Quyết định về hệ tài liệu public: `docs/vision.md` QĐ6.

## Định tuyến — thông tin nào ghi vào đâu

| Loại thông tin | Ngôi nhà | Khi nào cập nhật |
|---|---|---|
| Quyết định sản phẩm/kỹ thuật (ADR) | `docs/vision.md` | ngay khi chốt/sửa quyết định, kèm entry `diagnostics.md` |
| Quy ước code, kiến trúc mục tiêu, workflow | `docs/conventions.md` | khi quy luật chung thay đổi |
| Bằng chứng về upstream (path, call-site) | `docs/reference.map.md` | ngay khi phát hiện mới |
| Sự cố, bẫy, số liệu đo | `docs/diagnostics.md` | ngay khi xảy ra (entry mới **trên cùng**) |
| Thiết kế chi tiết màn hình (DD) | `docs/design/[Mã]-[tên]/DD.md` | **cùng commit** với thay đổi hành vi màn hình |
| Kế hoạch kiểm thử màn hình (TEST) | `docs/design/[Mã]-[tên]/TEST.md` | cùng commit với DD / khi thêm test case |
| Danh mục màn hình, mã & cấp mã | `docs/design/README.md` (registry) | khi thêm màn / đổi trạng thái / cấp mã mới |
| Hàng đợi công việc (task vận hành) | `docs/TODO.md` | claim đầu phiên / release cuối phiên; đổi trạng thái anytime |
| Prompt bàn giao phiên | `docs/handoff/<tên>.md` (vòng đời viết/ghi tiếp/xoá: `docs/handoff/README.md`) | cuối phiên dở việc / cuối phase |
| Research một lần (không duy trì) | vd `docs/techstack-analysis.md` | không cập nhật — cố ý untracked |

Ngoài `docs/`: contract cho agent = `AGENTS.md` (root) · luật bắt buộc = `harness/rules/` · quy trình từng việc = `harness/skills/` · hướng dẫn đóng góp = `CONTRIBUTING.md` (root).

## Chỉ mục

| File | Vai trò |
|---|---|
| `docs/TODO.md` | hàng đợi công việc theo phase (QĐ7) — dashboard + quy tắc ở header |
| `docs/TODO-archive.md` | kho ✅ của các phase đã đóng (QĐ7) — cắt từ TODO.md khi đóng phase |
| `docs/handoff/` | prompt bàn giao phiên agent — README vòng đời (viết/ghi tiếp/xoá) + `_TEMPLATE.md` |
| `docs/conventions.md` | quy ước code + kiến trúc mục tiêu + workflow skills + chế độ đơn nhiệm |
| `docs/vision.md` | tầm nhìn + các quyết định đã chốt (QĐ1–QĐ6) — vai trò ADR |
| `docs/reference.map.md` | bản đồ bằng chứng upstream ↔ việc cần làm (bridge ~130 method, open contract, ràng buộc URL) |
| `docs/diagnostics.md` | nhật ký chẩn đoán: sự cố, bẫy, số liệu đo (entry mới trên cùng) |
| `docs/design/README.md` | registry màn hình: bảng cấp mã (A/B/C/D/E/P/F/M/T/X) + vòng đời trạng thái |
| `docs/design/_TEMPLATE-DD.md` | template Detail Design theo màn hình |
| `docs/design/_TEMPLATE-TEST.md` | template Test Plan theo màn hình |
| `docs/design/D01-editor/` | DD + TEST cửa sổ soạn thảo DOCX (PH-0) |
| `docs/design/E01-editor-xlsx/` | DD + TEST cửa sổ soạn thảo XLSX (PH-1, T-048) |
| `docs/design/P01-editor-pptx/` | DD + TEST cửa sổ soạn thảo PPTX (PH-1, T-048) |
| `docs/design/F01-editor-pdf/` | DD + TEST cửa sổ xem/soạn PDF (PH-1, T-048) |
| `docs/design/converter-a2/` | DD converter core A2 — Rust thay x2t, mã K01 (Track A, T-A20; component-level — không phải màn UI) |
| `docs/design/viewer-a3/` | DD viewer Rust đọc-only "quick view" A3 — mã V01 (Track A, T-A25; component-level — không phải màn UI) |
| `docs/design/editor-a4/` | DD editor core A4 "Nano Light" — mã L01 (Track A, T-A27; component-level — không phải màn UI) |
| `docs/design/A01-startpage/` | DD + TEST trang bắt đầu (draft, PH-2) |
| `docs/design/A02-about/` · `docs/design/C01-print-preview/` · `docs/design/X01-settings/` | stub màn kế hoạch (planned) |
| `docs/techstack-analysis.md` | research Bun/Go/Rust 26.09 (untracked theo QĐ stack) |

## Quy tắc chung

- **Một loại thông tin = một ngôi nhà.** Cấm copy nội dung giữa các nhà — chỉ cross-link.
- **OKF-thin (QĐ8):** mọi `docs/**/*.md` mang frontmatter `type` thuộc vocabulary cố định
  (bảng: `conventions.md` § Chuẩn tài liệu OKF-thin); doc mới phải có dòng index ở đây —
  gate `harness/scripts/check-docs.mjs` chặn vi phạm khi chạy verify.
- **Ngôn ngữ: tiếng Việt** cho mọi tài liệu dự án (chốt user 26.09, QĐ6); payload công cụ
  (System-1, fixture script…) giữ tiếng Anh.
- Mỗi tự khai báo protocol cập nhật ở header riêng (mẫu: `diagnostics.md`).
