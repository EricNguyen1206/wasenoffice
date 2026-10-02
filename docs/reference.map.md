---
type: evidence
---
# docs/reference.map.md

Bản đồ bằng chứng: upstream code ↔ nanooffice cần làm gì. Trạng thái: observed (có path) /
candidate (cần kiểm chứng). Cập nhật mỗi khi phát hiện mới.

## Nguồn upstream (chỉ đọc qua submodule hoặc onlyoffice-study)

| Thành phần | Vị trí | Ghi chú |
|---|---|---|
| sdkjs (engine editing) | `upstream/sdkjs` (submodule) | pin v9.4.0 (72b0421c0b) |
| web-apps (UI) | `upstream/web-apps` (submodule) | pin v9.4.0 (9c0ca538c3) |
| core (x2t C++) | chưa clone (x2t binary lấy từ ONLYOFFICE.app) | PH-1 sẽ cần source |
| desktop-apps (shell cũ) | `~/DATA/Backend/rust/onlyoffice-study/desktop-apps` (clone 22.09) | app layer mac KHÔNG có ở đây |
| desktop-sdk (CEF wrapper) | `~/DATA/Backend/rust/onlyoffice-study/desktop-sdk` (HEAD 2f16486, clone 22.09) | nguồn chân lý native surface |
| build_tools | **CHƯA CÓ** | cần khi tự build engine/x2t |

## Bridge JS→Native — trạng thái PH-0 (observed trong runtime NanoOffice 22.09)

Chuẩn method: tên giữ NGUYÊN VĂN upstream. Điểm into: `upstream/sdkjs/common/apiBase.js:266`.
"Sync-truth" = shim trả lời đồng bộ trong engine frame (CEF shell làm điều đó native-side;
async bridge không thay thế được — call-sites dùng trong `if` / lúc load script).

| Method | Bằng chứng call-site | Trạng thái NanoOffice |
|---|---|---|
| `CreateEditorApi` | apiBase.js:266 | **DONE** (ack; engine chỉ cần call không lỗi) |
| `IsLocalFile` | apiBase.js:1419, 2394, 3222, 5085 (sync boolean) | **DONE** (true — PH-0 local-only; sync-truth) |
| `getEngineVersion` | web-apps Desktop.js | DONE (pkg version) |
| `GetEncryptedHeader` | editorscommon.js:12380 (sync lúc load sdk-all) | **DONE** ("ENCRYPTED;" = fallback engine) |
| `CheckUserId` | sdk-all EndLoad path, gọi không điều kiện | **DONE** ("nanooffice-ph0") |
| `GetInstallPlugins` | apiBase_plugins.js:1691,1745 (`JSON.parse` → `[system,user]`, mỗi cái `pluginsData[]`) | **DONE** (2 entries `pluginsData:[]` — tránh crash `a[0]`) |
| `isSupportPlugins` / `isSupportMacroses` | apiBase_plugins.js | DONE (true) |
| `SetDocumentName` | apiBase.js:701 | DONE (ack + log) |
| `IsSignaturesSupport` | apiBase | DONE (false — signature là PH-3) |
| `execCommand` | desktopinit.js / Desktop.js: `webapps:entry`, `webapps:features`, `editor:onready`, `editor:config`, `title:button` | **DONE-PH0** (ack + log; chưa trả lời `editor:config` — xem điểm treo) |
| `LocalFileSetSaved` / `LocalFileRecoverFolder` / `LocalFileSetSourcePath` / `LocalFileGetSourcePath` / `LocalFileSetOpenChangesCount` | wrapper cpp `onlocaldocument_loadend` sequence (6396–6485) | **DONE-PH0** (ack; GetSourcePath trả path đã set). PH-1: `SetOpenChangesCount` ghi lại open-count cho phép tính truncate khi save (`bridge/save.rs`) |
| `LocalFileGetSaved` / `LocalFileSetSaved(v)` | wrapper cpp:2221–2233 (trả `m_bLocalIsSaved`; SetSaved chỉ nhận khi đang unsaved) | **DONE-PH0** (sync-truth bool, mặc định true = fresh open đã save). PH-1: SetSaved(true/false) từ save-end (`delivery.js` consumeSaveEnd) |
| `LocalFileGetOpenChangesCount` | web-apps DE app.js gọi sync sau open | **DONE-PH0** (sync-truth 0 — mở không changes) |
| `LocalFileSaveChanges(data, deleteIndex, count)` | `common/Local/common.js:418` — engine gửi deltas lúc save; cpp append `"<…>","` vào `<recover>/changes/changes0.json` (cpp:2119–2152; truncate cpp:5218) | **DONE-PH1** (`bridge/save.rs::append_changes` — byte-format shell, truncate theo quote (open+delete)×2; sync-truth `true`) |
| `LocalFileSave(param, pwd, docinfo, fileType, json, oldPwd)` | `word/Local/api.js:264` (+ `DesktopOfflineAppDocumentStartSave` :214); wrapper cpp:2174 → `cefview.cpp:3099` → x2t params XML | **DONE-PH1** (`bridge/save.rs::save_document` — x2t `Editor.bin`+changes → file gốc, atomic rename; lỗi → `EndSave(2)`; cần `<m_sFontDir>`+`<m_sAllFontsPath>` — không có thì x2t drop changes im lặng) |
| `OnSave` | word/api.js:2710 (`CoAuthoringApi.onUnSaveLock`); wrapper cpp:1757 | **DONE-PH1** (ack) |
| `CheckNeedWheel` | word/Drawing/HtmlPage.js:2415 (sync `false ===` so sánh); wrapper cpp:1879 trả true | **DONE-PH1** (sync-truth `true` — legacy compat) |
| `Property_Get/SetCryptoMode` (property `AscDesktopEditor.CryptoMode`) | wrapper cpp:2675 (defineProperty) + 4024/4035; mặc định 0 | **DONE-PH1** (getter 0 / setter ack — crypto PH-3 off) |
| `onDocumentModifiedChanged` | engine gọi ngay sau open (báo dirty state) | **DONE-PH0** (ack; shell thật track cho title/save — PH-1) |
| `LocalFileRecents` / `LocalFileRecovers` / `LocalFileOpenRecent` / `LocalFileOpenRecover` | web-apps documenteditor app.js (`i.LocalFileRecents()` sau `doc:onready`) + loginpage panels.js | **DONE-PH0** (ack null; push list qua `onupdaterecents` là PH-2) |
| `LocalStartOpen` | sdkjs Local/common.js:64, cefview.cpp:3013 | **DONE-PH0** (ack + delivery chain EndLoad chạy thật) |
| `LoadFontBase64` | wrapper cpp:1475 — sync set `window["<path>"]="<size>;<base64>"`; **build word 9.4 KHÔNG dùng** (font đi qua scheme `ascdesktop://fonts/`, xem § Font contract) | STUB (UNIMPL khi bị gọi — giữ quan sát) |
| Còn lại (~110 method) | bảng gốc bên dưới | STUB (UNIMPL logged khi bị gọi) |

