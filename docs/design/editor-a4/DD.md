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
| Bảng (table cell) | **Refuse hẳn vòng 1** (số liệu pi mơ hồ) — **ĐÓNG ở slice A4 #3 (T-A30, §5.5)**: identity `block`+`cell`+`cellPara` giải tọa độ, single-cell edit mở; cross-cell/nested vẫn refuse | Edit trong cell mà không sửa cách đếm `pi` → trúng sai block (tái hiện bằng walk T-060) |
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
| L1+ (task sau) | selection kéo (shift+click/arrows) — **slice #1 = T-A28, §5.3** · IME tiếng Việt · blink · pgMar/spacing vào model (hợp流 T-V6) · hyperlink/settings/theme vào model hoặc refuse-được | theo parity matrix §5.1 + criterion đóng rung A4 |

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
| sectPr CUỐI tài liệu | ✅ (pgSz) | giữ pgSz + **pgMar** (T-A31); con khác của sectPr = **warn** | render pgMar = T-V6 |
| **sectPr GIỮA tài liệu** | ✅ (T-A31 — `Paragraph.sect` typed-minimal pgSz+pgMar) | **giữ** pgSz+pgMar; con khác = **warn** | gate 15 parse-back JSON + parity-a2 invariant |
| **hyperlink (`w:hyperlink`)** | ✅ (T-A31 — `Run.link` {rel_id, anchor, target, external}, runs flatten) | **giữ** nhóm link + runs (edit xuyên như text thường) | gate 15 parse-back rel+target+run text (T31LINK trong link run) + parity-a2 invariant |
| settings.xml | ✅ (T-A31 — passthrough byte-exact, KHÔNG vào model) | **giữ byte-exact** | parity-a2 check (e) + gate 15 sha256 |
| theme fonts | ✅ (T-A31 — passthrough byte-exact) | **giữ byte-exact** | parity-a2 check (e) + gate 15 sha256 |
| docProps (core/app) | ✅ (T-A31 — passthrough byte-exact) | **giữ byte-exact** | parity-a2 check (e) + gate 15 sha256 |
| tracked changes (`w:ins`/`w:del`), footnotes, fields | ❌ (skip subtree) | vẫn mất + **WARN khi mở** (`nano light warn:` + banner — T-A31, ĐÓNG OQ-L01-2) | detect unit tests + gate 15 `warndoor` |

Dòng MẤT còn lại (trung thực): tblPr/tcPr (styling bảng) · tracked changes/footnotes/
fields (nội dung — chỉ detect+warn) · con khác của sectPr (cols/docGrid/header-ref —
chỉ detect+warn). Mỗi dòng còn lại có warning hoặc criterion nâng riêng ở §10.

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

### 5.3 Slice A4 #1 — selection kéo + arrows (T-A28, 03.10.2026 — design-first)

> Phạm vi theo handoff: selection model + arrows + mouse + highlight overlay +
> mutation range-aware + C5-full phần đụng contract + gate 15 mở rộng.
> OUT (ceiling — ghi ở §10): IME composite (OQ-L01-3) · clipboard ·
> double/triple-click · caret blink · selection/chỉnh sửa trong bảng · C4/C5 phần còn lại.

**a) Model selection.** `SelRange { anchor, focus }` — 2 `Caret` (Rust
`editor.rs` + TS `editor.ts` interface, 1 nhà camelCase có comment cross-ref
C5). `anchor` = điểm bắt đầu gesture (đứng yên), `focus` = đầu di động;
`anchor == focus` ≡ caret thường (Rust-side `sel = None`). **Ordering tổng**:
`key(c) = (para, run, offset)` so từ điển — carets top-level (cell refuse)
nên đây là total order đúng thứ tự tài liệu; selection rỗng không phải trạng
thái riêng.

**Chuẩn hoá toạ độ (quyết chính của slice): `Caret.para` = BODY BLOCK index**
(= `Hit.block` của seam T-060), KHÔNG phải `pi` walk-counter. Lý do: `pi` đếm
table = 1 block nên với doc có bảng, `pi` của paragraph top-level SAU bảng ≠
body index — `paragraph_at` address qua `doc.body[para]` sẽ trúng sai block
(im lặng, đã tái hiện bằng walk `layout.ts`) hoặc trúng bảng; selection
cross-para mutate NHIỀU block nên bắt buộc địa chỉ đúng. Fix 3 điểm: editor.ts
consume `h.block` (2 chỗ đặt caret); `caretXY` match theo `line.block` thay
`line.para` (seam editor, KHÔNG phải đường parity draw — gate 14 sở hữu
semantics, fixtures table-free nên số không đổi, thêm assert block-match trên
fixture bảng); Rust không đổi (para đã là body index từ T-A27). Table-free:
`block == pi` — hành vi byte-identical.

**Đụng cell → REFUSE, không clamp**: endpoint có `cell ≠ null` → selection
không được lập (collapse về plain caret tại endpoint đó). Lý do: mutation đã
refuse trên cùng identity — lập highlight vào vùng không edit được tạo
"selection chết"; clamp buộc chọn phía khi cả hai đầu trong cell — thêm nhánh
không mua gì. Rust-side range op tự kiểm tra lại cả 2 endpoint (defense in
depth, cùng message refuse cũ).

**Undo snapshot mở rộng**: `(Document, Caret, Option<SelRange>)`. Caret/sel
trong snapshot = cái **cmd mang** (thật tại thời điểm mutate), không phải
`self.caret` — movement TS-side (arrows/click) giữa 2 lệnh không sync về
session (không round-trip cho pure movement — ceiling); undo/redo trả lại
cả caret lẫn sel của thời điểm đó, `EditOutcome` +`sel` để TS khôi phục
highlight sau undo.

**b) Bảng phím** (plain = di chuyển, shift+ = extend — anchor giữ, focus di;
mọi test qua synthetic KeyboardEvent hoặc gọi trực tiếp handler, rule 6b):

| Phím | Hành vi | Biên |
|---|---|---|
| Left/Right | ±1 ký tự (surrogate pair = 1 bước — khớp backspace) | qua biên run (run trước: offset = len(run trước)); qua biên paragraph (body index ±1, paragraph rỗng = (p,0,0)); chạm bảng/dưới document = no-op |
| Up/Down | line-to-line trên `model.lines` ± 1 giữ **goal-x** | goal-x = x caret lúc BẮT ĐẦU gesture (null → set), hitTest tự clamp vào biên dòng; reset bởi mọi hành động ngang/chuột/mutation; dòng in-cell bị SKIP; dòng ảnh (ở `media`, không ở `lines`) bị bỏ qua (ceiling) |
| Home/End | đầu/cuối **dòng layout** (không phải paragraph): `hitTest(line.x)` / `hitTest(line.x + line.total)` | hitTest clamp sẵn |
| Cmd+Left/Right | đầu/cuối **paragraph** qua `body[para].runs` | paragraph rỗng → (p,0,0) |
| Mouse | mousedown = anchor+focus (= hit); kéo: mousemove cập nhật focus; mouseup kết thúc; shift+click = extend | hit cell ≠ null → clamp về plain caret (mục a) |

