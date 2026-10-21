---
type: test
---
# Test Plan — A01 · Trang bắt đầu (startpage)

## 0. Metadata

| Trường | Giá trị |
|---|---|
| Mã màn hình | `A01` |
| DD tương ứng | [`DD.md`](DD.md) |
| Phase / điều kiện chạy | **A5 (T-A38 + T-A39)** · chạy khi Welcome/Recents/Create-new đã triển khai |
| Cập nhật gần nhất | 2026-10-17 |

## 1. Phạm vi & ma trận loại test

- **In scope (v1):** Welcome render, Open dialog, Recents (add/list/remove/mất file/persist restart),
  Create-new ×3, Untitled + Save As seam, i18n VI/EN, empty state.
- **Out of scope (v1):** Recovers (đình chỉ), drag-drop, nội dung bên trong editor (màn D01),
  i18n ngoài Startpage.
- Ma trận: ✅ `unit Rust` (store Recents, seam Save As) · ✅ `E2E gate` (smoke Startpage — Welcome
  + Create-new + path-probe gate 13; nút bấm được automate vì không qua NSOpenPanel) ·
  ✅ `E2E manual` (chỗ có NSOpenPanel/NSFontPanel hệ điều hành — ceiling trung thực) · ⬜ `visual`.

## 2. Môi trường & fixtures

App dev `pnpm tauri dev` (boot nền rule 6b khi agent chạy). Fixture: `harness/fixtures/make_docx*.py`
+ template blank ×3 (T-A39). Store Recents mẫu: mở vài fixture rồi đóng; xoá store ở app-data dir
để quay về FIRST_RUN.

## 3. Test cases

| ID | Ưu tiên | Loại | Truy vết DD | Given / When / Then | Bằng chứng mong đợi | Trạng thái |
|---|---|---|---|---|---|---|
| TC-A01-01 | P0 | E2E gate | §3 FIRST_RUN, §4 | Given store rỗng / When mở app / Then Welcome render đủ khối logo + 3 nút Create-new + Mở tài liệu + không Recents | smoke Startpage exit 0; log render | ✅ gate 17 (17.10) |
| TC-A01-02 | P0 | unit Rust | §5 Recents | add → list đúng thứ tự mtime; đầy 10 → cũ nhất rơi; remove → biến mất; store hỏng → reset rỗng | `cargo test` module recents | ✅ 6 tests (17.10) |
| TC-A01-03 | P0 | unit Rust | §5 Recents persist | add → restart (mở lại store) → list nguyên vẹn | test ghi/đọc lại file JSON | ✅ (17.10) |
| TC-A01-04 | P1 | unit Rust | §5 Recents | add trỏ file không tồn tại → list vẫn ghi nhận; UI hiển thị mờ (kiểm E2E manual) | test chứa path giả | ✅ unit; manual UI còn mở |
| TC-A01-05 | P0 | E2E manual | §3 OPENING (lối Open) | When bấm "Mở tài liệu…" / chọn file trên Open dialog / Then mount đúng như path-probe | NSOpenPanel không automate — manual + gate 13 làm chứng cùng handler | ✅ wiring qua gate 13/17; panel manual |
| TC-A01-06 | P0 | E2E gate | §4 Create-new ×3 | When bấm Tài liệu/Bảng tính/Trình chiếu / Then mở đúng doctype, nội dung trắng, title Untitled | gate 17-create (docx E2E trọn) + gate 18 oracle x2t cả 3 blank; E2E cell/slide còn mở | ✅ docx E2E + x2t ×3 |
| TC-A01-07 | P0 | unit Rust | §5 Untitled + Save As | Given doc untitled / When save / Then panel stub (env) nhận đường dẫn → ghi đích + re-key registry + title; huỷ panel → giữ untitled | gate 17-saveas E2E (marker trên đích + copy xoá + recents cập nhật); huỷ panel = manual | ✅ E2E stub (17.10) |
| TC-A01-08 | P1 | E2E manual | §5 Save As | When Cmd+S trên Untitled / Then NSSavePanel hiện; chọn chỗ / Then lưu + title đổi; lưu lần 2 không hỏi lại | quay màn hình; không crash | ⬜ T-A39 |
| TC-A01-09 | P1 | E2E manual | §6 file mất | Given recent trỏ file đã xoá / When nhấn / Then báo không tìm thấy + gợi ý gỡ; gỡ xong biến mất | thông báo hiển thị; không crash | ✅ gate 17-negative (17.10) |
| TC-A01-10 | P1 | E2E manual | §8 i18n | When đổi ngôn ngữ hệ thống / Then toàn bộ nhãn Startpage đổi VI↔EN | ảnh chụp 2 ngôn ngữ | ⬜ manual |
| TC-A01-11 | P0 | E2E gate | §7 perf | When cold start không file đối số / Then Welcome tương tác được < 1 s | smoke đo first-paint; gate boot hiện có | ✅ gate 17 ready-line tức thời (17.10) |
| TC-A01-12 | P2 | E2E manual | §8 a11y | When điều hướng thuần bàn phím / Then tới mọi action Welcome | thứ tự focus hợp lý, Esc thoát | ⬜ manual |
| TC-A01-13 | P0 | hồi quy | §5 gates | Mọi thay đổi Startpage / Then gate 13 (`#doc-path` + `#quick-view-btn`) vẫn GREEN | verify.sh FULL exit 0 | ✅ verify FULL 24 ▸ steps exit 0 (17.10) |
| TC-A01-14 | P0 | E2E manual | §5 Open dialog | When bấm "Xem nhanh…"/"Nano Light…" / Then panel chỉ offering .docx; bấm "Mở tài liệu…" / Then đủ 4 định dạng; .pdf mở được qua Open (engine) | panel filter nhìn thấy được; .pdf 26 MB (Downloads/123.pdf) contentReady ~3 s qua `WASEN_OFFICE_OPEN` (17.10) | ✅ filter code + PDF open GREEN; panel manual ceiling |
| TC-A01-15 | P0 | E2E gate | §4 Settings view (X01, T-A46) | When bấm sidebar Settings (hoặc ⌘,/gear) / Then main area chuyển sang view Phím tắt — 13 hàng keymap + capture chord (Esc hủy) + MCP status; Home/Settings active state đổi đúng; ✕ Đóng/Esc → về Home | gate 17-settings (probe `STARTPAGE_PROBE=settings` → log `settings view: keymap=N rows mcp=ok`) + browser-use E2E 21.10 (capture ⌘K, ✕ Đóng) | ✅ gate 17-settings + E2E (21.10) |
| TC-A01-16 | P1 | E2E manual | §4 AI view | When bấm sidebar "AI agent" / Then view "AI Agent & MCP Tools" (MCP chuyển từ Settings — 21.10); toggle client + copy config hoạt động | view render; toggle optimer qua `mcp_get_status` | ⬜ manual (toggle đã wired — K02/T-A41) |

