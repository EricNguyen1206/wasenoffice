# Rule: MIT purity — bảo vệ biên license của code tự viết

Mục tiêu (xem `docs/vision.md` Quyết định 1): code NanoOffice tự viết là **MIT, sạch vĩnh viễn**.
Rule này ràng buộc MỌI agent làm việc trong repo — agent chính lẫn subagent được spawn ra.

## Điều cấm tuyệt đối

1. **Không copy code/text từ `upstream/` vào code mình** — kể cả comment, docstring, bảng hằng số,
   regex, tên biến khối. License gắn vào text chứ không gắn vào ngôn ngữ: "dịch" JS → Rust dựa trên
   code AGPL vẫn tạo ra derivative (AGPL). Cần logic → re-implement từ spec chuẩn mở
   (ECMA-376 / ISO 29500 cho OOXML, ISO 32000 cho PDF) hoặc từ behavior quan sát được khi chạy thật.
2. **Không spawn subagent với prompt yêu cầu port/translate code upstream.** Agent cha chịu trách
   nhiệm kiểm prompt trước khi dispatch. Subagent KHÔNG tự động thừa kế AGENTS.md — prompt gửi đi
   phải chứa (hoặc trỏ tới đường dẫn) rule này. Output của subagent nghi ngờ giống upstream →
   coi là nhiễm, xóa và viết lại từ spec.
3. **Không thêm dependency có license copyleft mạnh** vào đồ dependency: cấm
   GPL-\*/AGPL-\*/SSPL-\*/LGPL-\*/EPL-\*/CDDL cho runtime. MPL-2.0 được phép (copyleft cấp file,
   giữ file tách bạch). Gate tự động: `harness/scripts/check-licenses.mjs` chạy trong `verify.sh`.
   Dependency mới phải có 1 dòng justification trong commit (tại sao cần, vì sao stdlib/cái có sẵn
   không đủ — đúng rung 5 trong ladder của skill `ponytail`).
4. **Không commit artifact engine**: `engines/**` là build của AGPL (sdkjs/web-apps/x2t) —
   giữ ngoài git; phân phối theo quyết định PH-4 trong `vision.md` (bundle vs download-first).
5. **Không giữ file từ `upstream/` trong `src/` hay `src-tauri/`** dưới mọi hình thức — kể cả file
   "tham khảo", `.bak`, "port của tôi của file X".

## Được phép (phân biệt biên)

- Tên API/method/format (`IsLocalFile`, `w:document`, tên element OOXML): KHÔNG có copyright —
  bridge contract giữ nguyên tên upstream là hợp lệ (rule gốc #4).
- Đọc upstream để HIỂU hành vi, rồi viết lại từ spec/behavior: hợp lệ (clean-room).
- Ghi path + mô tả bằng chứng vào `docs/reference.map.md`: luôn hợp lệ và bắt buộc (rule gốc #3).

## Kịch bản phát triển dự án — rủi ro MIT & guard

| # | Kịch bản | Rủi ro | Guard bắt buộc |
|---|---|---|---|
| S1 | Agent copy hàm từ `upstream/sdkjs` sang shim/Rust để "đỡ nghĩ" | derivative AGPL nhiễm cây MIT | điều 1; review diff bắt buộc với file chạm bridge; checklist `git log --stat` trước khi public |
| S2 | Thêm crate/npm tiện mà license AGPL/GPL | runtime dep copyleft | gate `check-licenses.mjs` fail → không commit được sạch |
| S3 | Subagent bị spawn với prompt "port hàm X từ upstream sang Rust" | subagent không đọc AGENTS.md vẫn làm theo | điều 2 — agent cha chịu trách nhiệm prompt; review output |
| S4 | Build sdkjs/web-apps/x2t vào `engines/` rồi commit luôn | nhúng AGPL artifact vào repo MIT | điều 4 + .gitignore + check staged trong verify.sh |
| S5 | Copy nguyên khối comment/docstring từ upstream | text cũng là copyright | điều 1 |
| S6 | Đóng gói font/dictionary đi kèm upstream | license riêng từng file | `licensing.md` checklist — kiểm từng thư mục fonts lúc PH-4 |
| S7 | Sửa trực tiếp upstream vì shim không đủ | vi phạm read-only + mất provenance | `upstream-read-only.md` — chỉ approval user mới mở đường patch |
| S8 | Phát hành binary có nhúng engine AGPL | nghĩa vụ AGPL của component đó | `licensing.md` § phát hành: NOTICE + source offer + logo §7 |
| S9 | Agent "giúp đỡ" viết lại engine sang Rust cho nhẹ/bịn license | derivative AGPL nếu port/translate; scope creep nếu tự ý | Từ 29.09 QĐ9 cho phép thay thế engine **theo Track A** — nhưng chỉ clean-room: `clean-room.md` (cấm đọc code module đang thay, chỉ spec + oracle); agent không được tự mở Track A task ngoài TODO |

## Enforcement tự động (trong `harness/scripts/verify.sh`)

- `check-licenses.mjs`: allowlist license cho toàn bộ cargo graph + node_modules — fail khi lệch.
- Check staged: không file nào từ `upstream/`, không file nào từ `engines/`.
- Similarity với upstream là việc review, không tự động được — thuộc trách nhiệm agent review diff.

## Khi nghi ngờ

Đánh dấu block bằng comment `// license-review:` + ghi entry vào `docs/diagnostics.md`, hỏi user.
Đừng đoán mò — đúng tinh thần evidence-discipline.
