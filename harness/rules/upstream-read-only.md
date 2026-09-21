# Rule: upstream-read-only

## Vùng cấm

- `upstream/**` là vùng CHỈ ĐỌC tuyệt đối:
  - Không sửa / xóa / thêm file.
  - Không `git commit` bên trong submodule.
  - Không copy-and-patch code từ upstream ra ngoài rồi sửa (đó là fork — phá khả năng rebase).
- `git submodule status` phải luôn clean trước khi commit ở repo chính. Nếu submodule bị dirty
  → `git -C upstream/<tên> checkout .` để phục hồi, điều tra nguyên nhân, không commit "cho gọn".

## Khi cần thay đổi hành vi engine

Xử lý theo thứ tự ưu tiên:

1. **Config API** — sdkjs/web-apps có sẵn object config khi init editor (`DocsAPI.DocEditor(config)`):
   editing behavior, events, customization → đa số dùng được config, không cần đụng code.
2. **Adapter ở phía mình** — shim JS mỏng nằm trong `src/` hoặc bridge Rust, chặn/đổi dữ liệu
   TRƯỚC KHI vào engine hoặc SAU KHI ra khỏi engine (proxy `window.AscDesktopEditor` là vị trí phù hợp).
3. **Chỉ khi 1 và 2 bất khả thi**: mở issue/patch upstream riêng, cân nhắc cherry-pick có ghi nhận
   provenance (`docs/diagnostics.md` phải có entry patch + upstream issue link). Đây là ngoại lệ
   duy nhất, phải được user phê duyệt trước khi làm.

## Rebase upstream

- Định kỳ (khuyến nghị mỗi quý theo upstream release): 
  ```bash
  git submodule update --remote upstream/sdkjs upstream/web-apps
  git -C upstream/sdkjs log -1 --format='%h %cs'   # ghi version mới vào docs/diagnostics.md
  ```
- Sau rebase: chạy `harness/scripts/verify.sh` toàn bộ bridge-surface test trước khi commit.
- Conflict không xảy ra ở submodule (chúng ta không sửa chúng) — thứ gãy là shim của mình,
  sửa shim, không sửa upstream.
