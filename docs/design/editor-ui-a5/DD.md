---
type: design
---

# Detail Design (DD) — U01 · UI editor riêng A5 (Track A, Milestone A5)

> Trạng thái: **draft** (mở màn Milestone A5 — T-A33, 11.10.2026). Tài liệu kiến trúc
> component-level (thay thế web-apps cho các surface của mình): TEST không tách file —
> kế hoạch kiểm thử ở §7, sống trong code + gate 15 + harness. Mã nhóm **U** cấp cho
> UI editor riêng (quy tắc `[Chữ][2 số]`; folder đặt `editor-ui-a5/` theo tiền lệ K01/V01/L01).
> Criterion đóng MILESTONE A5 = web-apps rút khỏi phân phối mặc định (vision.md QĐ9).

## 1. Bối cảnh & mục tiêu

Milestone A5 (QĐ9 `docs/vision.md`): **UI editor riêng** thay thế `web-apps` cho các
surface của WasenOffice (start page, quick view chrome, Nano Light chrome).
Engine ONLYOFFICE giữ vai trò "chế độ nâng cao" (D1 — không hard-cut tới A6);
khi mở chế độ nâng cao, web-apps vẫn được nạp.

Task T-A33 (Slice 0):
- Chốt quyết định sản phẩm về Framework (**RATIFY**).
- Chốt bảng giá trị màu sắc thực (**Graphite neutral-minimal**).
- Thiết kế kiến trúc chrome bọc quanh `DocumentSurface` (vừa tách ở T-064).
- Dựng và chạy thật **Slice 0** (Nano Light chrome: header + tên file + dirty dot + nút Save thật).

---

## 2. Quyết định kỹ thuật: Framework RATIFY (Design-it-twice)

| Tiêu chí | Phương án 1: Vanilla TS + Tailwind v4 (CHỌN cho Slice 0) | Phương án 2: Preact ~4.5KB (RATIFY cho Slice 1+) | Phương án 3: Svelte 5 (LOẠI) |
| :--- | :--- | :--- | :--- |
| **Runtime overhead** | **0 KB** (không thêm runtime dependency) | ~4.5 KB gzipped | ~10 KB compiler runtime |
| **Độ phức tạp Slice 0** | **Rất thấp** (~25 dòng DOM sync: `#doc-name`, `#dirty-dot`, `#save-btn`) | Cần config JSX factory, mount hook | Cần Svelte compiler plugin cho Vite |
| **Agent Fluency** | Rất cao (DOM API chuẩn) | **Cao nhất** (chuẩn JSX / React pattern) | Trung bình (cú pháp template riêng) |
| **License (MIT gate)** | Đạt (100% code nội bộ) | Đạt (MIT license, allowlist) | Đạt (MIT license) |
| **Khả năng mở rộng** | Nguy cơ phình code khi làm toolbar phức tạp | **Lý tưởng** cho component cây trạng thái sâu | Tốt cho reactivity |

### Quyết định sản phẩm:
1. **Slice 0 (Task T-A33):** Giữ **Vanilla TypeScript + Tailwind v4**. 
   *Lý do theo Ponytail ULTRA:* Header của Slice 0 chỉ có 3 element tương tác (`#doc-name`, `#dirty-dot`, `#save-btn`). Toàn bộ logic DOM sync chỉ tốn 25 dòng code. Thêm dependency framework ngay lúc này là over-engineering.
2. **Trigger chuyển đổi Preact:** Sẽ tích hợp Preact (~4.5KB) khi bắt đầu làm **Tab bar đa tài liệu (PH-2)** hoặc **Ribbon/Toolbar định dạng văn bản (Slice 2+ của A5)**, nơi số dòng manual DOM sync vượt ngưỡng 100 dòng.
3. **Loại Svelte 5:** Mặc dù cú pháp gọn, mức độ thông thạo (fluency) của các AI coding agent trên JSX/Preact cao hơn đáng kể so với Svelte runtime template.

---

## 3. Bảng Theme Tokens: Graphite Neutral-Minimal

