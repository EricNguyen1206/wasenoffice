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
| `LocalFileSetSaved` / `LocalFileRecoverFolder` / `LocalFileSetSourcePath` / `LocalFileGetSourcePath` / `LocalFileSetOpenChangesCount` | desktop-sdk wrapper cpp `onlocaldocument_loadend` sequence (6396–6485) | **DONE-PH0** (ack; GetSourcePath trả path đã set) |
| `LocalStartOpen` | sdkjs Local/common.js:64, cefview.cpp:3013 | **PARTIAL** (ack + khởi delivery chain) |
| Còn lại (~115 method) | bảng gốc bên dưới | STUB (UNIMPL logged khi bị gọi) |

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

**Điểm treo PH-0:** sau bước 4 pipeline dừng im lặng — không `asc_onError`, không
`asc_onDocumentContentReady`, không fetch fonts, `id_viewer` canvas 0 height. Data đã kiểm
chứng đúng (probe Uint8Array 1586 bytes head DOCY; roundtrip x2t giữ nguyên nội dung).
Nghi vấn: thủ tục async app-layer (editor:config answer) hoặc docsco/offline flag chưa khớp.
Chi tiết: `docs/diagnostics.md` 22.09.

## Ràng buộc URL (mới 22.09, quan trọng cho PH-1)

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
| File | `OpenFilenameDialog` (20), `LocalFileGetImageUrl` (22), `LocalFileSave(D)`, `ResaveFile`, `loadLocalFile`, `GetDropFiles`, `convertFile`, `startExternalConvertation` | `sdkjs/common/apiBase.js` | STUB (PH-1) |
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
| Mac app layer (editor:config answer, app lifecycle) | repo mac của ONLYOFFICE (chưa xác định) | giải mã điểm treo open pipeline |
