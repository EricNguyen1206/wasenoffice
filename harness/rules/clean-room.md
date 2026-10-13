# Rule: Clean-room discipline — điều kiện pháp lý của module thay thế engine

Kèm theo QĐ9 (roadmap tự chủ mã nguồn, `docs/vision.md`). Rule này ràng buộc MỌI agent
(agent chính + subagent) khi viết bất kỳ module nào thuộc **Track A** (thay thế x2t /
sdkjs / web-apps bằng code WasenOffice). Prompt subagent làm việc Track A PHẢI nhúng/trỏ
rule này — cùng xử lý với `mit-purity.md`.

## Nguyên tắc

License gắn vào **code gốc**, không gắn vào build (QĐ1). Module thay thế chỉ thực sự là MIT
khi viết **không dựa trên việc đọc code AGPL của module bị thay** — otherwise nó là
derivative dù viết bằng ngôn ngữ khác. "Clean-room" ở đây = chỉ 2 nguồn đầu vào hợp lệ:

1. **Spec chuẩn mở** — ECMA-376 / ISO 29500 (OOXML), ISO 32000 (PDF), MS-OI29500,
   MS-CMDISO… Ghi evidence dạng `spec-ref: <tên spec> §<điều>`.
2. **Quan sát black-box** — hành vi runtime của app thật: oracle CDP (kỹ thuật đã proven,
   `docs/diagnostics.md` 22.09 bổ sung), probing file format (magic bytes, cấu trúc binary
   quan sát được), diff input/output. Ghi evidence dạng `oracle-obs: <mô tả + lệnh tái lập>`.

## Điều cấm (từ phút viết module thay thế trở đi)

1. **Cấm mở/đọc/grep file upstream của module đang thay** — kể cả "chỉ xem thử cách họ làm".
   `mit-purity.md` cho phép đọc upstream để HIỂU hành vi chung; rule này SIẾT HƠN cho module
   đang được thay thế: hiểu hành vi phải qua spec + oracle, không qua source.
   - Ví dụ: đang viết converter Rust thay x2t (T-A20…) → cấm đọc `upstream/` phần converter
     core; hợp lệ: đọc spec ECMA-376, chạy x2t thật thu input/output, oracle CDP.
   - Ngoại lệ: file upstream KHÔC module đang thay (đang thay x2t → vẫn đọc web-apps UI
     để hiểu bridge) vẫn theo `mit-purity.md` như thường.
2. **Cấm "dịch" theo trí nhớ code đã đọc trước ngày rule này có hiệu lực** (29.09.2026).
   Module thay thế viết bởi agent đã từng đọc code module đó (các phiên reverse 22–28.09)
   → phần knowledge dùng phải là **hợp đồng hành vi đã ghi** trong `docs/reference.map.md`
   (call chain, byte-format, params XML — đều dạng oracle-obs/spec), không phải ký ức code.
   Đây chính là lý do reference.map tồn tại: nó là bản ghi clean-room an toàn.
3. **Cấm giữ file upstream mở cạnh editor** khi viết module thay thế (surface infection).
4. Vẫn kế thừa toàn bộ `mit-purity.md`: không copy text/tên biến khối, dependency qua
   license gate, v.v.

## Được phép

- **Tên API/method/format giữ nguyên** khi cần tương thích contract (bridge, element OOXML,
  tên param x2t như `m_sFontDir`): tên không copyright — tương tự bridge rule #4.
- Dùng **crates MIT/Apache/BSD/MPL** theo license gate (`check-licenses.mjs`).
- Đọc `docs/reference.map.md` + `docs/diagnostics.md` — kho hợp đồng hành vi đã ghi.

## Workflow bắt buộc cho task Track A

1. Mở task → ghi rõ trong diagnostics/handoff: module thay cái gì, 2 nguồn đầu vào
   (spec nào + oracle nào) sẽ dùng.
2. Mọi nhận định về module bị thay đổi từ `file:line upstream` sang `spec-ref:` / `oracle-obs:`.
3. Parity gate: module thay phải qua behavioral gate so với bản bị thay (pattern gate 9/10)
   trước khi khen xong — "chạy được" ≠ "thay được".
4. Khi nghi ngờ nhiễm: dừng, `// license-review:` + diagnostics + hỏi user — như mit-purity.

## Vì sao oracle CDP là hợp lệ

CDP evaluate quan sát **hành vi runtime của binary đã build** (như user bấm app) — không
trích source. Tương tự việc reverse một giao thức network bằng capture. Phân biệt: quan sát
runtime = hợp lệ; đọc source để bắt chước = nhiễm.