Probe gọi thẳng `mouseDownAt/mouseMoveAt/mouseUp` với toạ độ layout thật
(WebKit không cho constructing `offsetX/Y` trên synthetic MouseEvent —
ceiling ghi); phím thì dispatch KeyboardEvent thật qua handler (wiring E2E).

**c) Mutation range-aware (Rust `EditorSession::apply`).** `EditCmd` 4 verb
mang thêm `sel: Option<SelRange>` (serde default). Non-empty →
`apply_delete_range(anchor, focus)` chạy TRƯỚC verb tại start caret:

| Verb | Hành vi với range | Caret trả |
|---|---|---|
| insert | xoá range → chèn text tại start | start + len(text) |
| backspace | xoá range (KHÔNG rơi vào xoá 1 ký tự) | start |
| delete | xoá range | start |
| enter | xoá range → tách paragraph tại start | (start.para+1, 0, 0) |

`apply_delete_range` semantics: ordering 2 đầu; **cùng para**: cùng run =
cut `[s..e)`; cross-run = head giữ prefix (images xoá — range phủ tới cuối
text head), middle runs drop, tail giữ suffix từ `end.offset` (images giữ nếu
cut trước cuối text). **Cross-para**: head giữ prefix + drop các run sau nó,
các block giữa drop (**có Table → refuse TOÀN BỘ lệnh** — không drop im lặng,
chống data loss), tail giữ suffix, tail runs ghép vào head para, **pPr của
paragraph ĐẦU thắng** (Word). Ảnh theo run-text-end (parser flatten, DD §3.1):
ảnh của 1 run bị xoá ⇔ range phủ đến CUỐI text run đó. Paragraph rỗng đầu/tail
(virtual run 0/0) hợp lệ. Empty runs GIỮ nguyên (không splice) — caret
`(start.para, start.run, start.offset)` luôn hợp lệ, thêm nhánh đổi caret run
index là chỗ sinh bug. Refuse: endpoint cell ≠ null / range chứa Table /
caret stale (run/offset OOB) → Err, model nguyên, không push snapshot.

**d) Vẽ highlight — overlay parameter của `drawModel` (render.ts), KHÔNG fork
layout.** Mỗi `TextLine`: span dòng = `[(block, first.ri, charStart) …
(block, last.ri, charEnd)]`; ngoài `[start, end]` ordered → skip; phủ trọn →
rect `(line.x, baseline−lh+2, total, lh−1)`; partial → x từ `caretXY(endpoint)`
(x2−x1 ≤ 0 skip — đúng các biên "kết thúc ĐÚNG tại mép dòng"); dòng rỗng nằm
trong range → sliver 2px; cell lines skip. Màu `rgba(0,120,255,0.28)`, caret
đen vẽ SAU (trên highlight). `caretXY` return +`line` (index dòng) — additive,
Up/Down/Home/End dùng.

**e) Evidence + gate plan (ADDITIVE — log contract 9–15 đóng băng).**
Log mới `nano light selection: anchor=p/r/o focus=p/r/o span=<n>` (span =
khoảng cách ký tự giữa 2 endpoint ordered, cùng para; cross-para = −1) — emit
từ keydown shift-extend + mouseup (KHÔNG emit trong mousemove — spam). Hằng
`SMOKE_RE_NL_SEL` vào khối đóng băng smoke-lib CÙNG commit emitting end
(editor.ts). Gate 15 mở rộng: probe dispatch shift+ArrowLeft ×4 (chọn " two"
của dòng probe) → evidence span ≥ 3 → insert " T28-SELECT-REPL" thay selection
→ undo → redo → save → parse-back chứa "paragraph T28-SELECT-REPL" (các assert
cũ nguyên). Negative mới `NANO_LIGHT_NEGATIVE=seldoctor`: python assert
span == 0 → cắn exit 1 (chứng minh gate ĐỌC số liệu selection, pattern doctor
T-A27). Rust unit tests: range same-para/cross-para (+pPr đầu thắng + image
rule + refuse) · verbs replace selection · undo snapshot kèm sel.

**f) C5-full phần đụng contract.** SelRange camelCase 1 nhà (mục a); unify
nốt `QuickView` snake→camel: `viewer.rs` `#[serde(rename_all = "camelCase")]`
+ `viewer.ts` fields `parse_ms→parseMs`, `media_base→mediaBase` (~15 dòng) —
chuỗi log + evidence JSON KHÔNG đổi, gates GREEN nguyên số. `RenderDoc` giữ
casing hiện có (riêng `rel_id`/`cx_emu` là contract của layout.ts parity asset
— KHÔNG đụng trong slice này).

**g) Ceilings mở/đóng** — ghi §10: ĐÓNG "selection kéo/mũi tên chưa có"; MỚI:
IME composite (OQ-L01-3) · clipboard copy/cut/paste · double/triple-click ·
select-all · caret blink (giữ) · selection/sửa trong bảng (refuse) · arrows
không nhảy QUA bảng (no-op tại biên) · Up/Down bỏ qua dòng ảnh · goal-x clamp
theo advance đo được (rule V01, không snap biên ký tự) · movement TS-side
không sync caret về session giữa 2 lệnh (snapshot dùng cmd's caret — chính xác
tại thời điểm mutate).

**h) Kết quả slice (land 03.10.2026 — dd1a908 design → 9a1f923 rust →
04a0d7b ts+gate15 → 29244c8 C5 → 9ed3351 ponytail cuts):**

| Bằng chứng | Kết quả | Ghi |
|---|---|---|
| Gate 15 positive | exit 0 — first-draw **36.0 ms** (parse 8.1 ms) · selection evidence span 1→4 (shift+Left ×4 qua handler thật) · type-replace → save **2682 B / 1.6 ms** · parse-back chứa `paragraphT28` · reopen window 2 first-draw 14.0 ms | đích < 500 ms xa |
| Gate 15 negative ×3 | garbage exit 1 (`nano light failed:` cắn) · doctor exit 1 (marker assert cắn trên pre-save) · **seldoctor exit 1** (`span 4 != 0` — gate ĐỌC số liệu selection, không chỉ presence) | pattern T-A27 |
| Gate 14 | 90 → **93 checks** (+3 block-match trên fixture bảng: `caretXY(block 1)` vào cell line, `caretXY(block 2)` = line SAU bảng — pi collision không leak vào caret work) | seam caretXY |
| Gate 12 (regression) | số KHÔNG đổi: 16 dòng · 601.7 · 600.5 · 750,-250,220 · lhs 17.9 · RSS tree 282 MB ≤ 300 | caretXY đổi không đụng đường draw |
| parity-view | **6/6 PASS exit 0** — maxdy 0.64/4.07/0.00/6.47/2.73/2.73, maxdx 47.00 đúng bảng §5.5 (layout.ts CÓ đụng — caretXY block-match + `line` index additive; draw path px-identical) | OQ-V01-1 |
| cargo test | 38 shell (+4: range same-para/cross-para+refuse, verbs-replace, undo-restore-sel) + 12 crate, tuần tự rc=0 | verify FULL 19 ▸ steps exit 0 ×2 |

