---
type: design
---

# Detail Design (DD) — K02 · Keymap + Keytip reveal + Settings (Track A, Milestone A5)

> Trạng thái: **design chốt qua grill 17.10** (Round 1–4, user confirm "hiểu giống nhau").
> Mã K02 (tiền tố K theo cam kết grill; K01 là converter — nhóm khác). Folder `keymap-a5/`
> theo tiền lệ K01/V01/L01. Ngôn ngữ chính thức: `GLOSSARY.md` §Phím tắt (Keymap, Key hint,
> Cheat-sheet) + §Chrome (Chrome, Menu tabs, Tools box).

## 1. Bối cảnh & mục tiêu

User muốn productivity phím tắt **tùy chỉnh được** + gợi ý phím **không phải đoán** (kiểu
Excalidraw) cho người dùng văn phòng phổ thông. Grill chốt: giữ chrome MS-style (U01 slice 6),
keymap một nguồn chân lý UI, hint theo cơ chế **reveal** (giữ ⌘ → sáng, thả → mất).

## 2. Quyết định chốt (grill 17.10 — Round 1–4)

| # | Quyết định |
|---|---|
| Q1 | Chrome MS-style giữ nguyên (KHÔNG Google-style) |
| Q2 | Keymap phủ cả lệnh engine + hành động shell, 1 bảng; v1 ~13-20 hành động |
| Q3 | ~~Settings window riêng (own UI)~~ → **AMENDED 20.10 (T-A46, user chỉ thị "tham khảo ONLYOFFICE — đồng bộ UI system"): Settings = view trong start page** (`src/settings-view.ts`, theme Graphite; sidebar/⌘,/gear toggle; cửa sổ riêng + `settings.html`/`settings.ts` + command `open_settings` XOÁ). Cheat-sheet vẫn nằm trong view Phím tắt. 21.10: MCP tách sang **AI view** (kiến trúc 3 view home·settings·ai — X01/A01) |
| Q4 | Xung đột phím: hỏi xác nhận đè; gán mới thắng, gán cũ về mặc định |
| Q5 | v1 áp dụng cửa sổ engine trước; Nano Light/Startpage ăn theo slice sau |
| Q6 | Lệnh engine đi **đường native** (không intercept); shell đi keymap JSON main frame |
| Q7 | Key hint = **reveal**: giữ ⌘ → Keytip sáng trên Tools box + Chrome, thả/blur → mất |
| Q8 | Glyph nền tảng ⌘⇧⌥⌃; chord bắt đầu bằng phím đang giữ → badge chỉ hiện phần còn lại |
| Q9 | Phím reveal: ⌘ (macOS) / Alt (Windows khi port); mất focus → tự tắt reveal |
| Q10 | Settings: bánh răng cạnh tài khoản (Chrome) + Startpage footer + `⌘,` |
| Q11 | v1 = 13 hành động (bảng §5); Strikeout để trống làm demo gán mới |

Hệ quả thiết kế (không hỏi): engine chưa ready → lệnh engine không nhảy (đúng như Word);
reset từng hành động + reset tất cả; hint hiện cho MỌI lệnh engine (v2 chỉ mở rộng danh sách
chỉnh được trong Settings).

## 3. Bằng chứng upstream (đọc mã, không đoán)

| Sự kiện | Nguồn |
|---|---|
| API shortcut native của engine: `asc_applyAscShortcuts`, `asc_resetAllShortcutTypes`, `asc_resetShortcutType` | `upstream/sdkjs/common/apiBase.js:5001-5028` |
| Api object là window global trong app frame | `upstream/sdkjs/word/api.js:932-934` (`window.editor = this`) |
| 46 lệnh Word mở khóa (`c_oAscUnlockedShortcutActionTypes`); Cell 38; Slide 29 — gồm Bold/Italic/Underline/Save/Undo/Redo/SelectAll/Strikeout | `upstream/sdkjs/word/Editor/Shortcuts.js:39+` |
| web-apps persist theo format (`de-/sse-/pe-shortcuts`) + tự apply khi mở (`_applyShortcutsInSDK`) | `upstream/web-apps/apps/common/main/lib/controller/Shortcuts.js:50-69,485` |
| Tooltip nút tự sync theo keymap (`updateShortcutHints` → `btn.updateHint`) | `Shortcuts.js:334-367` |
| Không có UI chỉnh sẵn — controller headless cho integrator | không thấy view Shortcuts trong `main/app/view/` |
| Nút engine có id ổn định (`#id-toolbar-btn-print`, `#id-toolbar-btn-save`) | `upstream/web-apps/apps/documenteditor/main/app/view/Toolbar.js:196-213` — **id còn lại (bold/italic/underline/undo/redo/strikeout) là deliverable của S1 spike, không đoán** |
| Hàng tabs engine ẩn được bằng CSS shim, Backbone state vẫn chạy | DD U01 slice 6 (land 17.10) |

## 4. Kiến trúc (nhỏ nhất mà chạy)

