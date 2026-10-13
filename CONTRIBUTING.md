# Đóng góp cho WasenOffice

WasenOffice là office suite desktop (DOCX/XLSX/PPTX/PDF) hướng **nhẹ + AI local-first**: shell
Rust + Tauri, tái sử dụng engine editing JS gốc của ONLYOFFICE (AGPL-3.0, submodule chỉ đọc),
x2t chạy sidecar. Phần code dự án tự viết dùng MIT (xem `LICENSE`).

## Bắt đầu

1. Đọc theo thứ tự: `AGENTS.md` (contract chung) → [`docs/README.md`](docs/README.md) (bản đồ
   tài liệu) → [`docs/conventions.md`](docs/conventions.md) (quy ước + workflow).
2. Môi trường dev: macOS (hỗ trợ Windows/Linux theo roadmap), Rust stable, Node + pnpm, git.
3. Clone kèm submodules: `git clone --recurse-submodules <repo-url>`.
4. `pnpm install` → chạy `./harness/scripts/verify.sh` — phải `ALL CHECKS PASSED` (exit 0)
   trước và sau khi bạn sửa code.
5. Engine artifact (`engines/`) không nằm trong git — hiện trích từ bản ONLYOFFICE Desktop
   cài sẵn (9.4.0): copy `engines/editors/` từ payload của app — bằng chứng + kiến trúc loopback
   ở [`docs/diagnostics.md`](docs/diagnostics.md) entry 22.09 (PH-0 đã đóng 27.09 —
   [`docs/TODO-archive.md`](docs/TODO-archive.md)).

## Luật quan trọng (chi tiết: `harness/rules/`)

- `upstream/` **CHỈ ĐỌC** — không sửa, không copy code/text từ upstream vào phần MIT của dự án
  (`harness/rules/mit-purity.md`).
- Mọi nhận định về upstream phải kèm đường dẫn file làm bằng chứng, ghi vào
  [`docs/reference.map.md`](docs/reference.map.md).
- Tên bridge method giữ nguyên văn upstream (`harness/skills/bridge-shim/SKILL.md`).

## Quy trình gửi đóng góp

- 1 PR = 1 việc. Branch `feat/<phase>-<mô-tả>`; commit message `phase(scope): mô tả`
  (vd `ph1(sidecar): spawn x2t với timeout`).
- Thay đổi hành vi màn hình → cập nhật DD + TEST của màn trong [`docs/design/`](docs/design/README.md)
  **cùng PR** (screenlist + quy tắc ở đó).
- Đính kèm output `verify.sh` (kèm exit code) vào PR — bằng chứng chạy thật, không claim suông.

## License

Đóng góp vào phần code tự viết của WasenOffice được phát hành theo MIT (`LICENSE`). Engine
upstream giữ nguyên AGPL-3.0 + provenance submodule — xem [`docs/vision.md`](docs/vision.md)
Quyết định 1 và `harness/rules/licensing.md`.
