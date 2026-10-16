---
type: design
---
# Detail Design (DD) — A01 · Trang bắt đầu (startpage)

## 0. Metadata

| Trường | Giá trị |
|---|---|
| Mã màn hình | `A01` (nhóm A — trang chủ & thông tin chung) |
| Tên màn hình | `startpage` |
| Mã cha | — (màn chính) |
| Trạng thái | **active — v1 ratified 17.10.2026** (grill-with-docs, user chốt từng round) |
| Phase | **A5** (slice T-A38 + T-A39 — kéo lên từ PH-2 theo yêu cầu user 17.10) |
| Owner | Eric Nguyen |
| Cập nhật gần nhất | 2026-10-17 |
| TEST tương ứng | [`TEST.md`](TEST.md) |
| Thuật ngữ | [`GLOSSARY.md`](../../GLOSSARY.md) — Startpage, Welcome, Recents, Recovers, Lối mở, Open dialog, Create-new, Template blank, Untitled, Save As |

## 1. Tổng quan

Màn đầu tiên khi mở app không kèm file đối số: tạo tài liệu mới, mở file có sẵn, quay lại tài liệu
gần dùng — thay thế loginpage của upstream bằng own-UI (`src/main.ts`, theme Graphite DD U01).
Không login, không cloud, không mạng (local-first). v1 = **Welcome + Open dialog + Recents +
Create-new + i18n Startpage**; Recovers đình chỉ.

## 2. Bối cảnh & phạm vi

- **Goals (v1):** vào việc trong ≤ 2 thao tác; mở file bằng Open dialog của hệ điều hành (hết era
  gõ đường dẫn tay — bất cập user báo 17.10); Welcome nhìn "app thật" (không còn vùng trắng trơn).
- **Non-goals (v1):** Recovers (đình chỉ — hạ tầng crash-recovery chưa tồn tại), drag-drop file vào
  cửa sổ (slice sau), dark mode, tìm kiếm, templates marketplace, cloud (PH-3 cắt).
- **Quyết định liên quan:** QĐ local-first (dữ liệu không rời máy) · QĐ2 (budget nhẹ — Startpage
  không load engine) · grill 17.10 (user chốt Q1–Q12, ghi sẵn trong changelog).

## 3. Luồng người dùng & máy trạng thái

Entry: mở app không file đối số · thoát khỏi editor khi đóng tab/cửa sổ cuối.
Exit: mọi lối mở / Create-new → vùng soạn thảo trong **cùng cửa sổ chính** (không cửa sổ riêng —
giải câu hỏi mở cũ §10; đúng hành vi mount hiện tại).

| Trạng thái | Điều kiện vào | Chuyển tiếp | Dấu hiệu |
|---|---|---|---|
| FIRST_RUN | chưa có Recents | → NORMAL khi có file đầu tiên | Welcome không danh sách |
| NORMAL | có Recents | → OPENING khi chọn dòng / nút | danh sách Recents render |
| OPENING | chọn file / tạo mới qua lối mở nào | → exit sang vùng soạn thảo | mount như hiện tại |

## 4. Bố cục & hành vi trực quan (Welcome — 1 cột căn giữa, Q11)

Xuất hiện khi chưa có tài liệu mở; toolbar hiện tại chỉ quay lại khi vùng soạn thảo hoạt động.

1. **Logo + tên app + một câu định vị** (local-first).
2. **Hàng Create-new:** Tài liệu (DOCX) · Bảng tính (XLSX) · Trình chiếu (PPTX).
3. **Nút lớn "Mở tài liệu…"** (lối Open) + nút phụ "Quick view…" và "Nano Light…" — **mỗi nút tự
   mở Open dialog cho lối mở của nó** (Q4: không có bước chọn-trước, không state trung gian).
4. **Recents** (cap 10, Q10/Q11): dòng = loại + tên + đường dẫn rút gọn + mtime; file mất khỏi đĩa
   → dòng mờ, nhấn báo "không tìm thấy" + gợi ý gỡ; mỗi dòng có nút gỡ khỏi Recents.
5. **Chân:** phiên bản app + link nhỏ "hoặc dán đường dẫn…" mở ô path thu gọn — `#doc-path`
   **giữ nguyên khả năng probe cho gate 13** (Q5).

Ngôn ngữ UI: VI/EN theo `navigator.language`, fallback EN (Q6/Q12 — chỉ Startpage trong v1).

## 5. Thiết kế kỹ thuật

- **Open dialog:** `tauri-plugin-dialog` (open + save); filter `.docx/.xlsx/.pptx/.pdf`. Lý do chọn
  plugin thay vì `<input type="file">`: WKWebView trả `File` blob **không kèm đường dẫn đĩa**,
  trong khi `open_document` cần path thật (contract gates 9/13) — xem §9.
- **Recents:** 3 Tauri command riêng (`recents_list` / `recents_add` / `recents_remove`), store
  **JSON ở app-data dir** (Q10 — giải câu hỏi mở cũ §10: JSON, không SQLite). Ghi nhận khi open
  **thành công** từ cả ba lối mở. KHÔNG đi qua bridge `AscDesktopEditor` nhóm `LocalFileRecents`
  (Startpage là own-UI đi invoke trực tiếp; bridge chỉ đáng khi engine loginpage tái dùng — §9).
  Đường dẫn file KHÔNG bao giờ vào URL engine — registry fileId hiện có giữ nguyên vai trò.