## Desktop open contract (observed 22.09, từ desktop-sdk + runtime NanoOffice)

Chuỗi mở file local của shell thật — NanoOffice tái tạo trong shim + `get_document_for_delivery`:

1. Engine boot với document url `_offline_` → apiBase.js:682 `put_OfflineApp` → gọi `LocalStartOpen`
   (sdkjs Local/common.js:64 — chỉ sau khi modules load xong).
2. Shell convert file gốc bằng **x2t** → `Editor.bin` (magic `DOCY`) + `media/`
   (cefview.cpp:8338 "// start convert file"; đo PH-0: 28–116 ms cho fixture 1.9 KB).
3. Shell push vào editor frame (wrapper cpp:6396–6485, `onlocaldocument_loadend`):
   `LocalFileSetSaved` → `LocalFileRecoverFolder(folder)` → `LocalFileSetSourcePath(path)`
   → **`window.DesktopOfflineAppDocumentEndLoad(folder, base64, decodedLen)`** —
   `GetFileData2` = `NSCommon::GetFilePathBase64` (base64 + decoded length).
4. Engine: `$$ = zP.decode(b64, false, len)` → `g.tRb = f$c(data, "DOCY")` → `brb`
   (`h.prototype.brb`) → nhánh `Uwe` (tRb truthy) → `WKa.At(stream)` parse → fonts → render.

**Đã giải mã (27.09, TC-D01-06 GREEN):** điểm treo là shim thiếu chính lời gọi EndLoad
(`consumeDeliveredDocument` chỉ probe/hook — xem diagnostics 27.09). Chuẩn EndLoad:
`client_renderer_wrapper.cpp:6480` — `window.DesktopOfflineAppDocumentEndLoad("<folder>",
"<base64>", <decodedLen>)`, ngay sau đó cpp:6483 bắn `DesktopOfflineAppDocumentSignatures("")`
(empty string được engine xử lý đúng — nhánh `""!=a`).

## Desktop open contract — MULTI-FORMAT (mới 29.09, T-048)

Bản đồ format cho shell (bằng chứng từng dòng ở cột 3). Map ở code (T-052, formats
1 nhà): Rust `src-tauri/src/formats.rs` (bảng `FORMATS` + provenance từng hằng + workdir
layout `DocWorkdir`) ↔ `src/main.ts` `FILE_FORMATS` (nửa boot-page) — test Rust
`formats::tests::main_ts_file_formats_match_rust_table` đối chiếu 2 bảng (đếm entry +
filetype/doctype/isForm); delivery nhánh pdf: `src-tauri/src/lib.rs`
`get_document_for_delivery` (spawn qua seam `sidecar::convert`, T-A14).