Theo phân tích `docs/techstack-analysis.md`, hướng thiết kế chốt là **Graphite neutral-minimal** (tông màu zinc/slate trung tính, document-first, chrome mỏng chìm vào nền, tuyệt đối không sao chép ONLYOFFICE hay MS Office):

| Token Slot | Ý nghĩa | Giá trị Light (chuẩn) | Giá trị Dark (tự động) |
| :--- | :--- | :--- | :--- |
| `--color-bg` | Nền ứng dụng / khung ngoài | `#f4f4f5` (zinc-100) | `#18181b` (zinc-900) |
| `--color-fg` | Màu chữ mặc định | `#18181b` (zinc-900) | `#f4f4f5` (zinc-100) |
| `--color-accent` | Điểm nhấn tương tác / dirty dot | `#2563eb` (blue-600) | `#3b82f6` (blue-500) |
| `--color-panel` | Bề mặt điều khiển (button/input) | `#ffffff` | `#27272a` (zinc-800) |
| `--color-panel-active` | Nút khi nhấn / hover | `#e4e4e7` (zinc-200) | `#3f3f46` (zinc-700) |
| `--color-edge` | Đường viền ngăn cách nhẹ | `#d4d4d8` (zinc-300) | `#3f3f46` (zinc-700) |
| `--color-paper` | Màu nền trang tài liệu | `#ffffff` | `#ffffff` (trang giấy luôn trắng) |
| `--radius-panel` | Bo góc điều khiển | `6px` | `6px` |

---

## 4. Kiến trúc Chrome bọc Canvas

```
┌────────────────────────────────────────────────────────────────────────┐
│ Header Chrome: [Logo] [Tên tài liệu] [Dirty Dot]        [Nút Save]     │
├────────────────────────────────────────────────────────────────────────┤
│ Container: DocumentSurface Viewport (overflow: auto)                   │
│                                                                        │
│        ┌──────────────────────────────────────────────┐                │
│        │   Canvas 2D (#cv) — Document Rendering       │                │
│        │   (Inked text, carets, selections, media)    │                │
│        └──────────────────────────────────────────────┘                │
│                                                                        │
└────────────────────────────────────────────────────────────────────────┘
```

1. **Header DOM (`editor.html`):**
   - Nằm phía trên viewport canvas, không co giãn (`shrink-0`).
   - `#doc-name`: Hiển thị tên file từ payload `NanoLightDoc.name`.
   - `#dirty-dot`: Chấm tròn accent, hiển thị khi `dirty = true` (khi `doc != saved`).
   - `#save-btn`: Nút bấm lưu file gọi lệnh `wasen_light_save` (hoặc phím tắt `Cmd-S`).
2. **Floating Contextual Toolbar (Zen Minimalist — T-A36):**
   - Thanh công cụ nổi (`#format-toolbar`) chỉ xuất hiện khi có vùng chọn văn bản (`sel !== null` và `anchor != focus`).
   - Neo tọa độ tự động ngay phía trên selection focus (`caretXY`), tự ẩn khi selection sụp đổ hoặc khi tiếp tục gõ chữ.
   - Các nút định dạng: In đậm (**B**), In nghiêng (*I*), Gạch chân (<u>U</u>) kèm phím tắt `Cmd-B`, `Cmd-I`, `Cmd-U`.
   - Tương tác backend qua lệnh mutation `Format`, tự động tách run và cập nhật thuộc tính định dạng vào `wasenoffice_ooxml::Run` (bảo toàn undo/redo và save fidelity).
3. **Multi-document Tab Bar (T-A37):**
   - Thanh tab mỏng (`#tab-bar`) nằm ngay trên header hoặc tích hợp gọn gàng trong header.
   - Hiển thị danh sách các tài liệu đang mở: tab active mang viền `border-b-2 border-accent font-semibold`, tab inactive mang `hover:bg-panel-active text-fg/70`.
   - Mỗi tab hiển thị tên file, chấm dirty nhỏ khi có thay đổi chưa lưu, và nút đóng tab (`×`).
   - Nút `+` mở thêm tài liệu mới trong tab riêng mà không cần mở cửa sổ mới.
   - Backend hỗ trợ key session theo `window_label:tab_id` đảm bảo cô lập dữ liệu và undo/redo hoàn toàn độc lập giữa các tab.
