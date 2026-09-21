# docs/reference.map.md

Bản đồ bằng chứng: upstream code ↔ nanooffice cần làm gì. Trạng thái: observed (có path) /
candidate (cần kiểm chứng). Cập nhật mỗi khi phát hiện mới.

## Nguồn upstream (chỉ đọc qua submodule hoặc onlyoffice-study)

| Thành phần | Vị trí | Ghi chú |
|---|---|---|
| sdkjs (engine editing) | `upstream/sdkjs` (submodule) | bản clone đủ dùng: `~/.../onlyoffice-study/sdkjs` |
| web-apps (UI) | `upstream/web-apps` (submodule) | bản clone đủ dùng: `~/.../onlyoffice-study/web-apps` |
| core (x2t C++) | chỉ có bản clone study | `~/.../onlyoffice-study/core` |
| desktop-apps (shell cũ) | chỉ có bản clone study | tham khảo khi cần xem shell cũ làm gì |
| desktop-sdk (CEF wrapper) | **CHƯA CÓ** | ưu tiên clone khi làm bridge — nguồn chân lý mặt native |
| build_tools | **CHƯA CÓ** | cần khi tự build engine/x2t |

## Bridge JS→Native (observed, lower-bound ~130 method)

Điểm into: `upstream/sdkjs/common/apiBase.js:266` — `window["AscDesktopEditor"]["CreateEditorApi"](this)`.

Nhóm method + call-sites chính (trích từ khảo sát 21.09.2026):

| Nhóm | Method | Call-sites tiêu biểu | Trạng thái NanoOffice |
|---|---|---|---|
| File | `IsLocalFile` (44 calls), `OpenFilenameDialog` (20), `LocalFileGetImageUrl` (22), `LocalFileSave(D)`, `ResaveFile`, `loadLocalFile`, `LocalStartOpen`, `GetDropFiles`, `convertFile`, `startExternalConvertation` | `sdkjs/common/apiBase.js` | ⬜ STUB |
| Print | `Print`, `Print_Start/Page/End`, `IsFilePrinting`, `emulateCloudPrinting` (apiBase.js:5684), `localSaveToDrawingFormat` (:5698) | apiBase.js | ⬜ |
| Fonts/media | `LoadFontBase`, `GetImageBase`, `GetImageFormat`, `GetImageOriginalSize`, `MediaStart` (:3495), `CallMediaPlayerCommand`, `AddVideo/Audio` | apiBase.js | ⬜ |
| Spell | `SpellCheck` (:2385,2391,2466), `getDictionariesPath` | apiBase.js | ⬜ (spell.wasm có sẵn — khả năng không cần native) |
| Clipboard | `asc_desktop_copypaste` | `sdkjs/common/clipboard_base.js:1943` | ⬜ |
| Crypto | `CryptoMode` (12 calls), `GetSupportCryptoModes`, `OpenFileCrypt` (:1422), `buildCryptedStart/End`, `CryptoDownloadAs`, `PreloadCryptoImage` | apiBase.js | ⬜ (phạm vi PH-3, có thể cắt) |
| Signature | `Sign` (:4036), `SelectCertificate`, `ViewCertificate`, `GetDefaultCertificate`, `Remove(All)Signature(s)`, `IsSignaturesSupport` | apiBase.js | ⬜ (PH-3) |
| System | `SetDocumentName`, `SetFullscreen`, `SetLocalRestrictions`, `sendSystemMessage`, `CallInAllWindows`, `CreateEditorApi`, `getEngineVersion` | apiBase.js | ⬜ |
| Plugins | `PluginInstall/Uninstall`, `GetInstallPlugins`, `isSupportPlugins`, `isSupportMacroses` | apiBase.js | ⬜ |
| Viewer/merge | `NativeViewerOpen`, `OpenWorkbook`, `MergeDocumentUrl/File`, `CompareDocumentUrl/File`, `SetAdvancedOptions` (10) | apiBase.js | ⬜ |
| Cloud/login | `GetExternalClouds`, `updateClouds`, `checkAuth`, `portal:*`, `GetLocalFeatures` | `desktop-apps/common/loginpage/src/sdk.js` | ⬜ (PH-3) |
| Recents (login) | `LocalFileRecents/Recovers/Templates`, `LocalFileOpenRecent/OpenRecover`, `LocalFileRemove*`×4 | loginpage sdk.js | ⬜ |

## Native→JS (observed)

- Router: `window.on_native_message(cmd, param)` — 13 cmd (`style:change`, `window:features`,
  `editor:config`, `button:click`, `theme:changed`, `uitheme:added`, `renderervars:changed`,
  `element:show`, `althints:show`, `file:print`, `printer:config`, `file:saveas`, `file:save`)
  → `upstream/web-apps/apps/common/main/lib/controller/Desktop.js:79-228`.
- Global events ×8: `onchildframemessage`, `onupdaterecents`, `onupdaterecovers`, `on_check_auth`,
  `onChangeCryptoMode`, `onfeaturesavailable`, `onaddtemplates` → `loginpage/src/sdk.js:38-47`.
- Env object: `window.RendererProcessVariable` → consumed `web-apps/apps/common/main/lib/util/desktopinit.js:51-78`.

## CEF-independence (observed 21.09.2026)

- Grep `cefQuery|cefRequest|CefMessageRouter|CefRegisterExtension|CefV8Handler|OnWebKitInitialized|chrome.webview`
  trên sdkjs + web-apps: **0 match** (đã xác minh LC_ALL=C).
- WASM ship sẵn: `sdkjs/common/libfont/engine/fonts.wasm`, `common/zlib/engine/zlib.wasm`,
  `common/hash/hash/engine.wasm`, `common/spell/spell/spell.wasm`, `pdf/src/engine/drawingfile.wasm`.

## Điểm cần clone thêm (khi làm gì thì clone nấy)

| Việc | Repo cần | Lý do |
|---|---|---|
| Shim bridge đầy đủ | `ONLYOFFICE/desktop-sdk` | nguồn chân lý surface native + cơ chế inject |
| Build engine/x2t từ source | `ONLYOFFICE/build_tools` | pipeline deps + WASM + packaging |
