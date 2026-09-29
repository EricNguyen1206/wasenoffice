---
type: registry
---
# docs/design/ — registry màn hình & tài liệu thiết kế

> Source of truth cho thiết kế (DD) + kiểm thử (TEST) từng màn hình — public official (`docs/vision.md` QĐ6).
> Mỗi màn có **mã ổn định** `[Chữ][2 số]` (vd `D01`); màn con thêm suffix `-n` (`D01-1`).
> Thư mục màn: `[Mã]-[tên-kebab]` (vd `D01-editor/`); tài liệu màn con nằm trong thư mục màn chính;
> màn dùng chung nhiều màn tách lên cụm **C** (common).

## Bảng cấp mã (chữ = nhóm chức năng)

| Chữ | Nhóm chức năng | Đã cấp | Ghi chú |
|---|---|---|---|
| **A** | Trang chủ & thông tin chung | A01, A02 | Intro, Terms of Service… cấp số tiếp khi làm |
| **B** | Tài liệu về phần mềm (hướng dẫn sử dụng, help) | — | chưa có màn |
| **C** | Màn dùng chung nhiều màn hình (common) | C01 | tách khỏi màn cha khi thật sự dùng chung |
| **D** | Các màn liên quan DOCX | D01 | |
| **E** | Các màn liên quan Excel (XLSX) | E01 | |
| **P** | Các màn liên quan PPTX | P01 | |
| **F** | Các màn liên quan PDF | F01 | cấp 29.09 (T-048) — 1 chữ đúng quy tắc `[Chữ][2 số]`, không dùng "PD" 2 chữ |
| **M** | Editor Markdown (kiểu Obsidian) | — | ngoài roadmap hiện tại — giữ namespace |
| **T** | Editor txt (kiểu Notepad++) | — | ngoài roadmap hiện tại — giữ namespace |
| **X** | Cài đặt | X01 | |

## Registry màn hình

| Mã | Màn hình | Cha | Trạng thái | Phase | DD | TEST | Entry code |
|---|---|---|---|---|---|---|---|
| A01 | Trang bắt đầu (startpage) | — | 🟦 draft | PH-2 | [DD](A01-startpage/DD.md) | [TEST](A01-startpage/TEST.md) | `src/startpage/` (chưa tạo) |
| A02 | Giới thiệu & credit license (about) | — | ⬜ planned | PH-4 (đóng gói) | [stub](A02-about/DD.md) | — | — |
| C01 | Xem trước khi in (print-preview) | — | ⬜ planned | PH-2 | [stub](C01-print-preview/DD.md) | — | — |
| D01 | Cửa sổ soạn thảo tài liệu (editor) | — | 🟨 in-development | PH-0 | [DD](D01-editor/DD.md) | [TEST](D01-editor/TEST.md) | `src-tauri/src/`, `src/main.ts`, `src/shim/` |
| E01 | Cửa sổ soạn thảo bảng tính (editor XLSX) | — | 🟨 in-development | PH-1 | [DD](E01-editor-xlsx/DD.md) | [TEST](E01-editor-xlsx/TEST.md) | `src/main.ts` (FILE_FORMATS), `src/shim/` |
| P01 | Cửa sổ soạn thảo bài trình bày (editor PPTX) | — | 🟨 in-development | PH-1 | [DD](P01-editor-pptx/DD.md) | [TEST](P01-editor-pptx/TEST.md) | `src/main.ts` (FILE_FORMATS), `src/shim/` |
| F01 | Cửa sổ xem/soạn PDF (editor PDF) | — | 🟨 in-development | PH-1 | [DD](F01-editor-pdf/DD.md) | [TEST](F01-editor-pdf/TEST.md) | `src/main.ts` (FILE_FORMATS), `src/shim/` |
| X01 | Cài đặt (settings) | — | ⬜ planned | chưa có QĐ | [stub](X01-settings/DD.md) | — | — |

## Vòng đời trạng thái

```
planned → draft → in-development → stable → deprecated
```

- **planned** — chiếm namespace + 1–2 dòng mô tả; chưa cam kết thiết kế.
- **draft** — spec định hướng, chưa khớp code (code chưa có hoặc chưa đạt).
- **in-development** — đang build; DD mô tả đồng thời đích và hiện trạng (kể cả điểm treo).
- **stable** — hành vi khớp DD, mọi test P0 GREEN; chỉ đổi qua PR kèm DD + TEST.
- **deprecated** — không còn dùng; giữ lại để tra cứu.

## Quy tắc

**Cấp mã & tổ chức thư mục**

1. Mã cấp **một lần, không tái sử dụng**; màn bỏ → dòng registry chuyển `deprecated` (không xoá).
2. Màn con (dialog/thành phần trong màn chính): mã `<mã cha>-n` (n = 1…9), tài liệu nằm trong thư mục màn cha.
3. Màn dùng chung nhiều màn → cụm C (common), không làm màn con của một màn.
4. Đổi tên màn = rename thư mục `[Mã]-[tên mới]` + sửa registry **cùng commit** — mã giữ nguyên.
5. Màn mới → copy 2 template vào `[Mã]-[tên]/`, đăng ký registry (cấp mã) **trước** khi viết spec.
6. Commit rename thư mục tách khỏi commit đổi nội dung (giữ git history dễ đọc).

**Nội dung**

7. Đổi hành vi màn hình → cập nhật DD + TEST **cùng commit** với code.
8. Mọi nhận định về upstream trong DD phải có path → **link** `docs/reference.map.md`, không copy bảng.
9. TEST là điều kiện sign-off màn: chỉ tính "xong" khi mọi case P0 GREEN (RED phải có lý do + câu hỏi mở trong DD §10). Case FAIL đang biết → giữ trạng thái RED — không xoá kệ.
10. ID test case: `TC-<mã màn>-nn` (vd `TC-D01-06`); ID câu hỏi mở: `OQ-<mã màn>-n` — duy nhất, không tái dùng.
11. Nhắc mã lần đầu trong bất kỳ doc nào phải kèm tên (vd "D01 (editor)").
12. Ngôn ngữ: tiếng Việt (QĐ6).
