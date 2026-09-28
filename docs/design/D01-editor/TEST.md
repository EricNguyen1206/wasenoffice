---
type: test
---
# Test Plan — D01 · Cửa sổ soạn thảo tài liệu (editor)

## 0. Metadata

| Trường | Giá trị |
|---|---|
| Mã màn hình | `D01` |
| DD tương ứng | [`DD.md`](DD.md) |
| Phase / điều kiện chạy | PH-0 · cần engine artifact trong `engines/` (trích từ ONLYOFFICE.app 9.4.0 — quy trình ở diagnostics entry 22.09; PH-0 đã đóng 27.09 — `docs/TODO-archive.md`) · PH-1: save DOCX (TC-D01-10) |
| Cập nhật gần nhất | 2026-09-28 |

## 1. Phạm vi & ma trận loại test

- **In scope:** boot engine trong WKWebView, luồng open DOCX local, bridge first wave,
  hồi quy các bẫy đã gặp; **PH-1: luồng save DOCX về file gốc** (TC-D01-10).
- **Out of scope:** print/clipboard (PH-2), crypto (PH-3).
- Ma trận: ✅ `E2E manual` (app dev + console/filesrv log) · ✅ `unit` (Rust regression) ·
  ✅ `E2E auto` (gate 9 `smoke-render.sh` trong verify.sh — TC-D01-06) ·
  ✅ `E2E auto` round-trip save (gate 10 `smoke-save.sh` trong verify.sh — TC-D01-11) · ⬜ `visual` (PH-4 CI).

## 2. Môi trường & fixtures

- Engine artifact: `engines/editors/` trích từ ONLYOFFICE.app 9.4.0 (git-ignored) —
  bằng chứng nguồn + kiến trúc loopback ở diagnostics entry 22.09 (handoff ph0 đã xoá khi
  đóng PH-0 27.09).
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
| **TC-D01-06** | **P0** | E2E auto | §3 READY | When RENDERING chạy / Then `asc_onDocumentContentReady` + `id_viewer` height > 0 + font fetch `/fonts/…` | 3 dấu hiệu trên đồng thời | **GREEN** (27.09: contentReady + viewer 2302×958 + 14 fetch, `Hcc=15`; screenshot `docs/media/2026-09-27-tc-d01-06-render.png`; **tự động trong verify.sh gate 9** — `harness/scripts/smoke-render.sh`, diagnostics 27.09 T-042) |
| TC-D01-07 | P1 | unit | §6 NUL | When response body lớn / Then không nhân đôi NUL | unit test hồi quy trong filesrv pass (đã có từ 22.09) | GREEN |
| TC-D01-08 | P1 | E2E manual | §5 interface | When mở file / Then log mọi method `UNIMPL` bị gọi | danh sách UNIMPL thu hẹp theo từng wave — cập nhật `reference.map.md` | GREEN (danh sách còn ~110) |
| TC-D01-09 | P0 | E2E manual | §9 oracle | Given ONLYOFFICE.app thật + CDP / When mở cùng fixture / Then editor frame render canvas | `Asc.editor` tồn tại; canvas ~2654×1312 — dùng làm chuẩn diff trạng thái `AscFonts.Wg` | GREEN |
| **TC-D01-10** | **P0** | E2E manual | §5 open + save contract (`reference.map.md` § Desktop save contract) | Given fixture DOCX mở trong app (boot env `NANO_OFFICE_SAVE_PROBE=<text>`) / When shim probe chèn text qua `asc_AddText` + gọi `asc_Save()` (không keystroke — bẫy T-003) / Then file GỐC trên disk đổi và chứa text trong `word/document.xml`, mở lại render GREEN | size 1962→25613 B; `unzip -p … word/document.xml` grep marker = 1; save-end `code=0`; reopen: contentReady + viewer 1151×479 + 14 font fetch | **GREEN** (28.09 — `save delivered: … (25613 bytes)`; hợp đồng save + CDP oracle thật: `reference.map.md` § Desktop save contract, diagnostics 28.09 T-010) |
| **TC-D01-11** | **P0** | E2E auto | §5 open + save contract — tự động hoá TC-D01-10 (bằng chứng gốc thủ công) | Given fixture COPY fresh ở temp path + marker random env `NANO_OFFICE_SAVE_PROBE` / When gate 10 boot app → shim probe `asc_AddText`+`asc_Save()` (không keystroke — bẫy T-003) / Then log round-trip xuất hiện + file trên disk đổi (size+mtime) + `word/document.xml` chứa marker | PASS in 3 bằng chứng (log line `save-end delivered to engine: code=0`, byte delta 1962→25618 B, grep marker ×1); counterfactual `SMOKE_SAVE_NO_PROBE=1` phải FAIL theo timeout | **GREEN** (28.09 — gate 10 `harness/scripts/smoke-save.sh`; standalone ×2 exit 0 + negative exit 1; verify.sh 10 gates exit 0, diagnostics 28.09 T-011) |

## 4. Chính sách hồi quy

- Sửa bất kỳ file nào trong `src-tauri/src/{bridge,protocol}/` hoặc `src/shim/` → chạy lại
  **toàn bộ P0** (TC-D01-01 → 06, 09 → 11).
- Sửa `verify.sh` / `check-licenses.mjs` → chạy lại TC-D01-01.

## 5. Tiêu chí sign-off PH-0

- [x] Mọi P0 GREEN — TC-D01-06 chuyển GREEN 27.09 (blocker đã giải mã, diagnostics 27.09)
- [x] Bảng bridge: số method gọi / DONE / UNIMPL còn lại — cập nhật `reference.map.md` (27.09)
- [x] Baseline số liệu (disk/RAM/cold-start) ghi `diagnostics.md` (số 22.09; release đo ở PH-4)
- [x] Verdict go/no-go kèm bằng chứng — **T-003** · 27.09 · **GO WITH CAVEAT**: (a)(c)(d) đạt,
  (b) scroll/edit chờ user xác nhận thủ công ~30s (GUI bị chiếm dụng live khi đo — diagnostics
  27.09 T-003); baseline debug đã ghi, release đo ở PH-4

## 6. Changelog

| Ngày | Thay đổi | Commit |
|---|---|---|
| 2026-09-26 | Test plan đầu tiên — chuẩn hóa smoke protocol từ `conventions.md` + diagnostics | — |
| 2026-09-27 | Mã hóa màn hình: TC-ED-nn → TC-D01-nn | — |
| 2026-09-27 | TC-D01-06 GREEN + TC-D01-08 cập nhật danh sách UNIMPL | — |
| 2026-09-27 | TC-D01-06 → E2E auto: gate 9 smoke render trong verify.sh (T-042) | — |
| 2026-09-27 | §5 sign-off đủ — T-003 verdict GO WITH CAVEAT + baseline; tham chiếu handoff/ph0 → diagnostics/TODO-archive | — |
| 2026-09-28 | TC-D01-10 (P0) save DOCX round-trip GREEN — hợp đồng save + reverse x2t từ Editor.bin + changes (T-010) | — |
| 2026-09-28 | TC-D01-11 (P0) E2E auto — save round-trip vào verify.sh gate 10 `smoke-save.sh` (T-011) | 7ba9150 |
