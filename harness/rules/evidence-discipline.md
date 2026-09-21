# Rule: evidence-discipline

Mọi phát hiện về upstream phải phân tầng theo độ mạnh của bằng chứng:

| Trạng thái | Nghĩa là | Ghi nhận |
|---|---|---|
| `observed` | Thấy trực tiếp trong source/log/runtime (có path + dòng) | `docs/reference.map.md` |
| `candidate` | Cơ chế có thể giải thích nhưng chưa reproduce | `docs/diagnostics.md` mục "Candidates" |
| `reproduced` | Đã tái hiện trên input thật hoặc fixture tối thiểu | `docs/diagnostics.md` mục "Reproduced" |
| `confirmed` | Reproduced + counterfactual (bỏ nguyên nhân → hết triệu chứng) | `docs/diagnostics.md` + cập nhật skill/rule liên quan |

## Cấm

- Nâng cấp trạng thái khi tóm tắt (candidate → confirmed mà không có counterfactual).
- "Code nhìn giống nguyên nhân" / "hai agent đều đồng ý" — không phải bằng chứng root cause.
- Trình bày số liệu tự ước (tháng công việc, % RAM tiết kiệm) như số đo được.

## Bắt buộc khi báo cáo hoàn thành

1. Lệnh verify đã chạy + exit status.
2. Kết quả khác expect → nói rõ, không giấu.
3. Phần chưa verify → liệt kê, không mặc định là ổn.
