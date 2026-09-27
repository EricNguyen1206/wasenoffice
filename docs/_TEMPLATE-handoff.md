# Handoff — <tên việc> (dành cho session mới)

<!-- Copy file này vào docs/handoff-<tên>.md rồi điền. Handoff là PROMPT cho phiên mới:
     mọi giả định phải thành chữ trong file — không được nằm trong chat. -->

> Bạn (agent) đang ở phiên mới. File này LÀ prompt công việc: đọc nó + các file nó trỏ tới
> theo thứ tự, rồi triển khai. Đi tuần tự, không nhảy cóc.

## Nhiệm vụ

**<1 đoạn: việc gì, tại sao, thuộc phase/TODO-task nào (T-xxx)>.**

## Bước 0 — đọc trước khi làm bất cứ gì (theo thứ tự)

1. `AGENTS.md` — contract của dự án.
2. `docs/vision.md` — các quyết định đã chốt (đối chiếu, không re-litigate).
3. `docs/conventions.md` — quy ước + vòng đời task + chế độ đơn nhiệm.
4. `<tài liệu chuyên biệt: DD màn (docs/design/[Mã]-.../), rules, harness skills, reference.map.md>`
5. `docs/diagnostics.md` — <n> entry mới nhất (bẫy đã gặp).
6. `docs/TODO.md` — claim đúng task này (🟨 + ngày) trước khi làm.

## Trạng thái repo lúc handoff

- Branch `<branch>` · working tree <sạch / mô tả>.
- <services đang chạy, engine artifact, điều kiện đặc biệt>

## Các bước triển khai

1. <numbered, tuần tự>

## Ràng buộc (không thương lượng)

- Repo: đường dẫn tuyệt đối `<...>`.
- `upstream/**` chỉ đọc; KHÔNG copy code/text upstream (mit-purity).
- Spawn subagent → prompt PHẢI nhúng/trỏ `harness/rules/mit-purity.md`.
- Giao tiếp với user bằng tiếng Việt.

## Định nghĩa hoàn thành

- <done-criteria measurable: lệnh kiểm chứng + exit code + evidence mong đợi>
- `./harness/scripts/verify.sh` exit 0 + bằng chứng trong báo cáo.