### 5.4 Slice A4 #2 — IME composite tiếng Việt + clipboard plain-text + Cmd-A (T-A29, 03.10.2026 — design-first)

> Phạm vi theo handoff: composition pipeline (đóng OQ-L01-3) · clipboard
> plain-text Cmd-C/X/V · Cmd-A select-all · gate 15 mở rộng. Dựng trên seam
> T-A28 (SelRange + ranged mutations). **Rust: 0 đổi dự kiến** — paste = chuỗi
> `insert`/`enter` có sẵn, cut = ranged `backspace` (T-A28 §5.3c), copy =
> walk model TS-side. OUT (ceiling — §10): double/triple-click · caret blink ·
> selection/sửa trong bảng · arrows nhảy qua bảng · Up/Down bỏ qua dòng ảnh ·
> paste format RTF/HTML · IME ngôn ngữ khác · undo gộp theo từ · C4/C5 còn lại.

**a) Composition pipeline (đóng OQ-L01-3).** State TS-local trong `editor.ts`:
`composing: boolean` + `preedit: string`. Listener `compositionstart` /
`compositionupdate` / `compositionend` trên window (spec-ref: W3C UI Events —
`CompositionEvent.data`, `KeyboardEvent.isComposing`):

| Event | Hành vi |
|---|---|
| `compositionstart` | `composing = true`, `preedit = ''`, evidence, render |
| `compositionupdate` | `preedit = ev.data`, evidence, render |
| `compositionend` | `compositionEnd(ev.data)` — clear state, evidence, `data != '' → edit({kind:'insert', text: data, caret, sel})` |

`compositionEnd` là hàm core MÀ listener end lẫn probe cùng gọi — commit =
**1 command `insert` = 1 undo snapshot** (semantics `apply` Rust: 1 mutation
1 snapshot, T-A28 giữ nguyên — **0 đổi Rust**); data rỗng (IME cancel) → không
commit. **Keydown guard ĐẦU handler**: `composing || ev.isComposing` → return
KHÔNG preventDefault (IME sở hữu phím trong lúc compose — keydown printable
hiện hành chỉ chạy cho non-IME, đường cũ nguyên vẹn). Preedit vẽ tại caret
trong **overlay callback của `editor.ts`** (overlay param `drawModel` —
`render.ts` KHÔNG đụng): text tại `caretXY(caret)` + underline 1px tại
`baseline+2` (mirror underline draw), font = run tại caret (`PLAIN_RUN` fallback
para rỗng). **Không displacement inline** — glyph đứng dưới bị đè; ceiling
preedit re-layout post-A4. **Product vs probe**: product = event thật từ
webview IME (marked text cần editable sink — ceiling hidden-input §10, pipeline
sẵn sàng nhận event theo spec); probe gate = dispatch **CompositionEvent thật**
cho start/update (wiring E2E — precedent KeyboardEvent T-A28) + commit qua
`await compositionEnd(...)` (async commit phải await deterministic — listener
end wrap CÙNG fn; precedent offsetX: probe gọi handler trực tiếp khi synthetic
không đủ). Nếu run gate đầu thấy WKWebView constructing rớt `data` (evidence
cắn) → fallback probe gọi handler trực tiếp, cập nhật dòng này trong commit code.

**b) Bảng clipboard + Cmd-A.** Nguồn clipboard chốt: **webview async Clipboard
API (`navigator.clipboard`) + session stash fallback** — KHÔNG Tauri plugin
(dep mới + license gate không mua gì: stash đảm bảo round trong page; ghi chú
`main.ts` `frame.allow` chỉ áp iframe engine). Chỉ plain-text.

| Phím | Hành vi | Biên |
|---|---|---|
| Cmd-C | `selectionText()` (walk model TS, dưới) → `writeText` + `clipStash` | không selection → no-op, không evidence |
| Cmd-X | copy (op=`cut`) + `edit({kind:'backspace', caret, sel})` — ranged delete T-A28 | không selection → no-op |
| Cmd-V | `readText()` → lỗi/bị chặn → `clipStash` (evidence `src=system\|stash`) → normalize `\r\n?`→`\n` → split: phần đầu `insert` (sel ride — thay selection), mỗi `\n` kế = `enter` cmd (semantics §3.1), phần rỗng = para rỗng | N dòng = N undo units (ceiling gộp 1 unit); `\t` → Rust refuse control chars (loud, ceiling) |
| Cmd-A | anchor `(0,0,0)` + focus = cuối **paragraph block CUỐI** (walk ngược bỏ Table; runs rỗng → `(p,0,0)`); `caret = focus`, `render`, `emitSel` (span −1 cross-para) | không có paragraph nào → no-op; paste/delete sau Cmd-A span bảng → refuse TOÀN BỘ lệnh (T-A28, đúng ý) |

`selectionText()` (walk model, ordered `[s,e]` theo key `(para,run,offset)`):
same-para same-run = `slice(s.offset, e.offset)`; cross-run = head suffix +
middle full + tail prefix; cross-para join `'\n'`; **block Table contribute
rỗng** (plain-text). Paste-event listener KHÔNG làm — không editable sink để
event thật bắn vào; ceiling đi cùng hidden-input. Gate không probe cut riêng:
2 nửa (copy + ranged backspace) đã proven riêng.

**c) Evidence + gate plan (ADDITIVE — log contract 9–15 đóng băng).** Log mới:

- `nano light composition: state=<start|update|end> data=<text>`
- `nano light clipboard: op=<copy|paste|copy-all|cut> chars=<n> sys=<ok|fail|system|stash> text=<escaped>`
  — `text` escape `'\n'`→`'\\n'` (log 1 dòng); `chars` = độ dài TRƯỚC escape.