| Định dạng | Boot params | Đường deliver | Bằng chứng |
|---|---|---|---|
| DOCX | `doctype=word&filetype=docx` | x2t → `Editor.bin` magic `DOCY;v10` | oracle x2t 29.09; contract gốc 22.09 |
| XLSX | `doctype=cell&filetype=xlsx` | x2t → `Editor.bin` magic `XLSY;v10` | oracle x2t 29.09 (`/tmp/t048-oracle`, exit 0); app chọn: api.js:1125 `cell→spreadsheeteditor` |
| PPTX | `doctype=slide&filetype=pptx` | x2t → `Editor.bin` magic `PPTY;v10` | oracle x2t 29.09; api.js:1126 `slide→presentationeditor` |
| PDF | `doctype=pdf&filetype=pdf&isForm=false` | **KHÔNG x2t** — copy file GỐC vào work dir, deliver bytes gốc (raw `%PDF`) qua cùng EndLoad | oracle CDP ONLYOFFICE.app 29.09: recovery folder chỉ chứa `f.pdf` (1003 B nguyên bản) + `changes/` + `media/`, KHÔNG Editor.bin; render GREEN 5 canvases 2772×1364; khớp fallback `folder+"/"+GetFileName(sFileSrc)` khi Editor.bin thiếu — `client_renderer_wrapper.cpp` "onlocaldocument_loadend" (~6410-6416) |

- **doctype values** (upstream api documents): `upstream/web-apps/apps/api/documents/api.js:1120-1127`
  appMap — `word→documenteditor`, `cell→spreadsheeteditor`, `slide→presentationeditor`,
  `pdf→pdfeditor` ( boot page `index.html.desktop:56` đọc `doctype`, mặc định `word`).
  Shell thật tính doctype theo nhóm format: `desktop-sdk applicationmanager_p.h:2888-2935`
  (`GetFileUrlParams`) — DOCUMENT→word (DOCXF/OFORM có đuôi riêng), SPREADSHEET→cell,
  PRESENTATION→slide, CROSSPLATFORM (pdf/djvu/xps)→pdf + `isForm=false` + filetype,
  DRAW→diagram+view.
- **Boot page dùng chung cho cả 4 app** — api page đọc query params và tự tạo
  `DocsAPI.DocEditor` theo `documentType`; không có boot page per-app.
- **Font loader/glyph manager theo build** (observed trong engines artifacts 29.09 —
  hành vi giống hệt nhau, chỉ khác tên minified; shim generalise qua `detectFontVariant`
  trong `src/shim/desktopEditor.js` + readiness union trong `src/shim/delivery.js`):

| Build (app) | loader proto | status/path/cb/idx | stream class | registry | glyph mgr.wasm |
|---|---|---|---|---|---|
| word (DOCX + PDF) | `AscFonts.sje.prototype.BSa` | UB/Va/sV/wOb | `G2b` | `Hcc` | `Wg.vue` |
| cell (XLSX) | `AscFonts.mye.prototype.cZa` | GE/Wa/TR/iXb | `uac` | `Nmc` | `hj.RIe` |
| slide (PPTX) | `AscFonts.K8d.prototype.VMa` | Pz/Na/gY/wFb | `FTb` | `D3b` | `mi.Zie` |

- **Format code x2t cho save multi-format (quan sát 29.09 cho T-049 — chưa implement):**
  oracle params XML `<m_nFormatTo>`: **257 = XLSX** (exit 0, `file` = "Microsoft Excel
  2007+"); **513 = EXPORT PDF** từ editor binary bất kỳ (PPTY/XLSY → %PDF 1.7 — KHÔNG
  phải PPTX); PPTX code chưa định danh (514/515/516/512/1025/8193/4097/258 đều fail) —
  fallback không truyền `m_nFormatTo` mà dựa extension output hoạt động (PK hợp lệ);
  cần spec-ref `AVS_OFFICESTUDIO_FILE_*` (core chưa clone) hoặc oracle tiếp. PPTY→PPTX
  cần `<m_sAllFontsPath>`/`<m_sFontDir>` như DOCX (thiếu → `DoctRenderer error code=open`,
  exit 80 — cùng signature bài học T-010).

## Converter bundle (x2t) — layout + runtime deps (mới 30.09, T-A10, rung A1)

Mục tiêu: rút tham chiếu runtime ĐẦU TIÊN khỏi ONLYOFFICE.app cài sẵn — bundle converter
vào `engines/converter/<target>/` (script: `harness/scripts/bundle-x2t.sh`, committed;
nội dung engines/ KHÔNG commit — mit-purity điều 4). Mọi nhận định dưới đây là quan sát
black-box trên binary đã build (`otool -L`, chạy thật) — không đọc source core.

- **x2t strong-link cả 17 framework** (otool -L `…/converter/x2t` 30.09): mọi
  `@rpath/<fw>.framework` là LC_LOAD_DYLIB thường, **0 LC_LOAD_WEAK_DYLIB** → dyld cần đủ
  17 lúc load; graph deps khép kín trong 17 (không framework nào trỏ ra ngoài hệ) →
  hypothesis "framework on-demand qua dlopen" bị PHỦ ĐỊNH ở mức dylib. Danh sách pin
  trong bundle-x2t.sh (thiếu 1 → script thoát lỗi, không bundle nửa vời).
- **Subset đã bỏ được (chạy thật 30.09, exit 0):** `templates/` (207 MB — templates
  tạo-file-mới per-locale, không cần cho open→edit→save file có sẵn) + `empty/` (2.9 MB)
  + `dictionaries/` (spell only). Không có cả 3 trong app cài sẵn thì không so sánh được —
  bằng chứng là bundle thiếu cả 3 vẫn convert + save from-changes GREEN.
