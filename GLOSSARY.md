# GLOSSARY — WasenOffice

Ngôn ngữ chung của dự án cho vùng **trang bắt đầu và mở tài liệu**. Một danh từ = một khái niệm;
khi viết DD/TEST/code dùng đúng từ này. (Nhóm thuật ngữ khác — engine, converter, editor — bổ sung
khi có grill riêng.)

## Trang bắt đầu

**Startpage** (Trang bắt đầu):
Màn đầu tiên khi mở app không kèm file đối số — nơi tạo mới, mở file, quay lại tài liệu gần dùng.
_Avoid_: home page, landing, login page (upstream — đã strip).

**Welcome**:
Trạng thái của Startpage khi chưa có tài liệu nào mở: một cột căn giữa — tạo mới, Mở tài liệu,
Recents. Khi đã có tài liệu mở, Startpage chuyển sang vùng soạn thảo và Welcome không còn hiển thị.
_Avoid_: empty state, splash, home hero.

**Recents** (Gần đây):
Danh sách tài liệu mở thành công gần nhất, quản lý phía native; file đã mất khỏi đĩa hiển thị mờ
và gỡ được khỏi danh sách.
_Avoid_: history, recent files, recovery list.

**Recovers** (Khôi phục):
Tài liệu dở dang cần phục hồi sau sự cố — **đình chỉ trong v1** vì chưa có hạ tầng crash-recovery;
khác Recents ở chỗ Recents chỉ nhớ file đã mở, không giữ nội dung chưa lưu.
_Avoid_: auto-recovery, backup, Recents (không hoán đổi hai từ này).

## Mở tài liệu

**Lối mở** (open path):
Một trong ba cách đưa tài liệu vào app: **Open** (engine đầy đủ), **Quick view** (đọc-only),
**Nano Light** (editor nhẹ, docx). Mỗi nút hành động trên Startpage tự gắn với một lối mở.
_Avoid_: mode, viewer (chỉ đúng cho Quick view).

**Open dialog** (Hộp thoại mở):
Hộp thoại chọn file của hệ điều hành, lọc theo định dạng app nhận (.docx .xlsx .pptx .pdf), trả về
đường dẫn trên đĩa. Mỗi nút lối mở tự mở hộp thoại cho hành vi của nó — không có bước "chọn file
trước rồi bấm hành động".
_Avoid_: file picker, browse.

**Create-new** (Tạo mới):
Tạo tài liệu trắng DOCX/XLSX/PPTX từ Template blank; tài liệu ra đời là Untitled — chưa tồn tại
ở đâu trên đĩa.
_Avoid_: new document, blank document, sample.

**Template blank** (Bản mẫu trắng):
File Office rỗng đúng chuẩn, đóng gói sẵn trong app, làm nền cho Create-new. Là **tài liệu sinh
ra bằng công cụ**, không phải file lấy từ app nào.
_Avoid_: fixture (fixture là artifact cho test), sample, default file.

**Untitled** (Tài liệu chưa tên):
Tài liệu Create-new chưa được lưu tới đĩa; sống trong vùng làm việc của app cho tới lần lưu đầu.
_Avoid_: temp file, unsaved doc, draft.

**Save As**:
Lần lưu đầu của Untitled: chọn nơi lưu qua hộp thoại hệ điều hành, rồi mới ghi. Các lần lưu sau
ghi thẳng vào đích đã chọn, không hỏi lại.
_Avoid_: save dialog (chung chung), export.

## Chrome cửa sổ soạn thảo

**Chrome**:
Bộ UI do app tự vẽ bao quanh vùng tài liệu trong cửa sổ soạn thảo — header, menu tabs, khối
tài khoản. Khác với UI của engine (toolbar ngữ cảnh, panel trái) sống bên trong canvas engine.
_Avoid_: toolbar (dành cho Tools box), ribbon (khái niệm của MS/engine, không phải của mình).

**Menu tabs**:
Hàng tab bên trái Chrome (File, Home, Insert…) — điều khiển Tools box đang hiển thị. Không chứa
công cụ trực tiếp, chỉ là điều hướng.
_Avoid_: tab bar (dành cho multi-document tabs — slice A5 khác), ribbon tabs.

**Tools box**:
Thanh công cụ ngữ cảnh ngay dưới Chrome, hiển thị đúng bộ công cụ cho Menu tab đang chọn.
Của engine, điều khiển gián tiếp qua event công khai của engine.
_Avoid_: toolbar riêng lẻ không ngữ cảnh, ribbon.

## Phím tắt

**Keymap** (Bộ phím tắt):
Toàn bộ ánh xạ tổ hợp phím → hành động của app, là **một nguồn chân lý duy nhất** cho cả hai
nhóm: lệnh soạn thảo (bold, undo, save…) và hành động app (mở file, tab mới, Quick view…).
Mặc định tương thích MS Office; người dùng gán đè tổ hợp đã có thì phải xác nhận, gán mới
thắng và gán cũ rơi về mặc định.
_Avoid_: hotkey list, keybindings.json, shortcut profile (chỉ có một Keymap cho cả app).

**Key hint** (Gợi ý phím):
Chữ/tổ hợp phím hiển thị ngay trên nút để người dùng biết phím không cần đoán. Hiện theo cơ
chế **reveal**: giữ phím reveal là mọi nút sáng lên kèm hint, thả ra là mất — nghỉ thì UI
không chiếm chỗ. Luôn đọc từ Keymap — đổi gán là đổi hint, không bao giờ hiển thị phím đã
lỗi thời.
_Avoid_: badge thường trực, tooltip (chỉ hiện khi hover), placeholder.

**Cheat-sheet** (Bảng phím):
Khung xem toàn bộ Keymap hiện hành nhóm theo nhóm hành động, mở từ Settings — nơi tra cứu
khi Key hint không đủ.
_Avoid_: help page, manual.