4. **Tương tác với `DocumentSurface`:**
   - Viewport canvas nằm bên trong `<main id="canvas-box" class="flex-1 overflow-auto relative">`.
   - `DocumentSurface` (từ `src/viewer/surface.ts`) tự động đo đạc kích thước vùng hiển thị (`vw`/`vh`) theo container này.
   - Thao tác gõ chữ, điều hướng caret, selection tiếp tục diễn ra trên canvas mà không bị cản trở bởi header hay tab bar.

---

## 5. Lộ trình Slices (Milestone A5)

- **Slice 0 (T-A33 ✅):** Nano Light chrome header + theme Graphite + nút Save thật.
- **Slice 1 (T-A34 ✅):** Start page migration sang own-UI layout + theme Graphite neutral-minimal.
- **Slice 2 (T-A35 ✅):** Quick View chrome sang own-UI layout + theme Graphite.
- **Slice 3 (T-A36 ✅):** Floating Contextual Toolbar (Zen Minimalist / Zinc minimal) cho Nano Light: Bold/Italic/Underline, phím tắt Cmd-B/Cmd-I, neo vị trí theo caret selection, mutation verb Format trong Rust.
- **Slice 4 (T-A37 ✅):** Multi-document Tab Bar (Zinc minimal) cho Nano Light: thanh tab chuyển đổi tài liệu, mở/đóng tab, session keying theo tab.
- **Slice 5 (Seed):** Tab bar mở rộng đa cửa sổ & kéo thả tabs.
- **Slice 6 (T-A40 🔨 hiện tại):** Compact chrome **cửa sổ ENGINE** (chế độ nâng cao — D1).

### Slice 6 — compact chrome cửa sổ ENGINE (design 17.10, TRƯỚC code)

> User chỉ thị 17.10 (ảnh `test_haita.pdf`): chrome editor engine cồng kềnh — hàng 1 brand
> (đã 1 hàng mỏng theo fix), hàng 2 toolbar engine (save/print/undo + tên file + username),
> hàng 3 tabs engine (File/Home/Comment/Collaboration/View). Yêu cầu: **1 component header**
> gồm [menu trái · tên file giữa · tài khoản phải] + **component liền sau = tools box**
> gồm tool tương ứng menu đang chọn.