- **DoctRenderer layout contract (root cause fix):** save from-changes chạy DoctRenderer
  trong x2t, load script theo `DoctRenderer.config` cạnh binary: `../editors/sdkjs/common/
  Native/native.js` (định nghĩa `window.InitNativeEditors` — thiếu → exit 86 `TypeError:
  window.InitNativeEditors is not a function`), `jquery_native.js`, xregexp. Trong app
  cài sẵn `../editors` = `Resources/editors`; trong engines/ thì editors build nằm ở
  `engines/editors` (sibling của converter/, không phải của `<target>/`) → bundle script
  tạo symlink `engines/converter/editors → ../editors` để giữ nguyên relative layout.
  `<allfonts>` trong config trỏ `../../editors/sdkjs/common/AllFonts.js` — file chưa
  tồn tại ở engines (T-A11 tự sinh); save params XML truyền `<m_sAllFontsPath>`/
  `<m_sFontDir>` từ generator (T-A11, § AllFonts generator — `all_fonts_source` cũ đã
  xoá) nên save GREEN.
- **Đo size (oracle-obs `du`, 30.09):** converter dir app = 323 MB (templates 207 ·
  x2t 42 · UnicodeConverter 31 · graphics 14 · PdfFile 7.2 · IWorkFile 5.3 · empty 2.9 ·
  HWPFile 2.7 · HtmlFile2 2.2 · ooxmlsignature 2.1 · kernel 1.4 · DjVuFile 1.2 ·
  doctrenderer 1.0 · Fb2File 1.0 · DocxRenderer 0.66 · StarMathConverter 0.42 ·
  XpsFile 0.26 · OFDFile 0.23 · kernel_network 0.22 · EpubFile 0.17). Bundle subset
  (x2t + config + 17 fw) = **113 MB** — VƯỢT budget ≤100 MB của rung A1 (QĐ9/vision §2);
  trim còn lại thuộc T-A12 (UnicodeConverter 31 MB + x2t 42 MB là 2 mục tiêu lớn nhất;
  đề xuất re-baseline budget ở diagnostics — không tự sửa vision).
- **Resolution x2t path (1 nhà: `save.rs::x2t_path()`, cả open-delivery lẫn save đều
  gọi):** env `NANO_OFFICE_X2T` (chỉ nhận khi file tồn tại — env sai → WARN + rơi xuống
  nhánh sau) → bundled `engines/converter/<target>/x2t` (walk ancestors từ exe dir + cwd)
  → app cài sẵn (log WARN — dependency đang rút). `<target>` = triple rustc host
  (`aarch64-apple-darwin`), bảng compose từ `std::env::consts::ARCH/OS` ở
  `save.rs::target_dir_name()` — đồng bộ bảng với bundle script (rustc -vV host).

## AllFonts generator — Rust thay việc app sinh (mới 30.09, T-A11, rung A1 — clean-room ĐẦU TIÊN)

- **Nhà duy nhất:** `src-tauri/src/protocol/allfonts.rs` — enumerate FONT_URL_ROOTS qua
  `font_root_disk()` (2 root như app; từ T-A12 root `app` = `engines/fonts/` bundle —
  § Font bundle — hết đọc app cài sẵn)
  + parse sfnt std-thuần (seek-read từng table, không đọc nguyên file — TTC lớn) → 1 chuỗi
  AllFonts.js (`js_raw`). 2 consumer: filesrv serve `rewrite_font_paths(js_raw)` (URL loopback)
  + `bridge/save.rs` ghi `js_raw` (disk path) ra `$TMPDIR/nanooffice-allfonts/AllFonts.js` cho
  `<m_sAllFontsPath>`/`<m_sFontDir>`. Cùng 1 chuỗi nguồn → index filesrv và x2t khớp nhau
  **by construction** (không cần khớp thứ tự của app). `all_fonts_source()` (đọc data-dir app) ĐÃ XÓA.
- **Format file (oracle-obs — decode byte-exact artifact của app: 557 record sạch 91283/91283
  byte):** 5 global — `__all_fonts_js_version__=2`; `__fonts_files` (path đĩa, BOM ở đầu file);
  `__fonts_infos` `[family, rFile, rFace, iFile, iFace, bFile, bFace, biFile, biFace]` (cặp
  số = FILE index + FACE index trong TTC — không phải style code); `__fonts_ranges` (triplet
  `start,end,infoIdx` vào RAW infos — optional: consumer `common/libfont/character.js:74`
  early-return khi thiếu; phục vụ FontPickerByCharacter fallback glyph thiếu lúc render);
  `g_fonts_selection_bin` (base64: `count` + record `[recordLen(tính cả 4 byte của chính nó),
  nameLen+name, altCount+alts, pathLen+path, faceIdx, it, bo, fixed, panoseLen(10)+panose,
  ur1..4, cp1..2, weight, width, famClass, fmt(1=glyf,2=CFF), avg, asc, desc, gap, xh, cap,
  fsType]` — LE; consumer `common/libfont/map.js:722 CFontSelect::fromStream` v2 + `:1434
  CFontsSelection.Init`). File:line sdkjs ở đây HỢP LỆ — sdkjs là consumer, không phải module
  đang thay (rule clean-room § ngoại lệ).
