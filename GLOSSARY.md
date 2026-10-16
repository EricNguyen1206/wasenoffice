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
