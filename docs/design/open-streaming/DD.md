---
type: design
---
# Detail Design (DD) — T-067 · Open-delivery streaming + size cap

## 0. Metadata

| Trường | Giá trị |
|---|---|
| Mã | `T-067` (PH-1 — vòng đời file) |
| Loại nhiệm vụ | perf shell — không thuộc màn hình A-nào; DD nhóm mở `open-streaming` |
| Trạng thái | active — 17.10.2026 (triệu chứng user: RAM ~4.5GB + lag máy khi mở file nặng) |
| Owner | Eric Nguyen |
| Cập nhật gần nhất | 2026-10-17 |
| Điều tra | `docs/diagnostics.md` 17.10 (T-067) |

## 1. Vấn đề

Báo cáo user 17.10: mở app/file nặng → app ăn ~4.5GB RAM, máy lag. Số đo đã điều tra:

- **Start page sạch**: boot release, shell 90MB + 3 WebKit XPC ~91MB, ổn định xuyên suốt 60s.
  Culprit không nằm ở boot.
- **Culprit = lối MỞ TÀI LIỆU (engine)**: `get_document_for_delivery`
  (`src-tauri/src/lib.rs:228`) chở toàn bộ file qua IPC như base64 — một chuỗi nhân bản
  với mỗi lần mở:
  1. `std::fs::read` — N RAM (Rust)
  2. `base64::encode` — +1.33N (bytes gốc vẫn sống)
  3. `serde_json` serialize `DeliverableDocument { b64 }` — +1.33N
  4. Tauri IPC chuyển sang JS — +1.33N
  5. JS `atob(deliver.b64)` — +N (`delivery.js:273`)
  6. `DesktopOfflineAppDocumentEndLoad(..., b64, …)` — engine tự decode — +N

  → **peak logic ≈ 7–8× kích thước file**, TRƯỚC cả bộ nhớ engine parse (tính riêng).

  File của user: `~/Downloads/test-upload-1gb.pdf` 1.09GB → peak ~7–8GB logic,
  RSS ~4.5GB sau swap/compression của macOS — khớp report. `test.pdf` 49.9GB (cũng nằm
  trong recents) → nếu mở thì OOM/kill chắc chắn.

## 2. Contract upstream — streaming hook sẵn có (không đụng upstream)

`upstream/sdkjs/common/Local/common.js:184-197` — `window.DesktopOfflineAppDocumentEndLoad(url, data, len)`
chấp nhận **hai** dạng `data`:

1. base64 (đường hiện tại, `getBinaryArray(_data, len)` — decode trong engine);
2. **`binary_content://`** — nếu `data` bắt đầu `"binary_content://"` thì engine gọi
   `window["AscDesktopEditor"]["GetOpenedFile"](data)` và kỳ vọng nhận buffer;
   null → `ConvertationOpenError` Critical (fail-loud có sẵn).

Đây là hook streaming mà CEF shell thật dùng cho file lớn (GetOpenedFile trả native buffer).
WasenOffice cài cùng contract bằng shim thuần — **zero code upstream đụng tay** (QĐ9).

## 3. Quyết định

| # | Quyết định | Lý do |
|---|---|---|
| D1 | Delivery **bỏ b64** — Rust trả `{folder, name, src, docUrl, fileId, size}` (không bytes); JS fetch same-origin `/delivery/<id>/<name>` → `ArrayBuffer` stash → `EndLoad(folder, 'binary_content://<fileId>', size)`; engine gọi `GetOpenedFile` → shim trả stash `ArrayBuffer` (upstream tự `new Uint8Array(ab)` — view zero-copy) | triệt tiêu cả 4 lần nhân bản b64/JSON/IPC; JS còn đúng 1 bản N |
| D2 | `/delivery/<id>/<name>` là route mới `filesrv` (pattern T-V3): id = registry key; `name` phải KHỚP `DocWorkdir::delivered_name(flavor, disk)` — lệch/`..`/backslash → **403**; serve stream từ đĩa | tái dùng bảo mật traversal-proven của route media; disk path không vào URL |
| D3 | `serve()` chuyển sang **chunked write** (BufReader 64KB) — headers byte-identical, không allocation nguyên body | hiện `serve()` buffer `vec![0; len]` nguyên file → server thread tự ăn N; 1 nhà, bytes out không đổi |
| D4 | **Size cap** `WASEN_OFFICE_MAX_OPEN_MB` (default 250, `0` = không giới hạn, parse sai → fail-soft về default): file vượt cap bị TỪ CHỐI mở với error rõ ràng | streaming chỉ triệt tiêu multiplier của ta; RAM engine nội bộ (parse + structures + render ≈ 2–4×) KHÔNG triệt tiêu được không đụng upstream — file quá lớn phải refuse (case 49.9GB) |
| D5 | Cap áp 4 điểm: `register_and_url` (open + create + startup), `get_document_for_delivery` (defense), `viewer.rs render_payload` (Quick view + Nano Light — phủ cả path env `viewer_open`), error đi qua log contract `[web] ERROR: …` có sẵn | mỗi lối mở một điểm check duy nhất; entry command trọn đường boot |
| D6 | Log contract **giữ shape** — `delivering document to engine: <name> (N bytes, head a,b,c,d)` giử nguyên từng chữ; `probe decode:` giữ nguyên shape (probe đọc browser bytes thay vì b64) | gates 9/10/15 grep đối chiếu verbatim; "change in same commit" |
| D7 | **Replace, không layer**: đường b64 xoá hẳn (`atob`, `DeliverableDocument.b64`, `fs::read` delivery) — pipeline một mạch, gates phủ hồi quy | ponytail — dead path là bloat |