- **Đặc điểm consumer (Externals.js:636-694 `checkAllFonts`):** files → `CFontFileLoader`;
  infos → `CFontInfo(name, infoIdx, …)`; entry `"ASCW3"` bị consumer tự skip + thay bằng font
  symbol inline base64 (generator KHÔNG cần special-case — ASC.ttf có nameID1 = "ASCW3" như
  font thường); style slot trỏ face index thật trong file.
- **Rules sinh (xác nhận từ artifact, 557/557):** primary family = nameID1 pid3-e1-l0x409 →
  pid1-e0 → pid3-e1 → first; skip primary `.`-prefixed (hidden); alt = nameID1 pid3-e1 đầu
  tiên khác primary; slot = OS/2 fsSelection 0x20 (bold) / 0x01 (italic), thiếu OS/2 →
  head.macStyle 0x01/0x02 (Courier.ttc: metrics = 0, weight = 0); winner (family, slot) =
  |weight−400| min, tie → face sau. Files list chỉ chứa file có ≥1 face thắng slot.
- **spec-ref:** OpenType spec — `name` (NameRecord: platformID, encodingID, languageID, nameID,
  length, offset; UTF-16BE cho pid 0/3), `OS/2` (xAvgCharWidth@2, usWeightClass@4, usWidthClass@6,
  fsType@8, sFamilyClass@30, panose@32, ulUnicodeRange@42-57, sTypoAsc/Desc/Gap@68-73,
  ulCodePageRange@78/82, sxHeight@86, sCapHeight@88 — v1 = 86B, v2+ = 96B), `head`
  (unitsPerEm@18, macStyle@44), `post` (isFixedPitch@12), `cmap` (format 4 segments / 12 groups).
- **Deviations so với file app (behavior-tolerant, chi tiết diagnostics 30.09):** tie-break
  winner lệch 27/387 family (app không chọn theo weight — nghi CoreText trait float; VD
  `Sukhumvit Set` R = 'Semi Bold' w600 thay vì 'Text' w400); alt-name 2 face `.SF NS`;
  superset +1 family Apple Color Emoji (bitmap-only, app skip); `__fonts_ranges` tự sinh từ
  cmap coverage (widest-span-wins; 4370 vs 4686 triplet); Mac-Roman high-byte decode Latin-1;
  thứ tự file walk không replica app (index self-consistent là đủ).

## Font bundle — engines/fonts (mới 02.10, T-A12, rung A1)

- **Nhà:** `harness/scripts/bundle-fonts.sh` (committed; nội dung `engines/fonts/` không
  commit — `.gitignore engines/*`). Copy layout nguyên vẹn từ root font app cài sẵn
  (`/Applications/ONLYOFFICE.app/Contents/Resources/login/fonts/`): OpenSans ×5 + root
  LICENSE.txt; `crosextra/` Carlito ×4 + LICENSE; `caladea/` ×4 + LICENSE;
  `asana/` ASANA.ttc + LICENSE; `openoffice/` opens___.ttf + LICENSE. Manifest pinned
  20 file — lệch → script thoát lỗi (không bundle im lặng bộ khác). **KHÔNG bundle
  ASC.ttf** (license không rõ: name table chỉ có family "ASCW3" + version, không có
  manufacturer/license fields, app không kèm LICENSE riêng; engine 0 fetch — consumer
  thay bằng symbol font inline base64).
- **URL namespace giữ nguyên `/fonts/app/…`** (`protocol/mod.rs` `FONT_URL_ROOTS` +
  `font_root_disk()`: app = `engines/fonts/` qua `engines_root()`, sys =
  `/System/Library/Fonts`) — AllFonts sinh + engine fetch không đổi hành vi; chỉ thêm
  log ADDITIVE `font serve: … from <disk path>` (filesrv.rs — log contract gate 9/10
  nguyên vẹn). 2 consumer: filesrv serve URL loopback + save.rs ghi js_raw (disk path)
  cho x2t — cả hai tự theo resolver, hết path app.
- **License gate S6 (mỗi set — name table sfnt + LICENSE.txt kèm, oracle-obs 02.10):**

| Set | File | License | Bằng chứng trong font file | License text bundle |
|---|---|---|---|---|
| Open Sans | OpenSans-{Regular,Light,Semibold,ExtraBold,Bold}.ttf | Apache 2.0 | nameID 8 "Ascender Corporation", nameID 14 `apache.org/licenses/LICENSE-2.0` | root `LICENSE.txt` |
| Carlito | crosextra/Carlito-{Regular,Italic,Bold,BoldItalic}.ttf | SIL OFL 1.1 | nameID 8 "tyPoland Lukasz Dziedzic", nameID 14 `scripts.sil.org/OFL` | `crosextra/LICENSE.txt` |
| Caladea | caladea/caladea-{regular,bold,italic,bolditalic}.ttf | SIL OFL 1.1 (dual Apache 2.0) | nameID 14 `apache.org/licenses/LICENSE-2.0`; dir LICENSE.txt = OFL 1.1 | `caladea/LICENSE.txt` |
| Asana Math | asana/ASANA.ttc | SIL OFL 1.1 | dir LICENSE.txt (TTC — name table đọc qua face) | `asana/LICENSE.txt` |
| OpenSymbol | openoffice/opens___.ttf | SIL OFL 1.0 | dir LICENSE.txt | `openoffice/LICENSE.txt` |

