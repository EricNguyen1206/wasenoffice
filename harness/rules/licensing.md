# Rule: licensing

## Provenance hai lớp

- **Code WasenOffice tự viết** (`src-tauri/`, `src/`, `harness/`, `docs/`): **MIT** — `LICENSE` ở root
  (chốt 21.09.2026, mô hình đầy đủ trong `docs/vision.md`). LICENSE này KHÔNG áp dụng cho
  `upstream/**` và `engines/**`.
- **Engine upstream** (`upstream/**`, `engines/**` artifact build): **AGPL-3.0** của ONLYOFFICE.
  submodule giữ nguyên LICENSE + provenance — đây là điều kiện để phần code của mình không bị coi
  là derivative của engine.
- **License gắn vào code gốc, không gắn vào build:** rebuild/repackage artifact engine KHÔNG
  đổi được AGPL → MIT. Con đường duy nhất là clean-room rewrite từ spec (không nhìn code AGPL) —
  xem `docs/vision.md` § con đường full-MIT. Trong scope hiện tại việc này vẫn bị cấm.

## Khi phát hành / phân phối

- Binary có nhúng engine AGPL (sdkjs/web-apps bundle, x2t sidecar) → PHẢI:
  1. Ghi rõ component AGPL + phiên bản + link source trong About / NOTICE.
  2. Cung cấp source tương ứng của component đó (đúng bản build).
  3. KHÔNG quảng bá "WasenOffice hoàn toàn proprietary" khi có component AGPL bên trong.
- Không copy nguyên file AGPL sang phần code của mình — cần logic thì re-implement từ spec
  (OOXML là ECMA-376/ISO chuẩn mở, spec không có license problem).
- Font/dictionary đi kèm upstream có license riêng (mở `upstream/web-apps/LICENSE`, kiểm tra từng
  thư mục fonts khi đóng gói).

## Checklist trước khi public repo / distribute

- [ ] `git submodule` clean, không có commit local nào trong upstream
- [ ] NOTICE / About ghi credit ONLYOFFICE + link repo gốc
- [ ] Không có file nào được copy từ `upstream/` sang `src/` hoặc `src-tauri/` (kiểm bằng `git log --stat`)