Hằng `SMOKE_RE_NL_COMPOSE` + `SMOKE_RE_NL_CLIP` vào khối đóng băng smoke-lib
CÙNG commit emitting end. Gate 15 mở rộng — chain ADDITIVE sau T28 redo,
TRƯỚC save (1 save phủ mọi parse-back assert): compose telex-style
`Xin chao` → `Xin chào` (2 bước preedit) → commit → `undo`/`redo` (1 unit
proof) → shift+Left ×8 chọn `Xin chào` → copy → ArrowRight ×4 (caret sau
shift-extend = **focus** 23, không phải anchor — ArrowRight đưa về 27, trước
`chào`) → paste (chèn giữa tail → `Xin Xin chàochào`) → Cmd-A → copy-all
(text chứa marker + `paragraphT28` + `Xin Xin chàochào`; `chars` == len của
text unescaped — assert tự-consistent) → ArrowRight (collapse) → save →
parse-back assert marker + `paragraphT28` + `Xin chào` + `Xin Xin chàochào`.
Selection asserts: giữ `max(span) ≥ min_span` + MỚI `min(span) == −1` (Cmd-A
cross-para evidence). Negative mới `NANO_LIGHT_NEGATIVE=clipdoctor`: clipboard
assert đòi `text == 'WRONG-CLIP'` + `chars == 999999` → cắn exit 1 (gate ĐỌC
số liệu clipboard, pattern seldoctor). Rust unit tests: 0 mới (không đổi
Rust); TS: 0 harness mới — gate 15 là tầng E2E duy nhất của slice này.

**d) Đóng OQ-L01-3** — pipeline composition per spec UI Events đã implement +
probe E2E gate 15; real-IME engagement (hidden editable sink) là ceiling §10
(phụ thuộc WebKit IME routing, không phải pipeline — thử khi user gõ thật,
không chặn slice này).

**e) Kết quả slice (land 03.10.2026 — 3ada9e8 design → 5a1a0e5 ts+gate15 →
9a2f24f ponytail cuts):**

| Bằng chứng | Kết quả | Ghi |
|---|---|---|
| Gate 15 positive | exit 0 — first-draw 91–110 ms (parse 3.7–9.0 ms) · composition evidence 4 dòng, end `Xin chào` · clipboard `op=copy chars=8 sys=fail` → `op=paste sys=stash` (stash fallback proven — WebKit chặn async clipboard không user activation) → `op=copy-all chars=1425` · selection span min −1 (Cmd-A) max 8 · save 2699 B / 2.0 ms · parse-back chứa marker + `paragraphT28` + `Xin chào` + `Xin Xin chàochào` · media byte-exact · reopen 15 ms | đích < 500 ms xa |
| Gate 15 negative ×4 | garbage exit 1 · doctor exit 1 (marker assert cắn trên pre-save) · seldoctor exit 1 (`span 8 != 0`) · **clipdoctor exit 1** (`'Xin chào'/8 != 'WRONG-CLIP'/999999` — gate ĐỌC số liệu clipboard) | pattern T-A27/T-A28 |
| Gate 14 (regression) | 93 checks — KHÔNG đổi | layout.ts 0 diff |
| Gate 12 (regression) | số KHÔNG đổi: 16 dòng · 601.7 · 600.5 · 750,-250,220 · lhs 17.9 · RSS 243–281 MB ≤ 300 | preedit vẽ ở overlay editor.ts |
| parity-view | **KHÔNG chạy — `layout.ts` KHÔNG đụng** (0 diff; render.ts/viewer.ts cũng 0) | điều kiện parity chỉ khi đụng asset |
| cargo test | 38 shell + 12 crate rc=0 tuần tự — KHÔNG đổi (Rust 0 đổi: paste = chuỗi insert/enter, cut = ranged backspace T-A28) | verify FULL exit 0 ×2, 19 ▸ steps |

### 5.5 Slice A4 #3 — double/triple-click + caret blink + MỞ edit trong bảng single-cell (T-A30, 03.10.2026 — design-first)

> Phạm vi theo handoff: địa chỉ caret trong cell (`cellPara` additive) · bỏ refuse
> edit-trong-cell (địa chỉ pi-mơ-hồ đã giải bằng `block`+`cell` T-A28/T-060 — refuse gốc
> §2 hết cơ sở) · selection same-cell · arrows trong cell · dblclick word + triple
> paragraph · blink · Cmd-A quyết · gate 15 mở rộng. Lý do refuse §2 ("toạ độ pi cell
> mơ hồ") không còn: `Caret.para` = body block index + `cell` identity (T-060 C3),
> resolution đi block=Table → cell → cellPara. Round-trip bảng qua `write_docx` proven
> T-A23 — slice chỉ đổi CONTENT trong cell, KHÔNG thêm tblPr/tcPr vào model.
> OUT (ceiling §5.5i/§10): Tab/Shift-Tab nav giữa cell · tạo/xoá bảng/hàng/cột ·
> merge/split cell · cross-cell selection · tblPr/tcPr styling · arrows nhảy qua bảng ·
> Up/Down bỏ qua dòng ảnh · hyperlink/sectPr (OQ-L01-2 = slice #4) · RTF/HTML paste ·
> preedit displacement · paste multi-line 1 undo · hidden editable sink.

**a) Địa chỉ cell — `cellPara` (additive 3 tầng).** Cell chứa NHIỀU block (CT_Tc);
`cell=[row,col]` chỉ định cell, thiếu trục paragraph-within-cell. Thêm
`cellPara: number | null`:

| Tầng | Thay đổi |
|---|---|
| `layout.ts` (parity asset — additive) | `TextLine` +`cellPara` (index block của paragraph trong cell.blocks; top-level + nested = null) · `Hit` +`cellPara` · `walk` +param `cellBlocks` (bs là blocks cell CẤP 1 → paragraph nhận index i; recurse vào cell bảng LỒNG truyền false → cellPara null) |
| Rust `Caret` | +`cell_para: Option<usize>` `#[serde(default)]` (payload cũ parse được — rule additive) |
| TS `Caret` | +`cellPara: number | null` |

`caretXY` refinement (additive, backward-compatible): match `line.block === caret.para`
như cũ; KHI caret mang `cellPara` (≠ null) THÊM điều kiện `line.cellPara === caret.cellPara`
+ `line.cell` deep-equal `caret.cell`; caretlegacy không mang field → hành vi block-only
nguyên vẹn (gate 14 T-060 checks không đụng). Bảng LỒNG: cellPara null → mọi verb refuse
loudly ("nested table content is not editable") — chặn cả nhánh silent-wrong-cell (cellPara
của cell trong lồng resolve nhầm sang paragraph cell ngoài).

**b) Resolution path Rust — verbs generic trên `Vec<Block>`.** Các free fn
`apply_insert/backspace/delete/enter/delete_range` đổi param `&mut Document` →
`&mut Vec<Block>` (thân không đổi — cùng ngữ nghĩa index, `paragraph_at(body, idx)`
chỉ còn arm top-level + refuse Table-stale). `apply` dispatch:

```
caret.cell = None            → verb(&mut doc.body, caret, sel)          (đường cũ)
caret.cell = Some([r,c])     → cell_blocks_mut(doc, caret)  // body[para]=Table → rows[r][c].blocks
                               caret' = { para: cellPara, ..caret }   (rebase)
                               sel endpoints rebase cùng cách (đã check same-cell)
                               verb(blocks, caret', sel') → caret'' → { para: table_block, ..caret'' }
```