- **Đủ license redistribution kèm license text** (OFL/Apache — không phải AGPL; font giữ
  license riêng). Bundle full root (4.9 MB) thay vì chỉ Carlito fixture-fetch: giữ bộ
  substitute family cho tài liệu thật trên máy sạch (Cambria→Caladea, bullet→OpenSymbol,
  equation→Asana Math). Discovery fetch (oracle-obs 44 log gate, đủ 4 format + save):
  chỉ Carlito ×4 được fetch từ root app — xlsx + pptx (theme default Calibri).
- **Trim engines (T-A12, best-effort):** `engines/editors/sdkjs-plugins` = 38 MB — 0
  request HTTP trong mọi log gate (serviceworker tham chiếu prefix cũng 0 fetch) → loại
  khỏi bản copy engines/editors sau (quy trình assembly: `engines/README.md` — local,
  uncommitted). Local giữ nguyên. Engines tổng ~431 MB (editors 313 = sdkjs 161 +
  web-apps 114 + plugins 38 · converter 113 · fonts 4.9).

## Font contract FULL desktop build (mới 27.09 — quyết định cho PH-0)

- **Engine (word build 9.4):** CUỐI `engines/editors/sdkjs/word/sdk-all.js` (~offset 9.9 MB)
  định nghĩa LẠI `AscFonts.sje.prototype.BSa` = XHR async **`"ascdesktop://fonts/" + this.Va`**
  (`responseType="arraybuffer"`; onload: push `new AscFonts.G2b(bytes,len)` vào
  `AscFonts.Hcc`, `wOb(index)`, `UB=0`; lỗi `UB=1`; `UB=-1` = chưa nạp, `2` = đang nạp).
  `Va` = path gốc trong `window.__fonts_files` (AllFonts.js). `AscCommon.jM` (font manager,
  `XFg="../../../../fonts/"`) pump queue `tVa` poll 50 ms; `AscFonts.fU.YD(name)` tra font.
- **Shell thật:** CEF chặn scheme `ascdesktop://fonts/<abs disk path>` trả bytes font
  (runtime observed: mở fixture → `Hcc=15` streams UB=0 — Arial/Courier New/Times New Roman).
- **NanoOffice:** Rust filesrv rewrite path font → loopback URL khi serve AllFonts.js
  (`protocol/mod.rs` `rewrite_font_paths`); shim patch `AscFonts.sje.prototype.BSa` ngay sau
  khi eval sdk-all.js (`patchDesktopFontLoader` trong `src/shim/desktopEditor.js`) — same-origin
  XHR tới URL đó, nạp cùng registry/status/callback. Re-implement từ behavior, không copy code.
- **`LoadFontBase64`** (wrapper cpp:1475): contract sync `window["<path>"]="<size>;<base64>"`
  (decoder engine `zP.decode(s,true)` parse size prefix trước `;`) — nhưng word 9.4 không gọi;
  để STUB quan sát. Font nhúng (`embedded*`): shell eval `<convert folder>/fonts/<name>.js`
  (cpp:1475 nhánh else) — PH-1.

## Desktop save contract (word, 9.4.0 — mới 28.09, T-010)

Luồng save offline của shell thật — tĩnh (desktop-sdk + sdkjs, file:line) VÀ động (CDP oracle
trên ONLYOFFICE.app thật 28.09, mở fixture → `asc_AddText` → `asc_Save`):

1. Engine `asc_Save` (build desktop: `upstream/sdkjs/word/Local/api.js:155`) →
   `CoAuthoringApi.askSaveChanges` (stub offline trả `{saveLock:false}` đồng bộ —
   `upstream/sdkjs/common/Local/common.js:411`) → `_onSaveCallback` (`apiBase.js:1654`) →
   `_onSaveCallbackInner` (`word/api.js:2667`) → `CollaborativeEditing.Send_Changes`
   (override word: `word/Editor/CollaborativeEditing.js:76` → `saveChanges` :152).
2. Engine → shell: **`LocalFileSaveChanges(joinedChanges, deleteIndex, count)`**
   (`common/Local/common.js:418`; chuỗi = các change `"<len>;<base64>"` join bằng `","`;
   cpp append vào `<recover>/changes/changes0.json` — wrapper cpp:2119–2152, truncate khi
   undo: `RemoveChanges` cpp:5218 cắt tại dấu quote thứ `(openIndex+deleteIndex)*2`).
   Sync-truth `true`.
