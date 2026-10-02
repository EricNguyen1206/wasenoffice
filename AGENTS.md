# NanoOffice — AGENTS.md

> Harness cho dự án clone ONLYOFFICE Desktop Editors sang Rust + Tauri.
> Agent làm việc trong repo này BẮT BUỘC đọc và tuân thủ file này.

## identity & mục tiêu

- **Tên dự án:** NanoOffice
- **Mục tiêu:** app desktop office suite (DOCX/XLSX/PPTX/PDF) — shell Rust + Tauri,
  giữ engine editing JS gốc của ONLYOFFICE (sdkjs + web-apps), x2t chạy sidecar.
- **Thay engine chỉ bằng clean-room, không port/translate** (QĐ9, 29.09.2026): cấm mang code
  AGPL sang code mình dưới mọi hình thức; thay thế từng module theo Track A chỉ từ
  **spec mở + oracle quan sát** — đọc `harness/rules/clean-room.md` trước khi đụng task Track A.
  Engine AGPL giữ vai trò reference + "chế độ nâng cao" đến tận rung A6 (user chốt 29.09).

## Layout (nguồn chân lý về cấu trúc)

```
nanooffice/
├── AGENTS.md            ← file này — contract cho agent
├── docs/                ← kiến thức bằng chứng: reference.map, diagnostics, conventions
│   ├── design/          ← DD + TEST theo màn hình (source of truth UI, public — QĐ6)
│   └── handoff/         ← prompt bàn giao phiên agent (viết/ghi tiếp/xoá theo README ở đó)
├── harness/
│   ├── rules/           ← quy tắc bắt buộc (upstream-read-only, licensing, mit-purity, clean-room, evidence-discipline; migration/release sinh theo phase)
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
| PH-0 Spike (load editor + render DOCX trong WKWebView) | ✅ ĐÓNG 27.09 — verdict **GO WITH CAVEAT** | render GREEN tự động (verify.sh gate 9, TC-D01-06); baseline debug 27.09: RAM 1 doc ~561–579 MB vs ONLYOFFICE 570 MB (start page); caveat duy nhất: scroll/edit keyboard chờ user xác nhận thủ công ~30s — `docs/diagnostics.md` 27.09 T-003 |
| Track A rung A0+A1 (QĐ9 — tự chủ shell + phân phối) | ✅ A1 ĐÓNG 02.10 | **NanoOffice phân phối độc lập**: chạy trọn open→edit→save KHÔNG cần ONLYOFFICE.app cài sẵn — x2t bundled (T-A10, 113 MB) + AllFonts tự sinh (T-A11) + fonts bundled có license (T-A12) + gate 11 "sạch ONLYOFFICE" thường trụ verify.sh (T-A13: strict clean-machine + negative proven + test tích thể mv app → verify FULL exit 0 khi app ẨN); kế tiếp A2 (T-A20 — converter Rust, clean-room ECMA-376) |
| PH-1 Vòng đời file (open/save/convert qua x2t sidecar) | ⬜ | |
| PH-2 Trải nghiệm desktop (print/clipboard/fonts/tabs) | ⬜ | |
| PH-3 Tùy chọn (crypto/signature/cloud) | ⬜ | có thể cắt vĩnh viễn |
| PH-4 Đóng gói + updater | ⬜ | |

## Việc còn treo

Hàng đợi công việc sống ở **`docs/TODO.md`** (QĐ7 — quy tắc + dashboard ở đầu file đó).
Điểm nổi bật hiện tại: **RUNG A1 ĐÓNG 02.10 (T-A10→T-A13) — NanoOffice phân phối độc lập**:
chạy trọn open→edit→save KHÔNG cần ONLYOFFICE.app cài sẵn — x2t bundled (113 MB) +
AllFonts.js tự sinh + fonts bundled có license + **gate 11 "sạch ONLYOFFICE" thường trụ**
trong verify.sh (strict clean-machine mode `NANO_OFFICE_NO_APP_FALLBACK`; positive exit 0 +
negative proven exit 1) — bằng chứng đóng rung: test tích thể mv app → verify.sh FULL exit 0
khi app ẨN (`harness/scripts/clean-machine-proof.sh`, diagnostics 02.10). Runtime dependency
bắt buộc lên app: **0** (fallback x2t = WARN dev-only, bị strict mode chặn; upstream submodule
= reference theo D1 tới A6).
Kế tiếp: **T-A20** (mở màn A2 — Converter Rust thay x2t, bắt đầu DOCX, clean-room
ECMA-376 — chỉ spec-ref + oracle-obs) hoặc PH-1 còn lại (T-046/T-047/T-049 save multi-format).
Trước đó: T-A12 02.10 — font bundle `engines/fonts` + trim sdkjs-plugins; T-A11 30.09 —
AllFonts tự sinh Rust (clean-room đầu tiên); T-A10 30.09 — x2t bundled; T-048 29.09 mở cả 4
định dạng + T-050 30.09 fix gate pptx flaky.
- Stack đã chốt: **giữ Rust + Tauri** (re-validate 26.09 — `docs/techstack-analysis.md`).

## Khi bắt đầu một phiên làm việc

1. Đọc `docs/vision.md` (tầm nhìn + quyết định sản phẩm) và `docs/conventions.md` (quy ước code + kiến trúc mục tiêu).
2. Đọc `docs/TODO.md` → **claim đúng 1 task** (🟨 + ngày); không đụng task 🟨 của session khác. Task có handoff → đọc theo Bước 0 của handoff đó (`docs/handoff/`). Cuối phiên: cập nhật trạng thái + dashboard + dòng ▶.
3. Việc mới → kiểm tra `harness/skills/` có skill tương ứng không trước khi tự làm.
4. Sửa code xong → chạy `harness/scripts/verify.sh` (nếu tồn tại) trước khi báo complete.
5. Phát hiện điều mới về upstream → ghi bằng chứng vào `docs/reference.map.md`, đừng để chỉ trong chat.