## 4. Triển khai (bản đồ file)

- `src-tauri/src/config.rs` — `max_open_bytes(lookup)` pure + env wrapper + unit test.
- `src-tauri/src/lib.rs` — `register_and_url` bắt cap; `get_document_for_delivery`:
  bỏ `fs::read`/b64, trả `docUrl` + `size` (serde camelCase — gotcha T-059).
- `src-tauri/src/protocol/filesrv.rs` — route `/delivery/…` + `serve()` chunked
  + route tests (byte-exact + 403 + 404).
- `src/shim/desktopEditor.js` — `GetOpenedFile` vào KNOWN_METHODS + LOCAL_TRUTHS (đọc stash).
- `src/shim/delivery.js` — fetch `docUrl` → stash → call `binary_content://`; xoá đường b64
  (cả probe — giữ log shape).
- `docs/reference.map.md` — row `GetOpenedFile` (bằng chứng common.js:190) + § open contract
  note `binary_content://` (clean-room note: EndLoad nhận 2 dạng).
- `harness/scripts/smoke-open-guard.sh` — **gate 19** (+ step verify.sh).

## 5. Criterion + Test (17.10 — kết quả)

| Check | Kỳ vọng | Kết quả |
|---|---|---|
| T-067-01 gates 9/10/11/12/13/15/17/18 | EXIT 0 nguyên trạng (regression net — log contract D6) | ✅ verify FULL exit 0 (mọi smoke gate pass nguyên trạng) |
| T-067-02 cargo test --workspace | EXIT 0 — +route tests filesrv + cap config tests | ✅ 71 shell + 8 crate pass (kèm fix 4 fixture rebrand lệch tên — pre-existing từ 13.10; flake song song env đã biết gặp 1 lần, pass khi chạy tuần tự) |
| T-067-03 **gate 19** `smoke-open-guard.sh` | refusal + counterfactual + green | ✅ PASSED ×2 (arm 1 refuse `open guard:`, arm 2 cap=0 guard vắng, arm 3 contentReady) |
| T-067-04 stream route unit test | byte-exact / 403 / 404 | ✅ `delivery_route_serves_only_the_expected_artifact` |
| T-067-05 Đo RAM theo open | AFTER ≤ ½ tree BEFORE | ✅ bảng §6 (5457 → 2463 MB) |
| T-067-06 verify FULL | EXIT 0 ×2 (step count mới) | ✅ ×2 exit 0 — **25 steps** (19 ▸ shell gates) |

## 6. Số đo (17.10 — xong)

Đo tree RSS qua `ps -axo rss,comm` (pattern PH-0), file `test-upload-1gb.pdf` 1.09GB,
boot nền rule 6b + `WASEN_OFFICE_OPEN`; AFTER chạy kèm `WASEN_OFFICE_MAX_OPEN_MB=0`
(tắt cap để đối chiếu cùng file; log cũng chốt `delivering … (1095049220 bytes) →
/delivery/1/…` + head PDF magic 37,80,68,70). Đích: AFTER ≤ ½ tree BEFORE → GREEN.

| Metric | BEFORE (b64 IPC) | AFTER (streaming) |
|---|---|---|
| shell peak RSS | ~3378 MB | **~131 MB, phẳng vĩnh viễn** (kể cả trong fetch 3–4s) — −96% |
| WebContent peak | ~5041 MB | ~2332 MB (engine wasm tự parse cả 1GB — ngoài lane shell, §7) |
| tree peak | 5457 MB | **~2463 MB trong cửa sổ open** (fetch + engine parse) — −55%, ≤ ½ before ✓ |
| tree plateau sau open | ~5.0 GB | ~2387 MB (engine PDF giữ toàn bộ doc ở error state) — −52% |
| user mặc định (cap 250MB) | mở được → cạn máy | **refuse `open guard: file too large to open: 300+ MB > cap 250 MB`** trước engine (log `[web] ERROR`, app sống — gate 19 arm 1) |

AFTER + gate 19 (refusal/counterfactual/green) đều GREEN; verify FULL exit 0 (25 steps).

## 7. Ceilings (trung thực, 17.10 đo xong)

- Engine nội bộ vẫn giữ N (parse) + structures render — streaming chỉ cầm chắc N=1 bản JS.
  **Đo thật (1.09GB PDF, cap mở): engine wasm vẫn dính `RuntimeError: Out of bounds memory
  access` + `asc_onError: -82` khi parse cả file — tree giữ ~2.4GB ở error state; window sống,
  thấy lỗi rõ ràng.** Tức là engine PDF sdkjs parse-heavy có ceiling riêng ~2.4GB tree với
  1GB file — ngoài lane shell T-067; cần ceiling thấp hơn thì hạ `WASEN_OFFICE_MAX_OPEN_MB`
  (env knob, không phải hardcode).
- Quick view (Rust-native) parse toàn bộ model vào RAM — cap là chặn đầu tiên; incremental
  parse/paging là việc riêng (pagination Quick view đang treo post-A3).
- File 49.9GB không bao giờ mở được chừng nào engine-side chưa có hook page-range —
  refuse + hint là hành vi đúng (gọi rõ trong message error).