```
keymap.json (app-data — 1 store cho cả engine + shell, fail-soft về default)
   │ load/save: 2 Rust command (storage helpers atomic có sẵn)
   ▼
Settings window (open_surface_window — pattern viewer/editor có sẵn)
   │  13 hàng: action · capture phím · reset · confirm-override · tab Cheat-sheet
   ▼
chia 2 đường ghi (user không thấy ranh giới)
   ├─ engine cmds → shim app frame poll `window.editor` ready →
   │    asc_applyAscShortcuts(...) (engine tự resolve phím từ đó)
   └─ shell actions → main frame keydown capture → action có sẵn
        (⌘O=openDialog, ⌘N=createDocument, ⌘,=Settings, ⌘/=Cheat-sheet, ⌘W=close)

Keytip reveal (shim app frame):
  keymap.json → stamp data-hint lên nút theo id (engine + chrome) khi apply
  keydown/keyup ⌘ (capture, không chặn) → toggle class `keyhint` trên body
  CSS: body.keyhint [data-hint]::after { content: attr(data-hint) } — 1 rule
  blur/visibilitychange → bỏ class (keyup thất lạc)
```

Lưu ý: KHÔNG đụng localStorage `de-shortcuts` của engine (store của mình là duy nhất;
engine controller không bao giờ có entry riêng → không xung đột launch-apply).

## 5. Phạm vi v1 — 13 hành động

| Nhóm | Action | Phím mặc định |
|---|---|---|
| engine | Bold / Italic / Underline | ⌘B / ⌘I / ⌘U |
| engine | Strikeout | *(trống — demo gán mới)* |
| engine | Save / Undo / Redo / Select All | ⌘S / ⌘Z / ⌘⇧Z / ⌘A |
| shell | Mở file / Tạo mới / Settings / Cheat-sheet / Đóng cửa sổ | ⌘O / ⌘N / ⌘, / ⌘/ / ⌘W |

**Ceilings ghi thẳng (v2+):** Headings/căn lề/indent/insert (engine vẫn hiện hint — chỉ Settings
chưa chỉnh); Alt-reveal Windows; Nano Light + Startpage keymap; chord nhiều nhánh kiểu KeyTips
sequence (Alt,H,B).

## 6. Slice plan

- **S0** — DD này + claim TODO (land trước code).
- **S1 — spike** (`WASEN_OFFICE_KM_PROBE=1`): shim app frame logs `km-probe: editor=<typeof
  window.editor> unlocked=<số action unlock được>` + tồn tại id nút engine v1 cần stamp
  (`#id-toolbar-btn-bold`…). ChỉProceed S2 khi probe POSITIVE (bài học T-A40: probe trước khi tin).
- **S2 — store + Settings**: `keymap.json` + 2 Rust command (load/save, validate fail-soft) +
  settings window (surface pattern) 13 hàng + capture + confirm-override + reset + tab Cheat-sheet.
- **S3 — apply chain**: shim poll → `asc_applyAscShortcuts` (engine) + main-frame keydown → shell.
- **S4 — Keytip reveal**: stamp data-hint + CSS 1 rule + listener ⌘ + blur-clear.
- **S5 — gate + verify**: smoke keymap (boot probe: keymap applied + keyhint class toggle) +
  verify FULL + build + install.

## 7. Kiểm định

1. **S1 spike — ĐÓNG 18.10 POSITIVE:** `km-probe: editor=object unlocked=46 btn-ids=7/7
   reveal=ok` (log 02:12). id 7 nút xác thực upstream trước khi probe:
   `id-toolbar-btn-{bold,italic,underline,strikeout,save,undo,redo}` (Toolbar.js view).
   `selectall` CỐ Ý không có nút → `missing=selectall` trong log là đúng thiết kế.
2. **S3/S4 e2e — ĐÓNG 18.10:** override strikeout ⌘⇧X (`WASEN_OFFICE_KEYMAP_FILE`) →
   `km-apply: engine=8 hinted=7 missing=selectall` — native apply qua
   `asc_applyAscShortcuts` (shape duck-typed: `{type, keyCode, ctrlKey, shiftKey, altKey,
   commandKey, isHidden}` — editorscommon.js:12327). Đổi chord → ẩn chord mặc định
   (isHidden:true). Relay: app frame poll editor → req UP (boot frame forward) → main
   frame trả entries DOWN; shim stamp `data-wasenoffice-hint` theo id + re-stamp 3s
   (late renderers); reveal ⌘ → `body.keyhint` (CSS 1 rule, self-test synthetic event).
3. **Gate 20** `smoke-keymap.sh` (thường trụ verify.sh sau gate 19): arm 1 positive
   engine=8 + reveal=ok; arm 2 fail-soft (store rác → defaults engine=7). PASSED ×2 arms.
4. Regression: verify FULL exit 0 (26 steps — 25 + gate 20); tsc sạch; clippy sạch;
   cargo test workspace green (untitled flake = env race đã ghi, pass standalone).
5. Fail-soft: keymap.json rác → defaults, không crash — unit test + gate 20 arm 2.

**Ceilings ghi thẳng (chưa làm, có seam):** live re-apply cho doc đang mở (hiện apply lúc
open — đổi phím ăn từ tài liệu mở SAU); shell keys chỉ nhận khi main frame focus (engine
frame sở hữu phím của nó); Settings VI-only; reveal chỉ tools box engine (chrome chỉ gear
tooltip tĩnh); chord mới dùng cùng key với HĐH (⌘W…) không có lối thoát riêng.

## 8. Changelog

| Ngày | Nội dung |
|---|---|
| 2026-10-17 | DD K02 khởi tạo từ grill Round 1–4 — design chốt trước code |
