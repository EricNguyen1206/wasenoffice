# Rule: licensing

## Provenance hai lớp

- **Code NanoOffice tự viết** (`src-tauri/`, `src/`, `harness/`, `docs/`): license dự án chọn
  (đặt LICENSE riêng khi quyết — mặc định làm việc như proprietary/private study cho đến khi user quyết).
- **Engine upstream** (`upstream/**`, `engines/**` artifact build): **AGPL-3.0** của ONLYOFFICE.
  submodule giữ nguyên LICENSE + provenance — đây là điều kiện để phần code của mình không bị coi
  là derivative của engine.

## Khi phát hành / phân phối

- Binary có nhúng engine AGPL (sdkjs/web-apps bundle, x2t sidecar) → PHẢI:
  1. Ghi rõ component AGPL + phiên bản + link source trong About / NOTICE.
  2. Cung cấp source tương ứng của component đó (đúng bản build).
  3. KHÔNG quảng bá "NanoOffice hoàn toàn proprietary" khi có component AGPL bên trong.
- Không copy nguyên file AGPL sang phần code của mình — cần logic thì re-implement từ spec
  (OOXML là ECMA-376/ISO chuẩn mở, spec không có license problem).
- Font/dictionary đi kèm upstream có license riêng (mở `upstream/web-apps/LICENSE`, kiểm tra từng
  thư mục fonts khi đóng gói).

## Checklist trước khi public repo / distribute

- [ ] `git submodule` clean, không có commit local nào trong upstream
- [ ] NOTICE / About ghi credit ONLYOFFICE + link repo gốc
- [ ] Không có file nào được copy từ `upstream/` sang `src/` hoặc `src-tauri/` (kiểm bằng `git log --stat`)
