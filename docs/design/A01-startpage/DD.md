# Detail Design (DD) — A01 · Trang bắt đầu (startpage)

## 0. Metadata

| Trường | Giá trị |
|---|---|
| Mã màn hình | `A01` (nhóm A — trang chủ & thông tin chung) |
| Tên màn hình | `startpage` |
| Mã cha | — (màn chính) |
| Trạng thái | draft (spec định hướng — chưa có code) |
| Phase | PH-2 (kế hoạch) |
| Owner | Eric Nguyen |
| Cập nhật gần nhất | 2026-09-27 |
| TEST tương ứng | [`TEST.md`](TEST.md) |

## 1. Tổng quan

Màn đầu tiên khi mở app không kèm file đối số: nơi tạo tài liệu mới, mở file có sẵn, quay lại
tài liệu gần dùng. Thay thế **loginpage** của upstream bằng JS riêng của NanoOffice (`src/startpage/`)
— không login, không cloud, không mạng (đúng định vị local-first + dữ liệu không rời máy).

## 2. Bối cảnh & phạm vi

- **Goals:** vào việc trong ≤ 2 thao tác (tạo mới / mở gần đây); hiển thị recents + recovers;
  khởi động lạnh nhanh (budget QĐ2: < 2 s đến tương tác được).
- **Non-goals:** đăng nhập/portal/cloud (PH-3 — mặc định cắt khỏi roadmap), templates marketplace,
  tìm kiếm toàn văn (thuộc lớp document intelligence, H3).
- **Quyết định liên quan:** QĐ định vị (local-first, dữ liệu không rời máy) · QĐ2 (budget nhẹ —
  startpage không load engine, chỉ shell) · strip loginpage/cloud (vision § Đòn bẩy cắt nhẹ).

## 3. Luồng người dùng & máy trạng thái

Entry: mở app không file đối số · thoát khỏi editor khi đóng tab/cửa sổ cuối.
Exit: mọi hành vi mở/tạo → cửa sổ soạn thảo D01 (editor — xem DD của D01).

| Trạng thái | Điều kiện vào | Chuyển tiếp | Dấu hiệu |
|---|---|---|---|
| FIRST_RUN | chưa có recents/recovers | → NORMAL khi có file đầu tiên | danh sách rỗng + onboarding tối thiểu |
| NORMAL | có recents | → OPENING khi chọn file | danh sách recents render |
| OPENING | chọn file / tạo mới | → exit sang editor | spinner nhẹ hoặc chuyển cửa sổ |

## 4. Bố cục & hành vi trực quan

- **Header:** logo + tên app + nút settings (X01, planned).
- **Vùng hành động:** 3 nút tạo mới — Tài liệu (DOCX) · Bảng tính (XLSX) · Trình chiếu (PPTX);
  1 nút "Mở file…" (system open dialog).
- **Vùng recents:** danh sách (icon loại, tên file, đường dẫn rút gọn, mtime); mỗi dòng có
  menu xoá khỏi recents; nhóm "Đã khôi phục" (recovers) hiển thị tách.
- Empty states: FIRST_RUN hiển thị 3 nút tạo mới nổi bật + 1 câu hướng dẫn.
- Resize: cửa sổ tối thiểu 720 px; danh sách cuộn độc lập.

## 5. Thiết kế kỹ thuật

- **Entry code:** `src/startpage/` (chưa tạo — theo kiến trúc mục tiêu `docs/conventions.md`).
- **Luồng dữ liệu:** UI JS → bridge method nhóm Recents → Rust đọc/ghi store recents ↔ trả JSON →
  render. Mở file: fileId đăng ký với Rust → mở cửa sổ editor (cùng cơ chế PH-0 open contract).
- **Interface tiêu thụ:** nhóm Recents bridge (hiện STUB, quy hoạch PH-2): `LocalFileRecents`,
  `LocalFileRecovers`, `LocalFileTemplates`, `LocalFileOpenRecent`, `LocalFileOpenRecover`,
  `LocalFileRemove*` ×4 — bằng chứng call-site: `docs/reference.map.md` § Bridge JS→Native
  (nhóm Recents). Events: `onupdaterecents`, `onupdaterecovers`, `onaddtemplates`
  (loginpage sdk.js:38-47).
- **Quản lý trạng thái:** recents là nguồn chân lý phía **Rust** (JSON store; vị trí/format —
  câu hỏi mở); UI stateless render từ dữ liệu bridge.
- **Điểm chạm upstream:** KHÔNG dùng loginpage upstream (chỉ tham chiếu hành vi qua reference.map);
  không load engine ở màn này.

## 6. Xử lý lỗi & ca biên

| Tình huống | Hành vi mong đợi | Bằng chứng |
|---|---|---|
| File recent đã bị xoá khỏi đĩa | dòng mờ + nhấn thì báo "không tìm thấy" + gợi ý remove | N/A (chưa code) |
| File recover hỏng (Editor.bin thiếu) | ẩn khỏi nhóm recovers, log diagnostics | N/A |
| Store recents đọc hỏng | khởi tạo lại rỗng, không crash startpage | N/A |

## 7. Hiệu năng & ràng buộc

Startpage không load engine → mục tiêu hiển thị < 1 s từ cold start (đóng góp vào budget
cold start < 2 s của QĐ2). Không network call nào.

## 8. Trợ năng & đa ngôn ngữ

Toàn bộ label qua i18n bundle riêng của `src/startpage/` (VI/EN lúc khởi tạo); điều hướng
đầy đủ bằng bàn phím (Tab/Enter/Esc); focus ring rõ.

## 9. Phương án đã loại (Alternatives)

| Phương án | Vì sao loại | Bằng chứng |
|---|---|---|
| Dùng loginpage upstream + strip cloud | violates định vị local-first; tăng payload; rule read-only upstream | QĐ định vị; `docs/conventions.md` kiến trúc mục tiêu |
| Recents store phía JS (localStorage) | đĩa path không được ra JS (kiến trúc); đa cửa sổ khó đồng bộ | `docs/conventions.md` quy ước code |

## 10. Câu hỏi mở

- [ ] Store recents: file JSON ở app-data dir hay SQLite? (quyết khi bắt đầu PH-2 — qua acceptance)
- [ ] Có hiển thị "templates" cục bộ ở v1 không, hay cắt (đã có nhóm bridge `LocalFileTemplates`)?
- [ ] Màn startpage có phải cửa sổ riêng hay tab đầu tiên của cửa sổ chính? (PH-2 cùng `window/`)

## 11. Changelog

| Ngày | Thay đổi | Commit |
|---|---|---|
| 2026-09-26 | DD draft đầu tiên (spec định hướng trước khi code) | — |
| 2026-09-27 | Mã hóa màn hình: startpage → A01; tham chiếu editor → D01, settings → X01 | — |