3. Engine → shell: **`LocalFileSave(_param, password, docinfo, fileType, jsonOptions,
   oldPassword)`** (`word/Local/api.js:264`; `_param` chứa `saveas=true;` khi SaveAs;
   `DesktopOfflineAppDocumentStartSave` :214). Wrapper cpp:2174 → process message
   `onlocaldocument_onsavestart` → `cefview.cpp:3099` → không cần dialog → `LocalFile_SaveStart`
   (cefview.cpp:1409) → thread `CASCFileConverterFromEditor` (`fileconverter.h:936`):
   chạy **x2t với file params XML** (`<TaskQueueDataConvert>`: `<m_sFileFrom>` = `<recover>/Editor.bin`
   (open-time) hoặc `EditorWithChanges.bin` (crypto), `<m_sFileTo>` = path gốc,
   `<m_nFormatTo>` (DOCX = 65 = 0x0041), `<m_bFromChanges>true</m_bFromChanges>` khi
   `changes/changes0.json` tồn tại, `<m_sAllFontsPath>`, `<m_sTempDir>`, `<m_bDontSaveAdditional>true`)
   → **bytes KHÔNG đi qua bridge — x2t đọc Editor.bin + changes từ đĩa và ghi file gốc**.
4. Shell → engine (wrapper eval vào editor frame, cpp:6544–6583 sau `onlocaldocument_onsaveend`):
   `window.AscDesktopEditor.LocalFileSetSaved(true|false)` →
   `window.AscDesktopEditor.LocalFileSetSourcePath("<path gốc>")` →
   **`window.DesktopOfflineAppDocumentEndSave(<0 ok | 2 error>)`** (engine: `word/Local/api.js:269`
   — tắt block-action Save; error 2 → `ConvertationSaveError`). Shell thật bắn EndSave 2 lần
   (oracle 28.09) — engine chịu lặp.
5. Engine sau EndSave(0): `DesktopOfflineUpdateLocalName` (`common/Local/common.js:393`) →
   `LocalFileGetSourcePath()` + `SetDocumentName(<name>)` (oracle quan sát đúng chuỗi này).
6. Kèm theo trong oracle: `onDocumentModifiedChanged(true|false)`,
   `execCommand('title:button', {"disabled":{"save":false|true}})` — **UI đẩy trạng thái nút LÊN
   shell** (web-apps `Desktop.js:259–281`), shell KHÔNG đẩy xuống để enable Save; nút Save toolbar
   là api-driven (`asc_onDocumentModifiedChanged`), `file:save` native → `api.asc_Save()`
   (Desktop.js:225). `OnSave()` ack (word/api.js:2710, wrapper cpp:1757).
   `CheckNeedWheel()` sync-truth `true` (wrapper cpp:1879, word gọi ở scroll paths).
   `Property_Get/SetCryptoMode` = getter/setter property `AscDesktopEditor.CryptoMode`
   (wrapper cpp:2675, 4024; mặc định 0).

Hệ quả kiến trúc: save REVERSE không là `x2t Editor.bin out.docx` thuần (mất edits) — edits
sống trong `changes0.json`; x2t phải nhận params XML với `<m_bFromChanges>true</m_bFromChanges>`
và `<m_sFileFrom>` trỏ vào thư mục chứa `changes/`.


- Engine phân loại document URL theo chuỗi scheme: editorscommon.js:3088 (`getUrlType`) +
  allowlist `:2939` `(^((https?|ftps?|file|tessa|joplin|smb)://)|(mailto:))` → **custom scheme
  (`nanooffice-file://`) bị đánh Invalid/Unsafe, engine không bao giờ fetch**.
- Desktop-mode mở file OOXML cần `window.Asc.Addons.ooxml === true`
  (apiBase.js:5217 `asc_isSupportFeature`) — shell set (tương đương `CSdkjsAddons::CheckLocal`).
- Custom-scheme page = secure context → fetch `http://` bị chặn mixed-content → engine assets +
  fonts + documents phải **cùng origin**.
- **Deviation kiến trúc PH-0:** toàn bộ engine + documents serve qua loopback
  `http://127.0.0.1:14321` (Rust `protocol/filesrv.rs`; URL document chỉ chứa fileId opaque,
  disk path không bao giờ ra JS). `nanooffice-engine://` + `nanooffice-file://` vẫn đăng ký.
  Quyết định dài hạn chốt ở PH-1 khi làm sidecar.

## Bridge JS→Native (observed, lower-bound ~130 method — bảng khảo sát 21.09)

