# NanoOffice — AGENTS.md

> Harness cho dự án clone ONLYOFFICE Desktop Editors sang Rust + Tauri.
> Agent làm việc trong repo này BẮT BUỘC đọc và tuân thủ file này.

## identity & mục tiêu

- **Tên dự án:** NanoOffice
- **Mục tiêu:** app desktop office suite (DOCX/XLSX/PPTX/PDF) — shell Rust + Tauri,
  giữ engine editing JS gốc của ONLYOFFICE (sdkjs + web-apps), x2t chạy sidecar.
- **Không bao giờ port/rewrite engine editing sang Rust** trong scope hiện tại (xem `docs/conventions.md` § engine).

## Layout (nguồn chân lý về cấu trúc)

```
nanooffice/
├── AGENTS.md            ← file này — contract cho agent
├── docs/                ← kiến thức bằng chứng: reference.map, diagnostics, conventions
├── harness/
│   ├── rules/           ← quy tắc bắt buộc (migration, evidence, release)
│   └── skills/          ← quy trình từng việc cụ thể (open-docx, bridge-shim, ...)
├── upstream/            ← MẢNG CHỈ ĐỌC — chỉ để tra cứu bằng git/submodule, không sửa, không commit vào
│   ├── sdkjs/           (gitlink → ONLYOFFICE/sdkjs)
│   └── web-apps/        (gitlink → ONLYOFFICE/web-apps)
├── src-tauri/           ← MÃ NGUỒN CHÍNH của dự án (Rust: shell, bridge, sidecar)
├── src/                 ← frontend Tauri (start page, wrapper UI — KHÔNG sửa engine JS ở đây)
└── engines/             ← artifact build của sdkjs/web-apps (sinh ra, không edit tay)
```

## Quy tắc gốc (tóm tắt — chi tiết trong harness/rules/)

1. **Upstream chỉ đọc.** Mọi file trong `upstream/` không được sửa, không được copy-and-patch.
   Cần thay đổi hành vi engine → làm ở lớp wrapper/adapter phía mình (shim, monkey-patch qua config API).
   Lý do: giữ khả năng pull upstream theo quý. Xem `harness/rules/upstream-read-only.md`.
2. **Không sửa upstream → không vi phạm AGPL cho phần code của mình.** Phần code NanoOffice tự viết
   dùng **MIT** (`LICENSE` ở root); engine AGPL giữ nguyên provenance qua submodule. Khi PHÁT HÀNH binary có nhúng
   AGPL component → vẫn phải tuân thủ AGPL cho component đó (mở source component + ghi credit).
   Xem `harness/rules/licensing.md` và `docs/vision.md`.
3. **Mọi nhận định về upstream phải có đường dẫn file làm bằng chứng.** Ghi vào `docs/reference.map.md`.
   Cấm đoán mò. Xem `harness/rules/evidence-discipline.md`.
4. **Bridge là hợp đồng.** Mỗi method `window.AscDesktopEditor` phải có stub khai báo ở
   `src-tauri/src/bridge/` kèm marker `// UNIMPL` nếu chưa implement thật. Xem
   `harness/skills/bridge-shim/SKILL.md`.
5. **Verification trước khi tuyên bố xong.** Lệnh kiểm chứng + exit code phải được nêu trong
   báo cáo. "ZCode/chrome devtools nói chạy được" ≠ đã verify.
6. **MIT purity.** Không copy code/text từ `upstream/` vào code của mình (kể cả gián tiếp qua
   subagent); dependency mới phải qua license gate trong verify.sh; `engines/**` không commit.
   Xem `harness/rules/mit-purity.md`.

## Progress snapshot (cập nhật khi hoàn thành phase)

| Phase | Trạng thái | Ghi chú |
|---|---|---|
| PH-0 Spike (load editor + render DOCX trong WKWebView) | 🟨 đang làm — engine+bridge chạy, còn điểm treo open | engine boot OK trong WKWebView (không CEF); bridge 15+ method DONE-PH0; treo ở bước cuối open pipeline — xem `docs/diagnostics.md` 22.09 |
| PH-1 Vòng đời file (open/save/convert qua x2t sidecar) | ⬜ | |
| PH-2 Trải nghiệm desktop (print/clipboard/fonts/tabs) | ⬜ | |
| PH-3 Tùy chọn (crypto/signature/cloud) | ⬜ | có thể cắt vĩnh viễn |
| PH-4 Đóng gói + updater | ⬜ | |

## Việc còn treo

- **PH-0 đang chạy dở** (bắt đầu 22.09 từ `docs/handoff-ph0.md`): engine boot + bridge first
  wave OK, còn treo render — 3 phương án chờ user chọn, xem `docs/diagnostics.md` entry
  "22.09 (bổ sung)".
- Stack đã chốt: **giữ Rust + Tauri** (re-validate 26.09 — `docs/techstack-analysis.md`).

## Khi bắt đầu một phiên làm việc

1. Đọc `docs/vision.md` (tầm nhìn + quyết định sản phẩm) và `docs/conventions.md` (quy ước code + kiến trúc mục tiêu).
2. Việc mới → kiểm tra `harness/skills/` có skill tương ứng không trước khi tự làm.
3. Sửa code xong → chạy `harness/scripts/verify.sh` (nếu tồn tại) trước khi báo complete.
4. Phát hiện điều mới về upstream → ghi bằng chứng vào `docs/reference.map.md`, đừng để chỉ trong chat.
