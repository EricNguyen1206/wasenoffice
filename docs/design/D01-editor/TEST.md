# Test Plan — D01 · Cửa sổ soạn thảo tài liệu (editor)

## 0. Metadata

| Trường | Giá trị |
|---|---|
| Mã màn hình | `D01` |
| DD tương ứng | [`DD.md`](DD.md) |
| Phase / điều kiện chạy | PH-0 · cần engine artifact trong `engines/` (xem `docs/handoff/ph0.md` bước 2) |
| Cập nhật gần nhất | 2026-09-26 |

## 1. Phạm vi & ma trận loại test

- **In scope:** boot engine trong WKWebView, luồng open DOCX local, bridge first wave,
  hồi quy các bẫy đã gặp.
- **Out of scope:** save/convert UI (PH-1), print/clipboard (PH-2), crypto (PH-3).
- Ma trận: ✅ `E2E manual` (app dev + console/filesrv log) · ✅ `unit` (Rust regression) ·
  ⬜ `integration` tự động round-trip (PH-1, thêm vào `verify.sh`) · ⬜ `visual` (PH-4 CI).

## 2. Môi trường & fixtures

- Engine artifact: `engines/editors/` trích từ ONLYOFFICE.app 9.4.0 (git-ignored) —
  hướng dẫn `docs/handoff/ph0.md` bước 2.
- Fixture DOCX: `harness/fixtures/make_docx.py` (fixture chuẩn 1,9 KB, có tiếng Việt).
- Oracle chéo: ONLYOFFICE.app thật + CDP `--remote-debugging-port=9222`, eval trong editor
  iframe (`frames[0]`) — quy trình `docs/diagnostics.md` entry 22.09 (bổ sung).
- Chạy: `pnpm tauri dev` (devUrl :1420; filesrv loopback :14321).

## 3. Test cases

| ID | Ưu tiên | Loại | Truy vết DD | Given / When / Then | Bằng chứng mong đợi | Trạng thái |
|---|---|---|---|---|---|---|
| TC-D01-01 | P0 | unit | §6 | Given repo sạch / When chạy `./harness/scripts/verify.sh` / Then mọi gate pass | `ALL CHECKS PASSED`, exit 0 | GREEN |
| TC-D01-02 | P0 | E2E manual | §3 ENGINE_BOOT | Given app mở / When engine boot / Then UI ribbon/toolbar hiện, 0 RuntimeError | UI hiện; console không có `Aborted`; filesrv log hàng trăm `200` | GREEN |
| TC-D01-03 | P0 | E2E manual | §5 LoadJS | When bootstrap gọi `LoadJS(sdk-all.js, fonts.js, ChartStyles.js)` / Then eval xong + `completeLoad` | 3 payload resolve (0 × 404); `completeLoad` bắn | GREEN |
| TC-D01-04 | P0 | E2E manual | §6 wasm | When payload request sai thư mục / Then filesrv redirect về vị trí chuẩn | 0 × `both async and sync fetching of the wasm failed`; `fonts.wasm` init OK | GREEN |
| TC-D01-05 | P0 | E2E manual | §5 open 1–3 | When mở fixture DOCX / Then `LocalStartOpen` → x2t → `EndLoad` nhận b64+len đúng | convert 28–116 ms; probe decode head `DOCY`; `SetDocumentName` bắn | GREEN |
| **TC-D01-06** | **P0** | E2E manual | §3 READY | When RENDERING chạy / Then `asc_onDocumentContentReady` + `id_viewer` height > 0 + font fetch `/fonts/…` | 3 dấu hiệu trên đồng thời | **RED** — blocker PH-0 (DD §10 câu hỏi 1) |
| TC-D01-07 | P1 | unit | §6 NUL | When response body lớn / Then không nhân đôi NUL | unit test hồi quy trong filesrv pass (đã có từ 22.09) | GREEN |
| TC-D01-08 | P1 | E2E manual | §5 interface | When mở file / Then log mọi method `UNIMPL` bị gọi | danh sách UNIMPL thu hẹp theo từng wave — cập nhật `reference.map.md` | GREEN (danh sách còn ~115) |
| TC-D01-09 | P0 | E2E manual | §9 oracle | Given ONLYOFFICE.app thật + CDP / When mở cùng fixture / Then editor frame render canvas | `Asc.editor` tồn tại; canvas ~2654×1312 — dùng làm chuẩn diff trạng thái `AscFonts.Wg` | GREEN |

## 4. Chính sách hồi quy

- Sửa bất kỳ file nào trong `src-tauri/src/{bridge,protocol}/` hoặc `src/shim/` → chạy lại
  **toàn bộ P0** (TC-D01-01 → 06, 09).
- Sửa `verify.sh` / `check-licenses.mjs` → chạy lại TC-D01-01.

## 5. Tiêu chí sign-off PH-0

- [ ] Mọi P0 GREEN — hiện còn TC-D01-06 RED (blocker font pipeline, DD §10)
- [ ] Bảng bridge: số method gọi / DONE / UNIMPL còn lại — cập nhật `reference.map.md`
- [ ] Baseline số liệu (disk/RAM/cold-start) ghi `diagnostics.md`
- [ ] Verdict go/no-go kèm bằng chứng — NO-GO thì dừng chờ user, không tự đổi kiến trúc

## 6. Changelog

| Ngày | Thay đổi | Commit |
|---|---|---|
| 2026-09-26 | Test plan đầu tiên — chuẩn hóa smoke protocol từ `conventions.md` + diagnostics | — |
| 2026-09-27 | Mã hóa màn hình: TC-ED-nn → TC-D01-nn | — |