- **Create-new:** 3 **Template blank** (docx/xlsx/pptx) sinh lúc dev bằng thư viện MIT
  (openpyxl / python-pptx — cùng pattern fixture `harness/fixtures/make_docx_*.py`), bundle vào
  app; bấm nút = copy template ra vùng làm việc Untitled → mở qua pipeline open hiện có.
  Không viết blank-writer mới cho crate (§9).
- **Untitled + Save As:** save handler chặn tài liệu đánh dấu `untitled` → NSSavePanel
  (`plugin-dialog` save()) → ghi tới đích + **re-key registry + title**; một seam chung áp cho cả
  engine save lẫn Nano Light save (Q8). Huỷ panel → giữ Untitled, không mất nội dung.
- **Entry code:** `src/main.ts` (nâng cấp tại chỗ; tách module khi phình — không tạo `src/startpage/`
  riêng chỉ để đúng hình).
- **Gates:** gate 13 giữ nguyên contract probe (`#doc-path` + `#quick-view-btn`); smoke Startpage
  (welcome render + Create-new — nút bấm được automate vì không qua NSOpenPanel) thường trụ
  verify.sh; Recents = unit Rust; Save As = seam unit với panel stub qua env + manual (NSOpenPanel
  không automate được — ceiling trung thực).

## 6. Xử lý lỗi & ca biên

| Tình huống | Hành vi mong đợi |
|---|---|
| Recent trỏ file đã xoá khỏi đĩa | dòng mờ; nhấn → báo "không tìm thấy" + gợi ý gỡ; không crash |
| Store Recents đọc hỏng | khởi tạo lại rỗng, log diagnostics; không crash Startpage |
| Extension lạ qua Open dialog | dialog lọc sẵn; path thủ công vẫn rơi vào thông báo lỗi hiện tại |
| Huỷ Open dialog / Save As | không có gì thay đổi; Untitled giữ nguyên nội dung |
| Template bundle thiếu/hỏng | nút Create-new báo lỗi rõ trên UI + log, không mở cửa sổ trắng |
| Quick view/Nano Light nhận format không hỗ trợ | giữ nguyên thông báo lỗi hiện tại (docx-only) |

## 7. Hiệu năng & ràng buộc

Startpage không load engine → tương tác được < 1 s cold start (đóng góp budget QĐ2 < 2 s).
Không network call nào. Recents cap 10 — cũ nhất rơi khỏi danh sách khi đầy.

## 8. Trợ năng & đa ngôn ngữ

Toàn bộ nhãn Startpage qua dict i18n VI/EN inline TS (`navigator.language`, fallback EN) — chỉ
Startpage trong v1 (Q12). Điều hướng bàn phím đầy đủ (Tab/Enter/Esc); focus ring rõ.

## 9. Phương án đã loại (Alternatives — quyết định grill 17.10)

| Phương án | Vì sao loại |
|---|---|
| `<input type="file">` HTML làm picker | WKWebView `File` không mang đường dẫn đĩa; `open_document` cần path thật |
| Bridge nhóm `LocalFileRecents` (bản DD gốc) | Startpage own-UI → invoke trực tiếp, ít moving parts; chỉ đáng khi engine loginpage tái dùng |
| Tạo file thật ngay trong `~/Documents` khi Create-new | viết vào đĩa user không có lời hỏi; Save As panel là chuẩn desktop (Q8) |
| Blank-writer crate cho xlsx/pptx | writer chưa tồn tại, mở rộng crate lớn; template data đủ cho v1 (Q7) — là **deviation có chủ đích**: tương lai thấy crate thiếu `write_xlsx` + app bundle .xlsx template là đúng thiết kế này |
| SQLite cho store Recents | không có nhu cầu query; JSON đủ (Q10) |
| Recovers minimal (quét workdir mồ côi lúc boot) | tạo kỳ vọng "phục hồi được" trên nền crash-recovery chưa đảm bảo (Q9) |
| Layout 2 cột (launcher) | Recents cap 10; 1 cột sạch — nâng cấp tự nhiên khi Recents thêm search/nhóm (Q11) |
| i18n trọn app v1 | ngoài ranh giới DD A01; chrome khác là slice riêng (Q12) |
| Loginpage upstream + strip cloud | violates local-first; rule read-only upstream (giữ từ bản draft) |
| Store Recents phía JS (localStorage) | đa cửa sổ khó đồng bộ; native là nguồn chân lý (giữ từ bản draft) |

## 10. Câu hỏi mở

Không còn — các câu hỏi mở của bản draft (store format, templates v1, cửa sổ riêng) đã giải trong
grill 17.10 và chốt ở §4/§5/§9. Provenance Template blank ghi trong commit sinh chúng.

## 11. Changelog

| Ngày | Thay đổi | Commit |
|---|---|---|
| 2026-09-26 | DD draft đầu tiên (spec định hướng trước khi code) | — |
| 2026-09-27 | Mã hóa màn hình: startpage → A01; tham chiếu editor → D01, settings → X01 | — |
| 2026-10-17 | **v1 ratified** (grill-with-docs 2 rounds): Welcome + Open dialog (plugin-dialog) + Recents (JSON app-data, 3 command riêng) + Create-new (Template blank ×3, MIT-generated) + Untitled/Save As seam + i18n VI/EN Startpage; Recovers ĐÌNH CHỈ; câu hỏi mở §10 giải hết | — |