**Ranh giới upstream (QĐ9):** hàng tab + toolbar ngữ cảnh là DOM của web-apps trong iframe —
KHÔNG sửa upstream; mọi thao tác qua 2 seam được phép: (1) shim `desktopEditor.js`
monkey-patch `DocsAPI.DocEditor` (seam blessed theo AGENTS "shim, monkey-patch qua config
API") để truyền config customization; (2) cùng-origin giữa khung app khung iframe → driver
sự kiện qua NotificationCenter-trigger.

**Bằng chứng upstream (mã đọc qua, không đoán):**
| Chức năng | Chuỗi bạn dẫn | Nguồn |
|---|---|---|
| `toolbarNoTabs` = ĐỔI STYLE tabs ('line'), KHÔNG ẩn — deprecated (console.log Obsolete) | TabStyler.js đọc `toolbarNoTabs` → style 'line' | `upstream/web-apps/apps/common/main/lib/controller/TabStyler.js:63-75` |
| **CẤM `toolbarNoTabs` trong engine artifact của mình (root-cause 17.10):** api.js đọc `customization.toolbarNoTabs` → đổi iframe src sang **`index_loader.html`** (splash-loader variant) — artifact engines không có asset dev của variant này → boot CHẾT (3 GET rồi im lặng; counterfactual không có cờ → `index.html` đầy đủ, contentReady ✓) | api.js `getAppPath` chọn index theo cờ | `engines/editors/web-apps/apps/api/documents/api.js:1175-1178` (mirror upstream cùng dòng) |
| Placeholder load lúc `toolbarNoTabs` | Main.js kiểm tra config | `upstream/web-apps/apps/documenteditor/main/controller/Main.js:593` |
| Đổi tab ngữ cảnh (tool box đổi theo menu) | event `tab:set-active` → `toolbar.setTab(action)` + unfold | `upstream/web-apps/apps/documenteditor/main/app/controller/Toolbar.js:254-257` |
| Trigger mẫu `tab:set-active` | `trigger('tab:set-active', 'view', true)` | `upstream/web-apps/apps/documenteditor/main/app/controller/ViewTab.js:292` |
| FILE tab mở/đóng QUA NotificationCenter | `menu:show` → `onFileMenu('show')` → `setTab('file')` · `menu:hide` → `setTab()` | `upstream/web-apps/apps/documenteditor/main/app/controller/Toolbar.js:124-129,4191-4199` |
| FILE menu panel hiển thị cùng event | LeftMenu `menu:show` → `menuFilesShowHide('show')` | `upstream/web-apps/apps/documenteditor/main/app/controller/LeftMenu.js:93` |
| Kho tab engine (edit): file/home/ins/layout/links + addTab: review(Collaboration)/draw/headerfooter/charttab/forms/view | tabs view + addTab controller | `upstream/web-apps/apps/documenteditor/main/app/view/Toolbar.js:179-186` + `controller/Toolbar.js:3934-4075` |
| Boot config engine | boot page upstream desktop tạo `editorConfig` từ URL params — KHÔNG có passthrough customization | `upstream/web-apps/apps/api/documents/index.html.desktop:52-67,105-120` (_desktop` ≡ artifact engines — byte-identical đã diff) |

**Kênh driver chốt (100% NotificationCenter từ app frame — KHÔNG DOM-click):**
- Tab thường: `Common.NotificationCenter.trigger('tab:set-active', <action>)` với
  action ∈ `home · ins · layout · links · review (Collaboration) · view` (+ draw/forms/
  headerfooter/charttab khi engine bật).
- FILE: `trigger('menu:show')` mở (tab file + panel trái cùng lúc), `trigger('menu:hide')` đóng.
- Ẩn hàng tabs engine: **CSS trong shim app frame khi app ready**:
  `#toolbar .tabs { display: none !important; }` — strip element thuần nhìn, Backbone
  state vẫn chạy (setTab hoạt động bình thường, panels đổi theo). KHÔNG truyền
  `toolbarNoTabs` (root-cause boot-dead — bảng bằng chứng trên).
- **SPIKE KẾT QUẢ (17.10, criterion 1 ĐÓNG — POSITIVE):** probe 3 frame ×2 thời điểm —
  app frame (`documenteditor/main/index.html`) logs `common=object nc=object` ở cả t=2s
  lẫn t=6s; boot frame + main frame `undefined` (đúng thiết kế — chỉ app frame cần).
  Bằng chứng: `/tmp/ta40-spike2.log` (tab-probe t=2s/t=6s + asc_onDocumentContentReady).

**Cấu trúc DOM (khung app — `src/main.ts`):**
```
┌──────────────────────────────────────────────────────────────────┐
│ Header (1 hàng, ours): [File Home Comment Collab View]  tên-file   │
│   iCloud-like avatar account phải                                  │
├────────────────────────────────────────────────────────────────────┤
│ Tools box (engine giữ nguyên): toolbar ngữ cảnh engine, phản ứng    │
│ tab:set-active/menus engine (không thay đổi — không dán nhãn)       │
├────────────────────────────────────────────────────────────────────┤
│ Engine iframe (canvas tài liệu)                                     │
└────────────────────────────────────────────────────────────────────┘
```

**Kênh driver (shim ↔ app frame):**
- Shim ẩn strip `#toolbar .tabs` bằng CSS khi app frame ready (poll `#toolbar`, cap 12s).
- Menu tabs của mình (`src/main.ts`) click → postMessage `{type:'__wasenoffice_tab',
  action, state?}` xuống frame chain (shim mỗi frame relay xuống con) → frame có
  `Common.NotificationCenter` thực thi trigger (tab thường / `menu:show|hide` cho FILE).

**Phạm vi v1 (slice này):**
- Header: tabs Home/Comment/Collaboration/View + tên file + account; FILE tab mở menu
  qua `menu:show`.
- Engine: `toolbarNoTabs` (hàng tab engine ẩn).
- Nakhov: account = initials từ `username` param đã có (`'WasenOffice User'`).
- KHÔNG: dựng lại từng nút tools (toolbar engine giữ nguyên nội dung/nội dung nói được
  mọi tính năng nâng cao D1) — ribbon-parity là slice sau (T-A4-ribbon seed).

**KIỂM ĐỊNH (slice criterion):**
1. SPIKE rule trên: log probe chỉ ra `common=object nc=object` (hoặc fall về fallback
   thiết kế ceiling được ghi diagnostics).
2. Pixel: boot release → chụp screencapture — 2 hàng chrome (header + tools) thay cho
   3 hàng, không có hàng tabs engine.
3. Gate chính bằng pixel + `frontend_log`; verify FULL exit 0; tsc sạch.
4. Không đụng layout.ts / viewer.ts / editor (gio) — parity-view + gate 14/15 nguyên
   trạng (regression net).
---

## 6. Kế hoạch Kiểm thử & An toàn

1. **Gate 15 (Smoke Nano Light):**
   - Probe `WASEN_OFFICE_NL_PROBE` tự động gõ chữ $\to$ dirty dot hiện $\to$ bấm Save $\to$ disk thay đổi $\to$ reopen trong cửa sổ thứ hai.
   - Selector `#save-btn` và `#dirty-dot` phải được probe tìm thấy chính xác.
2. **Gate 14 (Layout core unit tests):**
   - Tuyệt đối không đụng vào `layout.ts` $\implies$ 114 checks giữ nguyên.
3. **Parity Render:**
   - Không đụng vào logic font/line height của `render.ts` $\implies$ giữ nguyên kết quả đo.

---

## 7. Changelog

## 8. PIVOT 18.10 — về native chrome + multitab (T-A43→T-A45), user chốt

**Bằng chứng kiến trúc ONLYOFFICE Desktop** (repo `DesktopEditors` — cấu trúc submodule,
github.com/ONLYOFFICE/DesktopEditors + `desktop-sdk`):
```
DesktopEditors = Qt shell (desktop-apps) + CEF wrapper (desktop-sdk) + engine submodules
shell KHÔNG vẽ chrome soạn thảo — chỉ: window · TAB BAR đa tài liệu · hội nhập OS
menu tabs/toolbar/panels = 100% engine (web-apps; desktop dùng web-apps-PRO → PDF có
Forms/Redact/Plugins/AI; artifact engines/ của mình lấy từ app cài = pro build)
```
**Quyết:** compact chrome (slice 6) **default OFF** — engine vẽ chrome native đúng doctype;
sauce A5 chuyển sang: T-A43 revert · T-A44 tab bar + iframe pool (brand tab = Startpage) ·
T-A45 per-tab fileId keying (T-059 rework). T-A40/T-A41 code giữ nguyên, ngủ sau flag
(`WASEN_OFFICE_COMPACT_CHROME=1` bật lại). Keytip reveal/keymap đi qua shim — không phụ
thuộc chrome, sống độc lập. PDF-dialog investigation (18.10): 5/5 flow synthetic sạch trên
`test_haita.pdf`, engine byte-identical ONLYOFFICE — treo chờ real-input test của user;
T-A43 loại thêm 1 biến số.

| Ngày | Nội dung | Commit |
| :--- | :--- | :--- |
| 2026-10-11 | Khởi tạo DD U01 mở màn Milestone A5 (T-A33) | *commit DD* |
| 2026-10-18 | §8 PIVOT: native chrome + multitab (T-A43→A45); compact chrome default OFF | *commit pivot* |