Refuse rules (thay refuse cell≠null cũ): caret cell≠null mà `cell_para` None → refuse;
selection endpoint Cell↔top-level hoặc Cell↔Cell khác → refuse (message rõ); same-cell
(kể cả cross-cellPara) OK. Backspace đầu cell / delete cuối cell = no-op (không thoát
khỏi cell — cùng logic biên `para == 0` / block kế là Table vốn có của code generic);
enter trong cell = paragraph MỚI trong cell.blocks. Snapshot undo giữ caret đầy đủ
identity — undo/redo trả đúng chỗ trong cell.

**c) Selection same-cell.** TS `moveTo`: điều kiện lập selection = cả 2 đầu cùng ngữ
cảnh (`inSameCell`: cùng null, hoặc cùng `cell` deep-equal VÀ cả hai `cellPara ≠ null`);
không thoả → collapse về plain caret (rule T-A28 giữ nguyên hình thức). `cmpCaret` thêm
`cellPara ?? 0` vào key (trước `run`) — ordering cross-cellPara trong cell đúng; top-level
không đổi. `drawSelection`: match dòng theo ngữ cảnh selection (in-cell → chỉ dòng cùng
cell + cellPara ≠ null; top-level → chỉ dòng cell null); lineStart/lineEnd carets mang
đủ identity để `caretXY` trúng đúng dòng. `selectionText` (copy) walk theo container
(`containerOf`: cell → tc.blocks + cellPara, top-level → body + para) — copy/cut trong
cell hoạt động. Rust tự kiểm tra lại cả 2 endpoint (defense in depth — rule T-A28).

**d) Arrows trong cell + fix hitTest tie-break (bắt buộc cho feature).** `stepH`:
chạy trên container của caret — biên run/paragraph giữ nguyên logic; hết container =
dừng (top-level: chạm bảng/dưới doc; in-cell: KHÔNG thoát khỏi cell — Tab nav giữa cell
là ceiling). `moveV`: dòng kế phải cùng ngữ cảnh (in-cell → cùng `cell`, top-level →
`cell == null` — skip-cell-line cũ giữ nguyên); goal-x trong cell (hitTest tự clamp vào
span dòng). Home/End/Cmd+←/→ giữ identity qua spread (không rớt cell như trước).

**Fix hitTest (đổi hành vi có kiểm soát):** 2 cell CÙNG HÀNG có baseline TRÙNG NHAU
(layout side-by-side cùng yCur) — y-pick hiện tại (greedy so kề, tie giữ dòng TRƯỚC)
làm MỌI click vào cell phải resolve về dòng cell trái. Fix: chuyển so-kề sang
so-with-best + tie-break theo x-span (dòng chứa x thắng khi baseline bằng nhau).
Table-free byte-identical (không bao giờ có baseline trùng); draw path KHÔNG qua hitTest
→ parity-view không chịu ảnh hưởng; gate 14 +checks chứng minh (cặp collision → hitTest
trả dòng đúng cell theo x; 93 checks cũ nguyên). hitTest `Hit` +cellPara stamp từ line.

**e) Double-click word + triple-click paragraph.** WKWebView không có event triple;
`dblclick` event không cho count mở rộng → **đếm click thủ công**: mousedown cách
< 500 ms + < 4 px của click trước → count+1, else reset 1 (chuẩn chung các editor);
probe gọi `mouseDownAt(x, y, extend, clicks)` trực tiếp (tiền đề offsetX T-A28 —
deterministic, không phụ thuộc timing). Handler mousedown thật tính count rồi truyền
cùng. Semantics (từ layout tokens — words/spaces có sẵn, KHÔNG regex mới):

| Gesture | Hành động |
|---|---|
| click (1) | đặt caret như cũ |
| double (2) | chọn word: expand quanh offset trong run text qua biên non-space (`selectWord`); clicked-space → collapse; word KHÔNG vắt run (tokens không vắt run — ceiling) |
| triple (3) | chọn paragraph: anchor đầu (run 0, offset 0) → focus cuối (run cuối, offset len) của ĐÚNG paragraph chứa caret (cellPara trong cell) |

Cả hai hoạt động top-level + trong cell (selection same-cell rule mục c); `emitSel`
chạy sau mỗi gesture (evidence span chứng minh). Sau dblclick/triple, gõ = insert thay
selection (đường range T-A28 có sẵn). Ceiling: kéo-chữ-theo-word sau dblclick · word
vắt run.

**f) Caret blink.** `caretOn: boolean` + interval 500 ms toggle → **repaint từ model
cache** (drawModel lại, KHÔNG re-layout, KHÔNG evidence); composing → caretOn ép true
(caret đặc, IME đang sở hữu); mọi render()/moveTo gọi `wakeCaret()` (reset phase). Không
gate visual — constraint: redraw path dùng lại `render()`-họ (repaint là nửa draw của
render, cùng overlay callback).

**g) Cmd-A với bảng — GIỮ NGUYÊN (top-level only).** Lý do: mở rộng bao gồm cell
content tạo span Cell↔top-level mà rule same-cell (mục c) refuse từ đầu — Cmd-A rồi
edit sẽ luôn lỗi, UX giả; giữ span chỉ-paragraph → Cmd-A + edit refuse loud trên
"range spans a table" (rule T-A28 có sẵn, đúng ngữ nghĩa). Không dòng code nào đổi.

**h) Evidence + gate plan (ADDITIVE — log contract 9–15 đóng băng).** Log mới
`nano light cell: op=<kind> row=<r> col=<c> para=<cellPara>` emit từ `edit()` TS khi
cmd caret mang cell. Hằng `SMOKE_RE_NL_CELL` vào khối đóng băng smoke-lib CÙNG commit
emitting end. Fixture gate 15: `make_docx_layout.py` +flag `table` (appends 1×2
"Alpha cell"/"Beta cell" — **default KHÔNG đổi byte** vì gate 12 + parity-view dùng
chung generator). Probe chain thêm (trước save duy nhất): dblclick word "Beta" trong
cell B → gõ `T30B` (thay word) → triple-click paragraph MEDIUM (dòng text top-level
cuối) → gõ `T30P` (thay cả paragraph) → click cuối dòng cell A → gõ `T30C` → enter →
gõ `T30C2` → save → reopen. Parse-back (crate CLI) assert MỚI: `T30B`/`T30P`/`T30C`/
`T30C2` có · "Beta" + "bundled font metrics" MẤT (word/para bị thay thật) · "cell"
còn (word kế nguyên) · JSON structure: bảng 2 cell, cell A 2 paragraph (enter trong
cell), cell B text đổi ĐÚNG word. Selection evidence: span của dblclick = len("Beta")
= 4 — assert `4 in spans`. Negative mới `NANO_LIGHT_NEGATIVE=celldoctor`: assert cell A
1 paragraph (contradict thật tế 2) → cắn exit 1 — chứng minh gate ĐỌC cấu trúc cell
từ parse-back, pattern doctor T-A27..A29. Rust unit tests: cell verbs qua `apply`
(insert/enter/backspace + caret identity往返) · save round-trip qua crate thật (cell
đổi, cell khác + cấu trúc nguyên) · selection same-cell OK + mixed refuse · stale/
nested refuse. Gate 14 +checks: cellPara stamp, caretXY full-identity, tie-break
collision. Gate 12 số KHÔNG đổi; parity-view 6/6 ×2 BẮT BUỘC (layout.ts đụng).