| Nhóm | Method | Call-sites tiêu biểu | Trạng thái NanoOffice |
|---|---|---|---|
| File | `OpenFilenameDialog` (20), `LocalFileGetImageUrl` (22), `LocalFileSaveDialog`, `loadLocalFile`, `GetDropFiles`, `convertFile`, `startExternalConvertation`, `ResaveFile` | `sdkjs/common/apiBase.js` | STUB (PH-1+; riêng `LocalFileSave` + `LocalFileSaveChanges` đã DONE-PH1 — xem bảng trên) |
| Print | `Print`, `Print_Start/Page/End`, `IsFilePrinting`, `emulateCloudPrinting` (apiBase.js:5684), `localSaveToDrawingFormat` (:5698) | apiBase.js | STUB (PH-2) |
| Fonts/media | `LoadFontBase`, `GetImageBase`, `GetImageFormat`, `GetImageOriginalSize`, `MediaStart` (:3495), `CallMediaPlayerCommand`, `AddVideo/Audio` | apiBase.js | STUB (PH-2) |
| Spell | `SpellCheck` (:2385,2391,2466), `getDictionariesPath` | apiBase.js | STUB (spell.wasm có sẵn — khả năng không cần native) |
| Clipboard | `asc_desktop_copypaste` | `sdkjs/common/clipboard_base.js:1943` | STUB (PH-2) |
| Crypto | `CryptoMode` (12 calls), `GetSupportCryptoModes`, `OpenFileCrypt` (:1422), `buildCryptedStart/End`, `CryptoDownloadAs`, `PreloadCryptoImage` | apiBase.js | STUB (PH-3) |
| Signature | `Sign` (:4036), `SelectCertificate`, `ViewCertificate`, `GetDefaultCertificate`, `Remove(All)Signature(s)` | apiBase.js | STUB (PH-3) |
| System | `SetFullscreen`, `SetLocalRestrictions`, `sendSystemMessage`, `CallInAllWindows`, `updateVersion` | apiBase.js | STUB/partial |
| System (ghi chú) | `isSupportNetworkFunctionality` | web-apps Main.js:483 — để undefined = network enabled | không cần |
| Viewer/merge | `NativeViewerOpen`, `OpenWorkbook`, `MergeDocumentUrl/File`, `CompareDocumentUrl/File`, `SetAdvancedOptions` (10) | apiBase.js | STUB (cắt được) |
| Cloud/login | `GetExternalClouds`, `updateClouds`, `checkAuth`, `portal:*`, `GetLocalFeatures` | `desktop-apps/common/loginpage/src/sdk.js` | STUB (PH-3, cắt được) |
| Recents (login) | `LocalFileRecents/Recovers/Templates`, `LocalFileOpenRecent/OpenRecover`, `LocalFileRemove*`×4 | loginpage sdk.js | STUB (PH-2) |

## Native→JS (observed)

- Router: `window.on_native_message(cmd, param)` — 13 cmd (`style:change`, `window:features`,
  `editor:config`, `button:click`, `theme:changed`, `uitheme:added`, `renderervars:changed`,
  `element:show`, `althints:show`, `file:print`, `printer:config`, `file:saveas`, `file:save`)
  → `upstream/web-apps/apps/common/main/lib/controller/Desktop.js:79-228`.
  **Desktop.js tự override `on_native_message` khi `AscDesktopEditor` tồn tại, và bắn
  `editor:config 'request'` qua execCommand (Desktop.js:141)** — native app-layer phải trả lời.
- Boot page desktop: `web-apps/apps/api/documents/index.html` đọc query params
  (`url/title/filetype/key/doctype/mode/lang/username/uitheme/type`, `placement=desktop`) và tự
  tạo `DocsAPI.DocEditor` — ứng với `local_editors_path` (applicationmanager.cpp:89);
  `GetFileUrlParams` (applicationmanager_p.h:2890) chỉ truyền `placement+doctype[+filetype]`,
  không truyền url → url mặc định `_offline_`.
- Global events ×8: `onchildframemessage`, `onupdaterecents`, `onupdaterecovers`,
  `on_check_auth`, `onChangeCryptoMode`, `onfeaturesavailable`, `onaddtemplates`
  → `loginpage/src/sdk.js:38-47`.
- Env object: `window.RendererProcessVariable` → consumed `web-apps/apps/common/main/lib/util/desktopinit.js:51-78`.
- Engine-global mở file offline: `window.DesktopOfflineAppDocumentEndLoad(folder, b64, len)`
  (sdkjs word build) — dùng `AscCommon.zP.decode` + `f$c` magic `DOCY` + `WKa.At`.

## CEF-independence (observed 21.09 + xác nhận thực tế 22.09)

- Grep `cefQuery|cefRequest|CefMessageRouter|CefRegisterExtension|CefV8Handler|OnWebKitInitialized|chrome.webview` trên sdkjs + web-apps: **0 match** (LC_ALL=C).
- WASM ship sẵn: `sdkjs/common/libfont/engine/fonts.wasm`, `common/zlib/engine/zlib.wasm`, `common/hash/hash/engine.wasm`, `common/spell/spell/spell.wasm`, `pdf/src/engine/drawingfile.wasm`.
- **PH-0 runtime:** sdkjs + web-apps + fonts.wasm + UI chạy trong Tauri WKWebView, không CEF.

## Điểm cần clone thêm (khi làm gì thì clone nấy)

| Việc | Repo cần | Lý do |
|---|---|---|
| Shim bridge đầy đủ | `ONLYOFFICE/desktop-sdk` ✅ | nguồn chân lý native surface + cơ chế inject |
| Build engine/x2t từ source | `ONLYOFFICE/build_tools` | pipeline deps + WASM + packaging |
| Mac app layer (editor:config answer, recents push, app lifecycle) | repo mac của ONLYOFFICE (chưa xác định) | fidelity UI desktop còn thiếu: editor:config reply, `onupdaterecents` push, dialog lỗi iLa từ viewer api (diagnostics 27.09) |