## 4. Chính sách hồi quy

Sửa `src/main.ts` (Welcome/layout/i18n) → chạy smoke Startpage + gate 13 + TC-A01-11.
Sửa store Recents (Rust) → TC-A01-02/03/04. Sửa seam Save As → TC-A01-07 + gate save hiện có.
Sửa template bundle → TC-A01-06. Đóng góp tất cả vào verify.sh; không tuyên bố xong khi chưa
verify FULL exit 0.

## 5. Tiêu chí sign-off

- [x] Mọi P0 GREEN (unit + E2E gate) — 17.10
- [x] Gate 13 không thoái hóa — verify FULL 24 ▸ steps exit 0
- [x] Không network call nào trong phiên Startpage (log filesrv/network)
- [x] Store Recents hoạt động qua restart app — unit persist (TC-A01-03)
- [x] Manual ceiling ghi rõ trong báo cáo (NSOpenPanel/NSSavePanel + cancel + i18n + a11y)

## 6. Changelog

| Ngày | Thay đổi | Commit |
|---|---|---|
| 2026-09-26 | Test plan draft (viết trước theo DD — case N/A đến khi có code) | — |
| 2026-09-27 | Mã hóa màn hình: TC-SP-nn → TC-A01-nn | — |
| 2026-10-17 | **v1 theo DD ratified**: thêm unit Recents (02–04), Create-new ×3 probe (06), seam Save As (07–08), hồi quy gate 13 (13); đánh dấu Recovers out-of-scope; gate hoá chỗ automate được — NSOpenPanel ghi rõ là manual ceiling | — |
| 2026-10-17 | **Sink T-A38/T-A39**: P0 gần như ✅ — gate 17 (4 mode: pos/neg/create/saveas) + gate 18 (oracle x2t ×3); ceiling còn lại: NSOpenPanel/NSSavePanel thật, cancel-panel, i18n manual (10), a11y (12), E2E cell/slide create | b4c692f |
| 2026-10-21 | **T-A46**: +TC-A01-15 (Settings view — gate 17-settings mode thứ 5) + TC-A01-16 (AI view manual); cửa sổ Settings riêng + 2 tab giả Agents/Agent Names xoá (K02-Q3 amended) | — |