**i) Ceilings mở/đóng** — ghi §10: ĐÓNG "blink", "selection/sửa trong bảng (refuse)",
"double/triple-click". MỚI: Tab/Shift-Tab nav giữa cell · tạo/xoá bảng/hàng/cột ·
merge/split cell · cross-cell selection · tblPr/tcPr styling · arrows nhảy QUA bảng
(vẫn no-op tại biên) · Up/Down bỏ qua dòng ảnh · nested-table editing (cellPara null)
· word vắt run / kéo-theo-word sau dblclick · Cmd-A bao gồm cell content · blink
visual gate.

### 5.6 Slice A4 #4 — save-fidelity: hyperlink + sectPr vào model, settings/theme/docProps passthrough, refuse-warn (T-A31, 04.10 — design-first)

> Phạm vi theo handoff: xóa lớp MẤT của matrix §5.1 + ĐÓNG OQ-L01-2 — slice tiềm năng
> đóng rung A4. OUT (ceiling §5.6h/§10): UI tạo/sửa/xoá hyperlink · link styling render ·
> Tab nav giữa cell · tạo/xoá bảng/hàng/cột · merge/split · cross-cell selection ·
> nested-table editing · fields/footnotes/tracked-changes VÀO model (chỉ detect + warn) ·
> preedit displacement · paste RTF/HTML · pgMar vào viewer layout (T-V6).

**a) Hyperlink → model.** `w:hyperlink` (§17.16.22, child của `w:p` §17.3.1.22, chứa run
content — `w:r` §17.3.2.25) là NHÓM runs mang một link property; parser FLATTEN nhóm (tiền
lệ flatten ảnh/break trong run — DD §3.1), KHÔNG thêm node trung gian:

| Tầng | Thay đổi |
|---|---|
| Model `Run` | +`link: Option<Hyperlink>` — `Hyperlink { rel_id: Option<String>, anchor: Option<String>, target: Option<String>, external: bool }` (`r:id` + target đã resolve qua part rels; `w:anchor` = link nội bộ, không relationship; `external` = TargetMode của rel nguồn) |
| Parser | `parse_paragraph`: Start `w:hyperlink` → đọc attrs, parse runs CON stamp `link`; r:id phải resolve được qua part rels với rel-type hyperlink (external = URI giữ nguyên, internal = `resolve_target`); thiếu CẢ `r:id` lẫn `w:anchor` → ParseError (refuse, tiền lệ numPr); hyperlink LỒNG = recurse với link của cấp trong (link trong cùng thắng) |
| Writer | `paragraph()` gộp CHẠY runs liền kề có `link` bằng nhau → 1 phần tử `<w:hyperlink r:id="…"( w:anchor="…")>`; rel re-emit với id CỦA MODEL (tiền lệ image — `r:id` phải re-resolve giống nhau); `TargetMode="External"` khi external (rels_xml +cờ external); root thêm `xmlns:r` khi có link (mở rộng `blocks_contain_image` → checks link cùng walk) |
| Editor | **0 dòng mutation**: link rides run — insert extends run (giữ link), enter clone run (link đi cả 2 nửa), backspace/delete/range gộp run KÉO theo link của run ĐẦU (chữ của run sau nhập vào nhóm link trước — semantics subset, tách/gộp nhóm link theo đúng Word = ceiling); KHÔNG refuse hyperlink ở verb nào |
| RenderDoc | **0 đổi** — runs trong link = runs thường (layout/draw không biết link; styling xanh/gạch = ceiling A5) |

Round-trip invariant giữ: 2 hyperlink kề nhau CÙNG target gộp thành 1 element khi ghi —
model GiỐNG NHAU (biên element không vào model). Adjacent khác link/không-link tách đúng.

**b) sectPr vào model — GO (ước lượng ~nửa ngày < ngưỡng ~1 ngày).** Mid-doc section =
`w:sectPr` trong `w:pPr` (parent pPr §17.3.1.26 — "mọi section trừ section CUỐI lưu sectPr
làm con của paragraph CUỐI section" — spec-ref mirror ISO/IEC 29500-1 1st ed, trang
SectionProperties). Typed-minimal, KHÔNG opaque-capture:

- `PageMar { top, right, bottom, left, header, footer, gutter: Option<i64> }` (`w:pgMar`
  §17.6.11, attrs verify qua trang PageMargin — twips, signed).
- `Section { page: Option<Page>, mar: Option<PageMar> }`; `Paragraph.sect: Option<Section>`
  (mid-doc); `Document.page_mar: Option<PageMar>` (CUỐI — giữ thêm pgMar cạnh pgSz có sẵn;
  matrix dòng "sectPr CUỐI mất pgMar" thu hẹp — viewer layout KHÔNG đụng, T-V6 vẫn là nhà
  render parity).
- Parser: `parse_ppr` Start `w:sectPr` → parse pgSz+pgMar vào `para.sect`; body-level đang
  có `parse_sectpr` mở rộng ăn thêm pgMar. Con khác của sectPr (cols/header-ref/docGrid…)
  skip — detect-warn mục (d) surfacing.
- Writer: pPr phát sectPr SAU jc (thứ tự CT_PPr: pStyle, numPr, ind, jc, sectPr); body-level
  sectPr thêm pgMar sau pgSz.
- **Mutation semantics:** `enter` tách paragraph — sect ở lại paragraph ĐẦU (head), paragraph
  mới clone pPr KHÔNG sect (1 section break không được nhân bản); gộp paragraph (backspace/
  delete/range) theo luật có sẵn "pPr ĐẦU thắng" → sect của head sống. Ghi vào comment + test.
- Viewer/editor layout KHÔNG đổi (vẫn pgSz cuối + margin cố định 1in — pgMar chỉ preserve
  trong model, không consume — OUT ceiling).

**c) settings.xml + theme + docProps → part passthrough byte-exact.** Model + JSON CLI
**bất biến** (passthrough KHÔNG vào `Document`): writer tự khám phá trong `media_src`
(package nguồn) khi write — mở rộng pattern media_src T-A26:

| Part | Khám phá | Rel trong output |
|---|---|---|
| `word/settings.xml` | doc-rels của main part nguồn, rel-type `…/relationships/settings` | doc-level, id fresh |
| `word/theme/theme1.xml` (đường tùy nguồn) | doc-rels nguồn, rel-type `…/relationships/theme` | doc-level, id fresh, target = `rel_target("word", part)` |
| `docProps/core.xml` | package-rels nguồn, rel-type `…/package/2006/relationships/metadata/core-properties` | package-level (`_rels/.rels`) |
| `docProps/app.xml` | package-rels nguồn, rel-type `…/extended-properties` | package-level |

