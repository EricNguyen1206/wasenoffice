---
type: design
---
# Detail Design (DD) — L01 · Editor core A4 "Nano Light" (Track A, rung A4)

> Trạng thái: **draft** (mở màn A4 — T-A27, 03.10.2026). Tài liệu kiến trúc
> component-level (không phải màn UI): TEST không tách file — kế hoạch kiểm thử ở §7,
> sống trong code + gate 15 + harness. Mã nhóm **L** cấp cho editor core (quy tắc
> `[Chữ][2 số]`; folder đặt `editor-a4/` theo handoff T-A27 — tiền lệ K01/V01).
> Criterion đóng RUNG A4 = parity matrix theo lớp tính năng (các task sau); vòng này
> chỉ cần vòng edit đầu tiên chạy thật + gate mới xanh.

## 1. Bối cảnh + mục tiêu

Rung A4 (QĐ9 `docs/vision.md`): **editor core riêng "Nano Light"** cho DOCX subset
hằng ngày — engine ONLYOFFICE giữ vai trò "chế độ nâng cao" (D1 — không hard-cut tới
A6); **mặc định mở file VẪN là engine** (quyết định default-switch thuộc A4 muộn/A5).
Vòng 1 (task này): caret → gõ → xoá → enter → undo/redo → dirty → **save thẳng bằng
`write_docx`** — KHÔNG Editor.bin, KHÔNG x2t trên đường Nano Light (đúng nghĩa "thay
save-side cho đường riêng" mà DD K01 §2 P2 hẹn "giữ x2t đến A4").

Nền có sẵn, không viết mới:
- crate `nanooffice-ooxml` (K01): `Document` (CHƯA-resolve — bản ghi-ngược-được),
  `resolve() → RenderDoc`, `write_docx(&Document, media_src)` round-trip bất biến +
  media byte-exact (T-A22/T-A26 đã proven).
- `src/viewer/layout.ts` (T-V7): `layoutBlocks → LineModel` + `hitTest(x,y) →
  {para,line,run,offset}` — **parity asset ε=2.0px (OQ-V01-1); editor CONSUME
  nguyên vẹn, fork = fail** (constraint review #4 C1).
- FileRegistry `register(owner=window-label)` (T-059) + route `/media/<fileId>/<part>`
  (T-V3) — ảnh theo fileId, không disk path trong URL.
- Pattern V5 (window + stash + nút start page) và pattern save atomic tmp+rename
  (`bridge/save.rs:251-254,323`).

Deps review kiến trúc #4 (03.10, diagnostics) nạp vào DD này: **C2** DocSession
window-scoped + undo + save-đường-crate; **C3** command layer song song delivery;
**C1** (T-V7) đã land.

## 2. Phương án — các quyết định chốt

| Quyết định | Phương án chọn | Phương án loại (lý do) |
|---|---|---|
| Nhà cho session | **`EditorSessions`: map `window-label → EditorSession`** (state Tauri mới, song song `QuickViewRequest`) | Thay `QuickViewRequest` bằng session map chung: quick-view là single-slot take-once được gates 12/13 sở hữu — đụng nó chỉ rủi ro regression, không lời (handoff cho phép "hoặc giữ nguyên quick-view, DD chốt") |
| Model sống đâu | **Rust-side**: `EditorSession` giữ `Document` CHƯA-resolve; mỗi mutation → `resolve()` lại → trả `RenderDoc` cho JS vẽ lại | Giữ model JS-side: mất gốc ghi-ngược (RenderDoc đã flatten §17.7.2 — ghi ngược là mất pStyle/rStyle/docDefaults, review #4 C2); undo/dirty phải tự làm lại ở TS |
| Undo | **Snapshot-clone `Document` mỗi mutation** (model nhỏ, clone rẻ), cap 100, bản snapshot kèm caret `(para,run,offset)` để undo/redo trả caret đúng chỗ | Inverse-command machinery: từng lệnh phải viết inverse — phình, đúng kiểu ponytail chống |
| Redo | **Có** — stack redo (snapshot + caret), mutation mới clear redo (~4 dòng, dưới ngưỡng ~30 dòng của handoff) | Không redo: đã rẻ, cắt đi không tiết kiệm gì |
| Dirty | **So bậc `Document != saved`** (`PartialEq` có sẵn; `saved` = clone lúc save) → undo về đúng điểm save tự về sạch | Cờ dirty đếm lệnh: undo về điểm save vẫn bẩn (sai ngữ nghĩa) |
| Toạ độ mutation | **Đúng hệ `hitTest` T-V7**: `{para, run, offset}` (offset run-relative, caret TRƯỚC ký tự offset) | Hệ toạ độ mới (char index toàn tài liệu): phải map 2 chiều, duplicate hitTest |
| Bảng (table cell) | **Refuse hẳn vòng 1**: `body[para]` là Table → lỗi rõ ràng; chỉ edit paragraph top-level. Layout `pi` của cell đụng độ pi block kế (ceiling T-V7 walk) → toạ độ mơ hồ | Edit trong cell: phải sửa cách đếm `pi` trong layout → đụng parity asset (cấm) |
| Caret vẽ | **Rect tĩnh (không blink) vòng 1**; forward-map `(para,run,offset) → (x, baseline)` = walk item của LineModel (`ri`/`charStart` công khai) + measurer — pure fn ở `editor.ts`, KHÔNG đụng layout.ts | Blink: nice-to-have, thêm timer + trạng thái — để slice sau nếu UX đòi; forward-map thêm vào layout.ts: module đang là asset đóng băng parity, thêm export vô ích rủi ro |
| Selection kéo | **Ceiling slice sau** (shift+click/mũi tên) — ghi §8 | — |
| Save | **`write_docx(&doc, Some(&media_src))` → tmp cạnh file gốc → rename** (pattern `save.rs`); đường save ENGINE giữ nguyên byte-identical | Qua Editor.bin/x2t: trái ý nghĩa rung (K01 §2: đường Nano Light là lúc thay) |
| Entry UI | **Window riêng own-UI `editor.html` (entry vite thứ 3, nằm ROOT — gotcha V01)** không shim/engine; mở qua nút **"Nano Light"** trên start page (command `nano_light_view`) HOẶC env `NANO_OFFICE_NANO_LIGHT` (gate 15, pattern V5 env-trước) | Nhét editor vào window viewer: lẫn hai sản phẩm, viewer là read-only về mặt contract gate |
| Gõ phím | **`keydown` trong webview** (printable / Backspace / Delete / Enter / Cmd-Z / Cmd-Shift-Z) → gọi command layer. Gate KHÔNG dùng keystroke thật (rule 6b cấm osascript) — probe env `NANO_OFFICE_NL_PROBE` chạy **cùng hàm edit** handler phím dùng | beforeinput/IME pipeline: vòng 1 không tiếng Việt composite (ghi §8), keydown đủ cho subset |

## 3. Kiến trúc

```
src-tauri/src/editor.rs               ← C3: command layer Nano Light (song song delivery)
  EditorSessions (map window-label → EditorSession)
  EditorSession { doc: Document (chưa-resolve), media_src: Vec<u8>,
                  undo/redo: Vec<(Document, Caret)>, saved: Option<Document>,
                  file_id: u32, path: PathBuf }
  commands: nano_light_view / nano_light_open / nano_light_edit / nano_light_save
editor.html (root) + src/editor.ts    ← webview riêng, không shim; consume
                                        src/viewer/layout.ts NGUYÊN VẸN
crates/nanooffice-ooxml (K01)         ← Document + resolve + write_docx (0 đổi)
```

- `nano_light_view(path)` (pattern `viewer_view` V5): validate → stash path →
  window `nanolight-<n>` focused(false) (rule 6b), không inject shim. Log
  `nano light view:`.
- `nano_light_open(window)`: parse file → **giữ `Document` + bytes gốc trong session**
  (thay cho DROP như `viewer_open` — review #4 C2) → register FileRegistry
  `owner=window-label` (media route) → trả `RenderDoc` + fonts + page + media_base
  (cùng shape payload `QuickView` — frontend viewer/editor dùng chung contract).
- `nano_light_edit(window, cmd)`: mutate `Document` tại toạ độ hitTest → resolve lại
  → trả `{doc, caret, dirty}` (undoDepth chỉ log Rust — không ai consume TS,
  cut ponytail). Log `nano light edit:`.
- `nano_light_save(window)`: `write_docx` + tmp + rename → `saved = clone`, dirty=false.
  Log `nano light saved:`.
- Cleanup: `WindowEvent::Destroyed` đã có sẵn ở lib.rs gọi `unregister_owner` (T-059);
  session map thêm hook cùng chỗ để drop session theo label.

### 3.1 Mutation semantics (toạ độ hitTest; mỗi lệnh trả caret mới)

| Lệnh | Hành vi | Caret trả |
|---|---|---|
| `insert {text, caret}` | chèn text vào run tại offset (paragraph 0 run → tạo run default); từ chối ký tự điều khiển `\t\n\u{c}\u{b}` | `(para, run, offset+len)` |
| `backspace {caret}` | offset>0: xoá ký tự trước caret (kể cả ký tự điều khiển trong run text) · offset=0, run>0: gộp text/images vào run trước, xoá run hiện tại · offset=0, run=0, para>0: gộp 2 paragraph — **pPr của paragraph CHỨA CARET thắng** (semantics Word: xoá dấu đoạn trước → nhận format đoạn sau) · đầu tài liệu / kề bảng: no-op | theo nhánh |
| `delete {caret}` | offset<len: xoá ký tự tại caret · cuối run: gộp run sau vào (text+images) · cuối paragraph: gộp paragraph sau vào · cuối tài liệu: no-op | giữ nguyên |
| `enter {caret}` | tách run tại offset (runs phải + props giữ cho 2 phía; **images theo run PHẢI** — parser flatten con trong run, ảnh luôn vẽ sau text, ceiling ghi §5), tách paragraph (pPr clone), block mới chèn sau | `(para+1, 0, 0)` |
| `undo` / `redo` | pop/push snapshot `(Document, caret)` | caret của snapshot |

Mọi nhánh mutate đều push snapshot vào undo TRƯỚC khi đổi model; no-op không push.

### 3.2 Caret forward-map (editor.ts, pure)

Từ `(para, run, offset)` → `(x, baseline)`: quét `model.lines` theo thứ tự, dòng có
`line.para == para`, item có `it.ri == run && it.charStart <= offset <
it.charStart + len(item)` → `x = it.x + measure(it.r, text.prefix)` (đúng语义 caret
TRƯỚC ký tự — mirror hitTest); không có item chứa strict (cuối run/paragraph) → item
CUỐI của run đó; paragraph rỗng → `line.x` (placeholder, như hitTest dòng rỗng).
Vẽ: `fillRect(x-0.75, baseline-lh+2, 1.5, lh-1)`.

## 4. Lộ trình slice

| Slice | Nội dung | XONG khi |
|---|---|---|
| **L0** (T-A27, phiên này) | vòng edit đầu chạy thật: DocSession + 4 command + caret + undo/redo + save thẳng + nút + env + **gate 15** | gate 15 positive exit 0 (edit thật → save → parse lại chứa text mới + media byte-exact → mở lại render đúng) + negative exit 1 ×2; verify FULL exit 0 |
| L1+ (task sau) | selection kéo (shift+click/arrows) · IME tiếng Việt · blink · pgMar/spacing vào model (hợp流 T-V6) · hyperlink/settings/theme vào model hoặc refuse-được | theo parity matrix §5.1 + criterion đóng rung A4 |

## 5. Tiêu chí đo

- **Round-trip vòng 1 (gate 15):** mở `.docx` trong Nano Light → synthetic edit
  (probe dùng CÙNG hàm edit của phím) → save → **parse lại bằng crate chứa text mới**
  → **mở lại (window 2) render đúng nội dung đã edit** + **media byte-exact**
  (sha256 từng part zip trước/sau) — toàn bộ KHÔNG qua Editor.bin/x2t.
- **Không thoái hóa nền:** parity-view 6/6 (chỉ bắt buộc nếu đụng `layout.ts`/viewer —
  vòng này KHÔNG đụng layout.ts; viewer.ts chỉ thêm gì nếu cần — ghi báo cáo);
  gates 9–14 nguyên vẹn (log contract đóng băng, log mới ADDITIVE `nano light *`).
- ms: first-draw Nano Light dự kiến cùng bậc viewer (18–89 ms) — gate 15 assert
  < 500 ms (cùng ngân sách rung).
- Kết quả slice ghi `§5.2` khi land (số đo thật).

### 5.1 Parity matrix — lỗ hổng NGOÀI `Document` (mở bằng Nano Light rồi save)

Nguồn xác định từng mục: đọc code crate (document.rs/write.rs — chính mình) +
parity-a2 đã surface. **KHÔNG nâng model mù** (discipline ceiling như V01 §10):
mỗi mục ghi giữ-được/mất + criterion nâng.

| Mục | Trong `Document`? | Qua Nano Light save | Criterion nâng |
|---|---|---|---|
| paragraph/text/pStyle/jc/numPr/ind, b/i/u/sz/rFonts/color/spacing (literal), rStyle 3-state | ✅ | **giữ** (round-trip bất biến, parity-a2 15/15) | — |
| bảng grid + rows + nested | ✅ (không tblPr/tcPr) | giữ cấu trúc; **mất tblPr/tcPr** (border/width cell) | khi edit bảng thật đòi (cùng lúc bỏ refuse §2) |
| list numbering resolve | ✅ | giữ (numPr raw + numbering part) | — |
| ảnh (part + extent + bytes) | ✅ (bytes qua `media_src`) | **giữ byte-exact** (T-A26) | — |
| headers/footers (blocks + rels) | ✅ | giữ | render header = slice sau |
| sectPr CUỐI tài liệu (pgSz) | ✅ (chỉ pgSz) | giữ pgSz; **mất pgMar + các con của sectPr khác** | T-V6 (pgMar) |
| **sectPr GIỮA tài liệu** | ❌ (bỏ khi parse) | **MẤT** (section break bị drop) | khi corpus có — nâng model + refuse tạm nếu user đụng |
| **hyperlink (`w:hyperlink`)** | ❌ (skip subtree khi parse — document.rs policy S1) | **MẤT CẢ TEXT bên trong** — data loss thật, phải biết | vòng gần nhất: tối thiểu surface text runs con; đúng chuẩn là vào model |
| settings.xml (defaultTabStop…) | ❌ | **mất** (tab stop về 720 quy ước) | khi tab edit đòi |
| theme fonts (`asciiTheme`…) | ❌ | **mất** (rFonts literal giữ nguyên) | khi corpus thật đòi |
| docProps (core/app) | ❌ | **mất** (metadata tạo lại tối giản) | A5+ nếu cần |
| tracked changes (`w:ins`/`w:del`), footnotes, fields | ❌ | **mất** (skip subtree) | refuse-warn khi phát hiện (xem §8 OQ-L01-2) |

Đây là danh sách THÀNH THẬN cho criterion đóng rung A4: từng lớp tính năng vào
matrix thì mới tính "thay được" — vòng 1 chỉ claim đúng dòng "giữ".

### 5.2 Kết quả slice L0 (03.10.2026 — T-A27)

Vòng edit đầu chạy thật trên `make_docx_layout.py` (6 block: 4 text + 2 ảnh),
debug build, boot nền env `NANO_OFFICE_NANO_LIGHT` + probe
`NANO_OFFICE_NL_PROBE` (chuỗi: hitTest đặt caret → insert marker → insert " XY"
→ backspace ×3 → enter → insert dòng 2 → undo → redo → save → reopen window 2):

| Đo | Số liệu | Đối chiếu |
|---|---|---|
| Parse + resolve (Rust) | **7.5–8.5 ms** | viewer cùng fixture 7.4 ms (gate 12 lần chạy cùng phiên) — cùng đường parse |
| invoke → first-draw | **27.0–39.0 ms** (×3 run) | viewer 27–89 ms; đích < 500 ms xa |
| Save (`write_docx` + tmp+rename) | **1.6–2.3 ms**, 2681 B | save engine qua x2t ~hàng trăm ms — đường Nano Light không có x2t |
| Media byte-exact | 2/2 part sha256 khớp snapshot trước save | T-A26 proven, gate 15 tái assert từng part |
| Reopen window 2 | evidence chứa marker trong text dòng | "mở lại render đúng nội dung đã edit" |

**Gate 15** `harness/scripts/smoke-nano-light.sh` (thường trụ verify.sh sau
gate 13): positive exit 0 ×3 (trước + sau ponytail cuts) + **negative ×2 exit
1**: `NANO_LIGHT_NEGATIVE=garbage` (docx rác → `nano light failed:` cắn nhanh
tại parse, pattern gate 13) · `NANO_LIGHT_NEGATIVE=doctor` (post-check parse
bản PRE-SAVE không có marker → assert parse-back cắn — chứng minh assert đọc
đúng file đã save, không chỉ vắng mặt). Verify FULL exit 0 **19 ▸ steps**
(gates 9a–d/10/11/12/13/14 nguyên vẹn + 15 mới); gate 12 layout số không đổi
(16 dòng · 601.7 · 600.5 · lhs 17.9) — **layout.ts/viewer.ts KHÔNG đụng**
(editor.ts là consumer mới; vite gom layout thành chunk dùng chung). cargo
test 33 shell (+5 test editor: verbs/carets, utf-16 surrogate, refuse
bảng+control chars, session undo/dirty, save round-trip qua crate thật) +
12 crate.

## 6. Rủi ro + mitigations

| Rủi ro | Mitigation |
|---|---|
| Edit làm hỏng model → file user hỏng | save = write_docx đã parity (bất biến round-trip T-A22) + tmp/rename atomic; mutation chỉ chạm `Document` qua các hàm nhỏ có test |
| Registry/session race (nhiều window Nano Light + quick-view + engine) | session keying theo window-label (pattern T-059); FileRegistry register(owner=label) — không còn `latest()` |
| JS đọc field undefined (bài học T-059) | mọi struct Rust serialize ra JS: `#[serde(rename_all = "camelCase")]` |
| Đụng layout.ts làm parity fork | KHÔNG sửa layout.ts (0 diff); editor.ts chỉ import |
| Tiếng Việt gõ composite (telex/marked) vòng 1 | keydown nhận ký tự đã compose của IME cơ bản; composite multi-keystroke = ceiling §8 |
| Log contract gates 9–14 | log mới chỉ ADDITIVE `nano light …`; hằng `SMOKE_RE_NL_*` vào khối đóng băng smoke-lib cùng commit evidence dùng |

## 7. Kế hoạch kiểm thử (thay TEST.md — component-level)

- **Rust unit (editor.rs, 1-3/lớp):** insert/backspace/enter trên model thuần
  (paragraph có/nhiều run, rỗng, đầu/cuối tài liệu, refuse bảng, control char);
  undo/redo往返 + dirty về sạch khi undo về điểm save; session map open/close theo
  window label.
- **Gate 15** `harness/scripts/smoke-nano-light.sh` (pattern smoke-lib, thường trụ
  verify.sh sau gate 14): boot nền env `NANO_OFFICE_NANO_LIGHT` + probe
  `NANO_OFFICE_NL_PROBE=<marker>` → chuỗi edit thật (insert marker + " XY" →
  backspace ×3 → enter → insert dòng 2 → undo ×1 → redo ×1) → save → assert:
  `nano light first-draw` < 500 ms · `nano light edit:` ×chuỗi · `nano light saved:`
  · **parse lại bằng crate CLI → JSON chứa marker** · media part sha256 == snapshot
  trước save · **window 2 (mở lại qua `nano_light_view`) render evidence chứa
  marker** · cấm `nano light failed:`.
  Negative ×2 exit 1: (a) doctor — post-check parse bản PRE-SAVE (marker không có
  → assert cắn, chứng minh assert đọc đúng file đã save); (b) .docx rác →
  `nano light failed:` cắn nhanh (pattern gate 13).
- **Không regression:** verify.sh FULL exit 0 (gates 9–14 nguyên vẹn).

## 8. Câu hỏi mở

- [ ] **OQ-L01-1:** default-switch mở docx = Nano Light (thay engine) — quyết
  A4 muộn/A5 (D1 không hard-cut); vòng 1 mặc định vẫn engine. Ghi khi có chỉ thị user.
- [ ] **OQ-L01-2:** khi mở file chứa construct MẤT trong matrix §5.1 (hyperlink/
  sectPr giữa/ins-del): Nano Light nên refuse-mở (an toàn dữ liệu) hay mở + cảnh báo
  mất-khi-save? Vòng 1: mở im lặng (matrix công khai); chạm user thật thì quyết.
- [ ] **OQ-L01-3:** IME composite (tiếng Việt telex kiểu marked-input qua bàn phím
  ảo) — keydown có đủ không cần beforeinput? Thử khi user gõ thật.

## 10. Ceilings vòng 1 (ponytail — ghi thẳng, không giả vờ)

- Selection kéo/mũi tên di chuyển caret: chưa có (caret chỉ qua click = hitTest).
- Ảnh: vẫn own-line (ceiling V3 kế thừa); enter/backspace quanh run chứa ảnh giữ
  ảnh theo run (kèm rules §3.1).
- Edit trong bảng: refuse (toạ độ `pi` cell mơ hồ — ceiling T-V7 walk).
- Canvas cao = toàn tài liệu, cap 8192 px logic (ceiling V5 kế thừa; pagination
  post-A3).
- Blink caret: không (rect tĩnh).