Part thiếu/hỏng rel → skip im (thoái hóa về hành vi cũ); content-type copy VERBATIM từ
`[Content_Types].xml` nguồn (Override với đúng type nguồn — không hardcode bảng type);
part không được declare trong nguồn → skip (nguồn lỗi, không nhân bản lỗi). Đối xứng:
parser 0 đổi → JSON CLI byte-identical với file không mang các part này; crate round-trip
giữ chúng byte-exact; T-A24 canonical open-side hưởng theo (engine Editor.bin nhận đủ).

**d) Refuse-warn khi mở (ĐÓNG OQ-L01-2 = mở + cảnh báo, KHÔNG refuse — D1 engine vẫn là
chế độ nâng cao).** `detect_save_losses(pkg) -> Vec<String>` trong crate — scan thô phần
document.xml của main part (byte pattern, không đụng model):

| Construct | Pattern |
|---|---|
| tracked changes | `<w:ins `/`<w:ins>` · `<w:del `/`<w:del>` (biên từ chặn false-positive `<w:insideH`/`<w:delText` — sau "ins"/"del" phải là space hoặc `>`) |
| fields | `<w:fldChar` · `<w:instrText` |
| footnotes/endnotes | `<w:footnoteReference` · `<w:endnoteReference` |
| sectPr lạ | block `<w:sectPr…</w:sectPr>` chứa con `<w:…>` khác pgSz/pgMar (pgMar mới vào model — còn lại = mất khi save) |

UX shape: `nano_light_open` chạy detect trên bytes gốc → log ADDITIVE
`nano light warn: save will drop: …` (hằng `SMOKE_RE_NL_WARN` vào khối đóng băng
smoke-lib CÙNG commit emitting end) + payload `NanoLightDoc` +`warnings: string[]`
(camelCase) → banner tạo động trong editor window (div gắn vào body — editor.html KHÔNG
đụng, đang dirty rebrand của user) + không chặn open/save.

**e) Matrix §5.1 — cập nhật từng dòng (kết quả ghi lại sau khi land, cùng criterion):**

| Dòng §5.1 | Trước | Sau slice | Criterion kiểm chứng |
|---|---|---|---|
| hyperlink | ❌ MẤT CẢ TEXT | **GIỮ** — nhóm link {rel_id, anchor, target, external} + runs | gate 15 parse-back: rel + target + run text nguyên; parity-a2 invariant |
| sectPr GIỮA | ❌ drop | **GIỮ** pgSz+pgMar typed; con khác = warn | gate 15 parse-back JSON `sect` khớp; parity-a2 invariant |
| sectPr CUỐI | giữ pgSz, mất pgMar | giữ pgSz + **pgMar**; con khác = warn | parity-a2 invariant + fixture assert |
| settings.xml | ❌ mất | **GIỮ byte-exact** (passthrough) | gate 15 sha256 settings.xml saved == snapshot |
| theme fonts | ❌ mất | **GIỮ byte-exact** | gate 15 sha256 theme part |
| docProps | ❌ mất | **GIỮ byte-exact** | gate 15 sha256 core/app |
| tracked changes / footnotes / fields | ❌ mất im | vẫn mất (ngoài model) + **WARN khi mở** | detect unit tests + gate 15 `warndoor` |

**f) Evidence + gate plan (ADDITIVE — log contract 9–15 đóng băng).**

- Crate: fixture corpus mới `harness/fixtures/make_docx_fidelity.py` → `corpus-a2/fidelity-ta31.docx`
  (hyperlink external + internal anchor, mid sectPr pgSz+pgMar, settings/theme/docProps)
  — parity-a2 glob tự nhặt; **parity-a2.sh +check passthrough**: settings/core/app +
  `word/theme/*` có trong input → sha256 input == crate-rt output (mở rộng check (d)
  media); INVARIANT + constructs mới vào bảng DD K01 §5.1 khi land.
- Gate 15: fixture `make_docx_layout.py` +flag `fidelity` (default byte-identical —
  gate 12/parity dùng chung generator, proven diff như T-A30); gate chạy
  `make_docx_layout.py $FIXTURE table fidelity`. Probe (CÙNG edit() của phím): sau chain
  T-A30, hitTest vào dòng chứa link text → insert `T31LINK` XUYÊN qua link run → save 1
  lần → parse-back asserts: (1) run mang `link.target == https://…` vẫn tồn tại và chứa
  `T31LINK` (edit xuyên link, link sống), (2) internal anchor hyperlink còn, (3) paragraph
  mang `sect` với pgSz+pgMar khớp fixture, (4) `word/settings.xml` + theme + docProps
  sha256 == snapshot pre-save, (5) rels đã-save chứa hyperlink rel TargetMode External,
  (6) positive KHÔNG có dòng `nano light warn:` (không false-positive trên fixture sạch).
  Negative mới ×2: `NANO_LIGHT_NEGATIVE=linkdoctor` (assert target `WRONG-TARGET` → cắn
  exit 1 — gate ĐỌC link từ parse-back, pattern doctor) + `NANO_LIGHT_NEGATIVE=warndoor`
  (fixture tiêm `<w:ins>` quanh 1 run → positive chạy hết, assert đòi warnings chứa
  "hyperlinks" (sai thật tế) → cắn — gate ĐỌC nội dung warn + detect bắn đúng tracked
  changes qua đường open thật).
- Bonus engine-oracle: `smoke-render.sh <file-đã-save-từ-NL>` exit 0 — file ghi bằng writer
  mới mở render GREEN trong ENGINE (canonical open-side T-A24 hưởng writer mới — chứng
  phụ).
- Rust unit tests: hyperlink round-trip qua crate thật (external + anchor + rels emitted,
  TargetMode), sectPr mid+final round-trip, passthrough byte-exact fixture, detect units
  (ins/del/fldChar/footnote/sectPr-lạ + không false-positive trên fixture sạch), editor:
  link rides enter/undo + sect ở lại head khi enter.
- Số KHÔNG đổi: gate 12 (16 dòng · 601.7 · 600.5 · lhs 17.9); gate 14 chỉ được cộng
  (hiện 103); parity-view chỉ bắt buộc nếu đụng layout.ts — **KHÔNG đụng** (resolve/layout
  0 diff).

**g) Kết quả slice (land 04.10.2026 — 9823571 design → 24158d6 crate → 61c617f
editor+gate15 → b5ecd3c ponytail cuts):**

| Bằng chứng | Kết quả | Ghi |
|---|---|---|
| Gate 15 positive | exit 0 ×2 (trước + sau cuts) — first-draw 50.0/99.0 ms (parse 7.8/4.7 ms) · save **3.2 ms / 4503 B** · parse-back: link target `https://example.com/nanooffice-fidelity` còn với **T31LINK trong link run** + anchor nội bộ còn + mid sectPr pgSz 11906/pgMar 1134/720 còn · **settings/theme/docProps sha256 == snapshot** · rels đã-save mang `TargetMode="External"` · **positive KHÔNG có `nano light warn:`** | đích < 500 ms xa |
| Gate 15 negative | **7/7 exit 1**: garbage · doctor · seldoctor · clipdoctor · celldoctor (re-proven trên fixture fidelity) · **linkdoctor** (đòi `WRONG-TARGET` — cắn trên target thật) · **warndoor** (fixture tiêm `w:ins` → warn thật `'save will drop: tracked changes'`, assert đòi 'hyperlinks' → cắn — detect bắn đúng đường open E2E) | pattern doctor T-A27..A30 |
| parity-a2 | exit 0 **×2** (11 file, hard-fails 0) — corpus + `fidelity-ta31.docx`; INVARIANT 11/11; check (e) passthrough mới; diff fidelity 4 dòng = 2 lớp vô hại đã phân loại (hex lowercase + rel_id renumber — DD K01 §5.1) | x2t giữ nguyên link target qua oracle |
| Gate 12/14 | số KHÔNG đổi: 16 dòng · 601.7 · 600.5 · 750,-250,220 · lhs 17.9 · RSS 133 MB; gate 14 **103 checks** | layout.ts 0 diff — parity-view không chạy (điều kiện chỉ khi đụng) |
| Bonus engine-oracle | `smoke-render.sh <file-NL-đã-save>` exit 0 — file ghi bằng writer mới (hyperlink + 2 sectPr + 4 part passthrough) mở render GREEN trong ENGINE (canonical open-side T-A24 hưởng writer mới) | smoke log /tmp/nanooffice-smoke-ZoMhTL |
| cargo test | 41 shell lib (+1 link/sect ride edits/undo/round-trip) + **16 crate** (+4: fidelity parse, detect flags/spares, fidelity round-trip byte-exact, passthrough-silent) rc=0 tuần tự | verify FULL exit 0 **19 ▸ steps** ×2 |

Matrix §5.1: **6 dòng MẤT → GIỮ** (hyperlink, sectPr giữa, pgMar cuối, settings,
theme, docProps) + tracked changes/footnotes/fields lênwarn — criterion từng dòng
ghi sẵn trong bảng §5.1; **OQ-L01-2 ĐÓNG** (mở + cảnh báo, §8).

**h) Ceilings mở/đóng — ghi §10:** ĐÓNG "hyperlink/sectPr (OQ-L01-2 = slice #4)" của
§5.5(i). MỚI: UI tạo/sửa/xoá link · link styling render · link semantics đầy đủ Word
(tách/gộp nhóm link khi edit cắt qua biên — đang giữ link run đầu) · `w:history`/tooltip/
tgtFrame attrs · sectPr con khác (cols/docGrid/header-ref trong sectPr — đang warn) ·
fields/footnotes/tracked-changes vào model (đang detect+warn) · docGrid/lnNumType ·
settings.xml VÀO model (defaultTabStop consume — đang passthrough byte) · pgMar consume
vào viewer layout (T-V6).

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
- [x] **OQ-L01-2: CHỐT ở §5.6(d) (T-A31, 04.10) —** mở + cảnh báo mất-khi-save,
  KHÔNG refuse-mở: banner trong editor window (tạo động, editor.html không đụng) +
  log `nano light warn: save will drop: …` + payload `warnings`. Lý do: D1 — engine
  AGPL vẫn là chế độ nâng cao cho construct ngoài model; refuse-mở chỉ đổi lỗi thành
  block mà không cứu dữ liệu nào. Detect = `detect_save_losses` (byte scan) — unit
  tests + negative warndoor chứng minh cả detect lẫn UX.
- [x] **OQ-L01-3: CHỐT ở §5.4 (T-A29, 03.10) —** composition pipeline per spec
  UI Events (`compositionstart/update/end`, commit = 1 insert = 1 undo unit,
  preedit overlay, keydown tắt khi composing) + probe E2E gate 15; real-IME
  engagement (hidden editable sink) là ceiling §10 — pipeline không phụ thuộc nó.

## 10. Ceilings vòng 1 (ponytail — ghi thẳng, không giả vờ)

- Selection kéo/mũi tên di chuyển caret: **ĐÓNG ở slice A4 #1 (T-A28, §5.3)** —
  ceilings còn lại của selection ghi ở §5.3(g): double-click/blink/sửa trong
  bảng/arrows không nhảy qua bảng.
- IME composite + clipboard + select-all: **ĐÓNG ở slice A4 #2 (T-A29, §5.4)** —
  ceilings còn lại: **hidden editable sink** (real-IME engagement — WebKit cần
  editable để giao marked text; pipeline §5.4a sẵn sàng nhận event) · menu/
  context-menu paste event (cùng rợ hidden-input) · preedit displacement inline
  (đang đè glyph, không re-layout) · paste multi-line gộp 1 undo unit (đang N
  units cho N dòng) · paste `\t` refuse (Rust control chars) · paste format
  RTF/HTML · IME ngôn ngữ khác tiếng Việt · undo gộp theo từ · double/triple-
  click · caret blink · selection/sửa trong bảng · arrows nhảy qua bảng ·
  Up/Down bỏ qua dòng ảnh.
- Ảnh: vẫn own-line (ceiling V3 kế thừa); enter/backspace quanh run chứa ảnh giữ
  ảnh theo run (kèm rules §3.1).
- Edit trong bảng: **ĐÓNG ở slice A4 #3 (T-A30, §5.5)** — refuse §2 bỏ (identity
  block+cell+cellPara giải tọa độ); ceilings còn lại của bảng ghi ở §5.5(i).
- Save-fidelity: **ĐÓNG ở slice A4 #4 (T-A31, §5.6)** — hyperlink/sectPr vào model,
  settings/theme/docProps passthrough, refuse-warn (OQ-L01-2 đóng); ceilings còn lại
  ghi ở §5.6(h): UI tạo/sửa/xoá link · link styling render · link semantics Word đầy
  đủ (tách/gộp nhóm khi edit cắt biên — đang giữ link run đầu) · `w:history`/tooltip/
  tgtFrame · sectPr con khác (cols/docGrid/header-ref — đang warn) · fields/footnotes/
  tracked-changes VÀO model (đang detect+warn) · docGrid/lnNumType · settings.xml vào
  model (defaultTabStop consume — đang passthrough byte) · pgMar consume vào viewer
  layout (T-V6).
- Canvas cao = toàn tài liệu, cap 8192 px logic (ceiling V5 kế thừa; pagination
  post-A3).
- Blink caret: **ĐÓNG ở slice A4 #3 (T-A30, §5.5f)** — 500 ms toggle, repaint từ model
  cache, đặc khi composing; visual gate = ceiling §5.5(i).
