/**
 * Bridge shim — JS side of `window.AscDesktopEditor` (Tauri).
 *
 * Plain script (no imports/exports) so one file serves as:
 *   1. Tauri initialization script injected into ALL frames (src-tauri/src/lib.rs)
 *   2. vite side-effect import on the start page (src/main.ts)
 *
 * Frame topology (PH-0): start page (main frame, has __TAURI__) → DocsAPI boot
 * page iframe → editor iframe. The engine lives in the GRANDCHILD.
 *
 * Contract (harness/skills/bridge-shim/SKILL.md):
 * - Method names EXACTLY as upstream (case-sensitive).
 * - Sync-returning methods are answered locally (LOCAL_TRUTHS) — the CEF shell
 *   answers those synchronously and an async bridge cannot (call-sites use them
 *   inside `if` conditions / at script load). Values mirror the Rust dispatch
 *   in src-tauri/src/bridge/mod.rs — keep both in sync.
 * - Everything else relays to the main frame → Rust invoke; UNIMPL rejections
 *   stay visible instead of silently breaking.
 * - Document delivery follows the desktop open contract (desktop-sdk
 *   client_renderer_wrapper.cpp "onlocaldocument_loadend"): LocalStartOpen
 *   (engine → native) → Rust reads the registered file → main frame relays the
 *   bytes back down → engine frame calls window.DesktopOfflineAppDocumentEndLoad.
 *   The delivery chain itself lives in src/shim/delivery.js (T-045) — this file
 *   only triggers it on LocalStartOpen and publishes the shared plumbing
 *   (`window.__wasenoffice_shim`: tauriInvoke/forwardLog) that delivery.js
 *   consumes. Run order matters: this file loads FIRST in every frame (init
 *   script order in src-tauri/src/lib.rs, import order in src/main.ts).
 */
(function (window) {
  'use strict';

  if (window.__wasenoffice_bridge_installed) return;
  window.__wasenoffice_bridge_installed = true;

  // Methods observed in upstream call-sites (lower bound, docs/reference.map.md).
  var KNOWN_METHODS = [
    // file group (PH-0 wave)
    'IsLocalFile', 'OpenFilenameDialog', 'LocalFileGetImageUrl', 'LocalFileSave',
    'LocalFileSaveDialog', 'LocalFileGetOpenChangesCount', 'LocalStartOpen',
    'loadLocalFile', 'GetDropFiles', 'convertFile', 'startExternalConvertation',
    'ResaveFile', 'CreateEditorApi',
    // desktop local-open delivery (ack'd; bytes flow via DesktopOfflineAppDocumentEndLoad)
    'LocalFileSetSaved', 'LocalFileGetSaved', 'LocalFileRecoverFolder', 'LocalFileSetSourcePath',
    'LocalFileGetSourcePath', 'LocalFileSetOpenChangesCount',
    'LocalFileGetOpenChangesCount', 'onDocumentModifiedChanged', 'CheckUserId',
    // offline-endload streaming (T-067): EndLoad receives `binary_content://`
    // and pulls the bytes through this method — answered from the ArrayBuffer
    // stashed by delivery.js (LOCAL_TRUTH: the engine calls it synchronously).
    'GetOpenedFile',
    // desktop local-save (T-010: LocalFileSaveChanges appends deltas, LocalFileSave
    // triggers the Rust x2t conversion; result returns via DesktopOfflineAppDocumentEndSave)
    'LocalFileSaveChanges', 'OnSave', 'CheckNeedWheel',
    'Property_GetCryptoMode', 'Property_SetCryptoMode',
    // desktop recents (see LOCAL_TRUTHS note)
    'LocalFileRecents', 'LocalFileRecovers', 'LocalFileOpenRecent', 'LocalFileOpenRecover',
    // fonts/media
    'LoadFontBase', 'LoadFontBase64', 'GetImageBase', 'GetImageFormat',
    'GetImageOriginalSize', 'MediaStart', 'CallMediaPlayerCommand', 'AddVideo', 'AddAudio',
    // print
    'Print', 'Print_Start', 'Print_Page', 'Print_End', 'IsFilePrinting',
    'emulateCloudPrinting', 'SetPdfCloudPrintFileInfo', 'localSaveToDrawingFormat',
    'localSaveToDrawingFormat2',
    // spell/clipboard
    'SpellCheck', 'getDictionariesPath', 'asc_desktop_copypaste',
    // crypto/signature (PH-3)
    'GetCryptoMode', 'SetCryptoMode', 'GetSupportCryptoModes', 'OpenFileCrypt',
    'CryptoDownloadAs', 'buildCryptedStart', 'buildCryptedEnd', 'PreloadCryptoImage',
    'GetEncryptedHeader', 'Sign', 'SelectCertificate', 'ViewCertificate',
    'GetDefaultCertificate', 'RemoveSignature', 'RemoveAllSignatures', 'IsSignaturesSupport',
    // system/window
    'SetDocumentName', 'SetFullscreen', 'SetLocalRestrictions', 'sendSystemMessage',
    'CallInAllWindows', 'getEngineVersion', 'GetExternalClouds', 'updateClouds',
    'checkAuth', 'GetLocalFeatures', 'updateVersion',
    // plugins
    'PluginInstall', 'PluginUninstall', 'GetInstallPlugins', 'isSupportPlugins', 'isSupportMacroses',
    // viewer/merge
    'NativeViewerOpen', 'OpenWorkbook', 'MergeDocumentUrl', 'MergeDocumentFile',
    'CompareDocumentUrl', 'CompareDocumentFile', 'SetAdvancedOptions',
  ];

  var BRIDGE_CMD = 'asc_desktop_editor';
  var RELAY_TYPE = '__wasenoffice_bridge';   // engine frame → main frame (log + dispatch)
  var LOG_TYPE = '__wasenoffice_log';        // any frame → main frame (stdout)
  // DOWN_TYPE ('__wasenoffice_down', main frame → engine frame document
  // delivery) lives in src/shim/delivery.js — the only consumer of that lane.

  /**
   * Methods answered WITHOUT a Rust round-trip. The upstream editor calls these
   * synchronously inside `if` conditions or at script load — an async answer
   * would always read falsy and break the open flow. Keep in sync with the
   * Rust dispatch (src-tauri/src/bridge/mod.rs). Evidence per entry lives in
   * docs/reference.map.md.
   */
  var LOCAL_TRUTHS = {
    IsLocalFile: function () { return true; },
    getEngineVersion: function () { return '0.1.0'; },
    CreateEditorApi: function () { return null; },
    execCommand: function () { return null; },
    GetEncryptedHeader: function () { return 'ENCRYPTED;'; },
    CheckUserId: function () { return 'wasenoffice-ph0'; },
    // EndLoad's binary_content:// companion (T-067): returns the ArrayBuffer
    // stashed by delivery.js for the freshest delivery; null → the engine
    // raises ConvertationOpenError (same fail-loud as the real shell).
    GetOpenedFile: function () {
      var d = window.__wasenoffice_delivery;
      return (d && typeof d.getOpenedFile === 'function' && d.getOpenedFile()) || null;
    },
    GetInstallPlugins: function () {
      // CPluginsManager::GetPluginsJson shape: [systemFolder, userFolder]
      return "[{\"url\":\"/editors/sdkjs-plugins/\",\"pluginsData\":[{\"name\":\"Photo Editor\",\"nameLocale\":{\"ru\":\"\\u0424\\u043e\\u0442\\u043e\\u0440\\u0435\\u0434\\u0430\\u043a\\u0442\\u043e\\u0440\",\"fr\":\"Photo Editor\",\"es\":\"Editor de Fotos\",\"pt-BR\":\"Editor de foto\",\"de\":\"Foto-Editor\",\"si\":\"\\u0da1\\u0dcf\\u0dba\\u0dcf\\u0dbb\\u0dd6\\u0db4 \\u0dc3\\u0d82\\u0dc3\\u0dca\\u0d9a\\u0dbb\\u0d9a\\u0dba\",\"it\":\"Editor di foto\",\"sq-AL\":\"Editori i fotove\",\"sr-Cyrl-RS\":\"\\u0423\\u0440\\u0435\\u0452\\u0438\\u0432\\u0430\\u0447 \\u0424\\u043e\\u0442\\u043e\\u0433\\u0440\\u0430\\u0444\\u0438\\u0458\\u0430\",\"sr-Latn-RS\":\"Ure\\u0111iva\\u010d Fotografija\",\"zh-ZH\":\"\\u56fe\\u7247\\u7f16\\u8f91\\u5668\"},\"guid\":\"asc.{07FD8DFA-DFE0-4089-AL24-0730933CC80A}\",\"version\":\"1.0.2\",\"baseUrl\":\"\",\"variations\":[{\"description\":\"Edit images, screenshots, and photos right in your documents: crop, resize, apply effects.\",\"descriptionLocale\":{\"ru\":\"\\u041e\\u0431\\u0440\\u0430\\u0431\\u0430\\u0442\\u044b\\u0432\\u0430\\u0439\\u0442\\u0435 \\u043a\\u0430\\u0440\\u0442\\u0438\\u043d\\u043a\\u0438, \\u0441\\u043a\\u0440\\u0438\\u043d\\u0448\\u043e\\u0442\\u044b \\u0438 \\u0444\\u043e\\u0442\\u043e\\u0433\\u0440\\u0430\\u0444\\u0438\\u0438 \\u043f\\u0440\\u044f\\u043c\\u043e \\u0432 \\u0434\\u043e\\u043a\\u0443\\u043c\\u0435\\u043d\\u0442\\u0435: \\u043e\\u0431\\u0440\\u0435\\u0437\\u0430\\u0439\\u0442\\u0435, \\u0438\\u0437\\u043c\\u0435\\u043d\\u044f\\u0439\\u0442\\u0435 \\u0440\\u0430\\u0437\\u043c\\u0435\\u0440, \\u043f\\u0440\\u0438\\u043c\\u0435\\u043d\\u044f\\u0439\\u0442\\u0435 \\u0440\\u0430\\u0437\\u043b\\u0438\\u0447\\u043d\\u044b\\u0435 \\u044d\\u0444\\u0444\\u0435\\u043a\\u0442\\u044b.\",\"fr\":\"Modifiez des images, des captures d\\u2019\\u00e9cran et des photos directement dans vos documents\\u00a0: coupez, ajustez, appliquez des effets.\",\"es\":\"Edite im\\u00e1genes, capturas de pantalla y fotos directamente en sus documentos: recorte, cambie el tama\\u00f1o y aplique efectos.\",\"pt-BR\":\"Edite imagens, capturas de tela e fotos diretamente em seus documentos: corte, redimensione e aplique efeitos.\",\"de\":\"Bearbeiten Sie Bilder, Screenshots und Fotos direkt in Dokumenten: Zuschneiden, Gr\\u00f6\\u00dfe \\u00e4ndern, Effekte anwenden.\",\"si\":\"\\u0d94\\u0db6\\u0d9c\\u0dda \\u0dbd\\u0dda\\u0d9b\\u0db1\\u0dc0\\u0dbd \\u0d85\\u0da9\\u0d82\\u0d9c\\u0dd4 \\u0dbb\\u0dd6\\u0db4, \\u0dad\\u0dd2\\u0dbb\\u0dc3\\u0dda\\u0dba\\u0dcf \\u0dc3\\u0dc4 \\u0da1\\u0dcf\\u0dba\\u0dcf\\u0dbb\\u0dd6\\u0db4 \\u0dc3\\u0d82\\u0dc3\\u0dca\\u0d9a\\u0dbb\\u0dab\\u0dba \\u0d9a\\u0dbb\\u0db1\\u0dca\\u0db1. \\u0d9a\\u0db4\\u0dca\\u0db4\\u0dcf\\u0daf\\u0dd4\\u0dc0, \\u0dba\\u0dc5\\u0dd2 \\u0db4\\u0dca\\u200d\\u0dbb\\u0db8\\u0dcf\\u0dab\\u0db1\\u0dba, \\u0d86\\u0da0\\u0dbb\\u0dab \\u0dba\\u0dd9\\u0daf\\u0dd3\\u0db8 \\u0d86\\u0daf\\u0dd2\\u0dba \\u0d87\\u0dad\\u0dd4\\u0dc5\\u0dad\\u0dca\\u0dba.\",\"it\":\"Modifica immagini, screenshot e foto direttamente nei tuoi documenti: ritaglia, ridimensiona, applica effetti.\",\"sq-AL\":\"Editoni imazhe, foto ekrani dhe foto direkt.n\\u00eb dokumentet tuaja: prisni, rregulloni madh\\u00ebsin\\u00eb, aplikoni efekte\",\"sr-Cyrl-RS\":\"\\u0423\\u0440\\u0435\\u0434\\u0438\\u0442\\u0435 \\u0441\\u043b\\u0438\\u043a\\u0435, \\u0441\\u043d\\u0438\\u043c\\u043a\\u0435 \\u0435\\u043a\\u0440\\u0430\\u043d\\u0430 \\u0438 \\u0444\\u043e\\u0442\\u043e\\u0433\\u0440\\u0430\\u0444\\u0438\\u0458\\u0435 \\u0434\\u0438\\u0440\\u0435\\u043a\\u0442\\u043d\\u043e \\u0443 \\u0434\\u043e\\u043a\\u0443\\u043c\\u0435\\u043d\\u0442\\u0438\\u043c\\u0430: \\u0438\\u0441\\u0435\\u0446\\u0438\\u0442\\u0435, \\u043f\\u0440\\u043e\\u043c\\u0435\\u043d\\u0438\\u0442\\u0435 \\u0432\\u0435\\u043b\\u0438\\u0447\\u0438\\u043d\\u0443, \\u043f\\u0440\\u0438\\u043c\\u0435\\u043d\\u0438\\u0442\\u0435 \\u0435\\u0444\\u0435\\u043a\\u0442\\u0435.\",\"sr-Latn-RS\":\"Uredite slike, snimke ekrana i fotografije direktno u dokumentima: isecite, promenite veli\\u010dinu, primenite efekte.\",\"zh-ZH\":\"\\u76f4\\u63a5\\u5728\\u6587\\u6863\\u4e2d\\u7f16\\u8f91\\u56fe\\u50cf\\u3001\\u622a\\u56fe\\u548c\\u7167\\u7247\\uff1a\\u88c1\\u526a\\u3001\\u8c03\\u6574\\u5927\\u5c0f\\u3001\\u5e94\\u7528\\u6548\\u679c\\u3002\"},\"url\":\"index.html\",\"icons\":[\"resources/light/icon.png\",\"resources/light/icon@2x.png\"],\"icons2\":[{\"style\":\"light\",\"100%\":{\"normal\":\"resources/light/icon.png\"},\"125%\":{\"normal\":\"resources/light/icon@1.25x.png\"},\"150%\":{\"normal\":\"resources/light/icon@1.5x.png\"},\"175%\":{\"normal\":\"resources/light/icon@1.75x.png\"},\"200%\":{\"normal\":\"resources/light/icon@2x.png\"}},{\"style\":\"dark\",\"100%\":{\"normal\":\"resources/dark/icon.png\"},\"125%\":{\"normal\":\"resources/dark/icon@1.25x.png\"},\"150%\":{\"normal\":\"resources/dark/icon@1.5x.png\"},\"175%\":{\"normal\":\"resources/dark/icon@1.75x.png\"},\"200%\":{\"normal\":\"resources/dark/icon@2x.png\"}}],\"isViewer\":false,\"EditorsSupport\":[\"word\",\"cell\",\"slide\"],\"isVisual\":true,\"isModal\":true,\"isInsideMode\":false,\"initDataType\":\"html\",\"initData\":\"\",\"buttons\":[{\"text\":\"Ok\",\"primary\":true},{\"text\":\"Cancel\",\"primary\":false,\"isViewer\":false,\"textLocale\":{\"ru\":\"\\u041e\\u0442\\u043c\\u0435\\u043d\\u0430\",\"fr\":\"Annuler\",\"es\":\"Cancelar\",\"de\":\"Abbrechen\",\"si\":\"\\u0d85\\u0dc0\\u0dbd\\u0d82\\u0d9c\\u0dd4\",\"it\":\"Annulla\",\"sq-AL\":\"Anuloni\",\"sr-Cyrl-RS\":\"\\u041e\\u0442\\u043a\\u0430\\u0436\\u0438\",\"sr-Latn-RS\":\"Otka\\u017ei\",\"zh-ZH\":\"\\u53d6\\u6d88\"}}],\"size\":[10000,10000],\"store\":{\"background\":{\"light\":\"linear-gradient(180deg, #FF8E3D 0%, #FF6F3D 100%)\",\"dark\":\"linear-gradient(180deg, #FF8E3D 0%, #FF6F3D 100%)\"},\"screenshots\":[\"resources/store/screenshots/screen_1.png\",\"resources/store/screenshots/screen_2.png\"],\"icons\":{\"light\":\"resources/store/icons\",\"dark\":\"resources/store/icons\"},\"categories\":[\"specAbilities\",\"work\",\"recommended\"]}},{\"description\":\"About\",\"descriptionLocale\":{\"ru\":\"\\u0421\\u043f\\u0440\\u0430\\u0432\\u043a\\u0430\",\"fr\":\"\\u00c0 propos\",\"es\":\"Sobre programa\",\"de\":\"\\u00dcber\",\"si\":\"\\u0db4\\u0dd2\\u0dc5\\u0dd2\\u0db6\\u0db3\\u0dc0\",\"it\":\"Informazioni su\",\"sq-AL\":\"Rreth\",\"sr-Cyrl-RS\":\"\\u0423 \\u0432\\u0435\\u0437\\u0438\",\"sr-Latn-RS\":\"U vezi\"},\"url\":\"index_about.html\",\"icons\":[\"resources/img/icon.png\",\"resources/img/icon@2x.png\",\"resources/img/icon2.png\",\"resources/img/icon2@2x.png\"],\"isViewer\":false,\"EditorsSupport\":[\"word\",\"cell\",\"slide\"],\"isVisual\":true,\"isModal\":true,\"isInsideMode\":false,\"initDataType\":\"none\",\"initData\":\"\",\"size\":[392,147]}]},{\"name\":\"OCR\",\"nameLocale\":{\"ru\":\"\\u0420\\u0430\\u0441\\u043f\\u043e\\u0437\\u043d\\u0430\\u0432\\u0430\\u043d\\u0438\\u0435 \\u0442\\u0435\\u043a\\u0441\\u0442\\u0430\",\"fr\":\"Reconnaissance optique de caract\\u00e8res\",\"es\":\"Reconocimiento \\u00f3ptico de caracteres\",\"de\":\"OCR\",\"pt-BR\":\"OCR\",\"si\":\"OCR\",\"it\":\"OCR\",\"sq-AL\":\"OCR\",\"sr-Cyrl-RS\":\"OCR\",\"sr-Latn-RS\":\"OCR\",\"zh-ZH\":\"OCR \\u6587\\u5b57\\u8bc6\\u522b\"},\"guid\":\"asc.{440EBF13-9B19-4BD8-8621-05200E58140B}\",\"version\":\"2.1.0\",\"variations\":[{\"description\":\"Recognize text from pictures and screenshots and insert it into your documents.\",\"descriptionLocale\":{\"ru\":\"\\u0420\\u0430\\u0441\\u043f\\u043e\\u0437\\u043d\\u0430\\u0432\\u0430\\u0439\\u0442\\u0435 \\u0442\\u0435\\u043a\\u0441\\u0442 \\u0441 \\u0444\\u043e\\u0442\\u043e\\u0433\\u0440\\u0430\\u0444\\u0438\\u0439 \\u0438 \\u0441\\u043a\\u0440\\u0438\\u043d\\u0448\\u043e\\u0442\\u043e\\u0432 \\u0438 \\u0432\\u0441\\u0442\\u0430\\u0432\\u043b\\u044f\\u0439\\u0442\\u0435 \\u0435\\u0433\\u043e \\u0432 \\u0434\\u043e\\u043a\\u0443\\u043c\\u0435\\u043d\\u0442\\u044b.\",\"fr\":\"Reconnaissez le texte des images et des captures d'\\u00e9cran et ins\\u00e9rez-le dans vos documents.\",\"es\":\"Reconozca el texto de im\\u00e1genes y capturas de pantalla e ins\\u00e9rtelo en sus documentos.\",\"de\":\"Erkennen Sie Text auf Bildern und Screenshots und f\\u00fcgen Sie ihn in Ihre Dokumente ein.\",\"pt-BR\":\"Reconhe\\u00e7a texto de imagens e capturas de tela e insira-o em seus documentos.\",\"si\":\"\\u0da1\\u0dcf\\u0dba\\u0dcf\\u0dbb\\u0dd6\\u0db4 \\u0dc3\\u0dc4 \\u0dad\\u0dd2\\u0dbb\\u0dc3\\u0dda\\u0dba\\u0dcf\\u0dc0\\u0dbd \\u0d85\\u0da9\\u0d82\\u0d9c\\u0dd4 \\u0db4\\u0dd9\\u0dc5 \\u0dc4\\u0db3\\u0dd4\\u0db1\\u0dcf\\u0d9c\\u0dd9\\u0db1 \\u0d92\\u0dc0\\u0dcf \\u0d94\\u0db6\\u0d9c\\u0dda \\u0dbd\\u0dda\\u0d9b\\u0db1 \\u0dc0\\u0dbd\\u0da7 \\u0d87\\u0dad\\u0dd4\\u0dc5\\u0dd4 \\u0d9a\\u0dbb\\u0db1\\u0dca\\u0db1.\",\"it\":\"Riconosci il testo da immagini e screenshot e inseriscilo nei tuoi documenti.\",\"sq-AL\":\"Njihni tekstin n\\u00eb foto dhe skrinshotet dhe futeni n\\u00eb dokumentet tuaja.\",\"sr-Cyrl-RS\":\"\\u041f\\u0440\\u0435\\u043f\\u043e\\u0437\\u043d\\u0430\\u0458\\u0442\\u0435 \\u0442\\u0435\\u043a\\u0441\\u0442 \\u0441\\u0430 \\u0441\\u043b\\u0438\\u043a\\u0430 \\u0438 \\u0441\\u043d\\u0438\\u043c\\u0430\\u043a\\u0430 \\u0435\\u043a\\u0440\\u0430\\u043d\\u0430 \\u0438 \\u0443\\u0431\\u0430\\u0446\\u0438\\u0442\\u0435 \\u0433\\u0430 \\u0443 \\u0432\\u0430\\u0448\\u0435 \\u0434\\u043e\\u043a\\u0443\\u043c\\u0435\\u043d\\u0442\\u0435.\",\"sr-Latn-RS\":\"Prepoznajte tekst sa slika i snimaka ekrana i ubacite ga u va\\u0161e dokumente.\",\"zh-ZH\":\"\\u8bc6\\u522b\\u56fe\\u7247\\u548c\\u622a\\u56fe\\u4e2d\\u7684\\u6587\\u672c\\uff0c\\u5e76\\u5c06\\u5176\\u63d2\\u5165\\u5230\\u6587\\u6863\\u4e2d\\u3002\"},\"url\":\"index.html\",\"icons\":[\"resources/light/icon.png\",\"resources/light/icon@2x.png\"],\"icons2\":[{\"style\":\"light\",\"100%\":{\"normal\":\"resources/light/icon.png\"},\"125%\":{\"normal\":\"resources/light/icon@1.25x.png\"},\"150%\":{\"normal\":\"resources/light/icon@1.5x.png\"},\"175%\":{\"normal\":\"resources/light/icon@1.75x.png\"},\"200%\":{\"normal\":\"resources/light/icon@2x.png\"}},{\"style\":\"dark\",\"100%\":{\"normal\":\"resources/dark/icon.png\"},\"125%\":{\"normal\":\"resources/dark/icon@1.25x.png\"},\"150%\":{\"normal\":\"resources/dark/icon@1.5x.png\"},\"175%\":{\"normal\":\"resources/dark/icon@1.75x.png\"},\"200%\":{\"normal\":\"resources/dark/icon@2x.png\"}}],\"isViewer\":false,\"EditorsSupport\":[\"word\"],\"isVisual\":true,\"isModal\":true,\"isInsideMode\":false,\"initDataType\":\"none\",\"isUpdateOleOnResize\":false,\"buttons\":[{\"text\":\"Insert Text\",\"primary\":true,\"textLocale\":{\"ru\":\"\\u0412\\u0441\\u0442\\u0430\\u0432\\u0438\\u0442\\u044c \\u0442\\u0435\\u043a\\u0441\\u0442\",\"fr\":\"Ins\\u00e9rer le texte\",\"es\":\"Insertar texto\",\"de\":\"Text einf\\u00fcgen\",\"si\":\"\\u0db4\\u0dd9\\u0dc5 \\u0d87\\u0dad\\u0dd4\\u0dc5\\u0dd4 \\u0d9a\\u0dbb\\u0db1\\u0dca\\u0db1\",\"it\":\"Inserisci testo\",\"sq-AL\":\"Futni tekstin\",\"sr-Cyrl-RS\":\"\\u0423\\u0431\\u0430\\u0446\\u0438 \\u0442\\u0435\\u043a\\u0441\\u0442\",\"sr-Latn-RS\":\"Ubaci tekst\",\"zh-ZH\":\"\\u63d2\\u5165\\u6587\\u5b57\"}},{\"text\":\"Cancel\",\"primary\":false,\"textLocale\":{\"ru\":\"\\u041e\\u0442\\u043c\\u0435\\u043d\\u0430\",\"fr\":\"Annuler\",\"es\":\"Cancelar\",\"de\":\"Abbrechen\",\"si\":\"\\u0d85\\u0dc0\\u0dbd\\u0d82\\u0d9c\\u0dd4\",\"it\":\"Annulla\",\"sq-AL\":\"Anulo\",\"sr-Cyrl-RS\":\"\\u041e\\u0442\\u043a\\u0430\\u0436\\u0438\",\"sr-Latn-RS\":\"Otka\\u017ei\",\"zh-ZH\":\"\\u53d6\\u6d88\"}}],\"size\":[592,100],\"store\":{\"background\":{\"light\":\"#444444\",\"dark\":\"#444444\"},\"screenshots\":[\"resources/store/screenshots/screen_1.png\",\"resources/store/screenshots/screen_2.png\",\"resources/store/screenshots/screen_3.png\"],\"icons\":{\"light\":\"resources/store/icons\",\"dark\":\"resources/store/icons\"},\"categories\":[\"specAbilities\",\"work\"]}},{\"description\":\"About\",\"descriptionLocale\":{\"ru\":\"\\u0421\\u043f\\u0440\\u0430\\u0432\\u043a\\u0430\",\"fr\":\"\\u00c0 propos\",\"es\":\"Sobre programa\",\"de\":\"\\u00dcber\",\"si\":\"\\u0db4\\u0dd2\\u0dc5\\u0dd2\\u0db6\\u0db3\\u0dc0\",\"it\":\"Informazioni su\",\"sq-AL\":\"Rreth\",\"sr-Cyrl-RS\":\"\\u0423 \\u0432\\u0435\\u0437\\u0438\",\"sr-Latn-RS\":\"U vezi\",\"zh-ZH\":\"\\u5173\\u4e8e\"},\"url\":\"index_about.html\",\"icons\":[\"resources/img/icon.png\",\"resources/img/icon@2x.png\",\"resources/img/icon2.png\",\"resources/img/icon2@2x.png\"],\"isViewer\":false,\"EditorsSupport\":[\"word\"],\"isVisual\":true,\"isModal\":true,\"isInsideMode\":false,\"initDataType\":\"none\",\"isUpdateOleOnResize\":false,\"buttons\":[{\"text\":\"Ok\",\"primary\":true}],\"size\":[392,147]}]},{\"name\":\"Typograf\",\"nameLocale\":{\"ru\":\"\\u0422\\u0438\\u043f\\u043e\\u0433\\u0440\\u0430\\u0444\",\"fr\":\"Typograf\",\"es\":\"Typograf\",\"pt-BR\":\"Typograf\",\"de\":\"Typograf\",\"cs-CS\":\"Typograf\",\"sq-AL\":\"Typograf\",\"sr-Cyrl-RS\":\"Typograf\",\"sr-Latn-RS\":\"Typograf\",\"zh-ZH\":\"\\u6392\\u7248\\u5de5\\u5177\"},\"guid\":\"asc.{55159EC6-C91A-4DDB-8E1E-558454666461}\",\"version\":\"1.0.2\",\"variations\":[{\"description\":\"Prepare your texts for publishing by correcting typography.\",\"descriptionLocale\":{\"ru\":\"\\u041f\\u043e\\u0434\\u0433\\u043e\\u0442\\u043e\\u0432\\u044c\\u0442\\u0435 \\u0441\\u0432\\u043e\\u0438 \\u0442\\u0435\\u043a\\u0441\\u0442\\u044b \\u043a \\u043f\\u0443\\u0431\\u043b\\u0438\\u043a\\u0430\\u0446\\u0438\\u0438, \\u0438\\u0441\\u043f\\u0440\\u0430\\u0432\\u0438\\u0432 \\u0442\\u0438\\u043f\\u043e\\u0433\\u0440\\u0430\\u0444\\u0438\\u0447\\u0435\\u0441\\u043a\\u0438\\u0435 \\u043e\\u0448\\u0438\\u0431\\u043a\\u0438.\",\"fr\":\"Pr\\u00e9parez vos textes pour la publication en corrigeant la typographie.\",\"es\":\"Prepare sus textos para la publicaci\\u00f3n corrigiendo la tipograf\\u00eda.\",\"pt-BR\":\"Prepare seus textos para publica\\u00e7\\u00e3o corrigindo a tipografia.\",\"de\":\"Bereiten Sie Ihre Texte f\\u00fcr die Ver\\u00f6ffentlichung vor, indem Sie Typografie korrigieren.\",\"cs-CS\":\"P\\u0159ipravte sv\\u00e9 texty k publikov\\u00e1n\\u00ed opravou typografie.\",\"sq-AL\":\"P\\u00ebrgatitni tekstet tuaja p\\u00ebr botim duke korrigjuar tipografin\\u00eb.\",\"sr-Cyrl-RS\":\"\\u041f\\u0440\\u0438\\u043f\\u0440\\u0435\\u043c\\u0438\\u0442\\u0435 \\u0441\\u0432\\u043e\\u0458\\u0435 \\u0442\\u0435\\u043a\\u0441\\u0442\\u043e\\u0432\\u0435 \\u0437\\u0430 \\u043e\\u0431\\u0458\\u0430\\u0432\\u0459\\u0438\\u0432\\u0430\\u045a\\u0435 \\u0438\\u0441\\u043f\\u0440\\u0430\\u0432\\u0459\\u0430\\u045a\\u0435\\u043c \\u0442\\u0438\\u043f\\u043e\\u0433\\u0440\\u0430\\u0444\\u0438\\u0458\\u0435.\",\"sr-Latn-RS\":\"Pripremite svoje tekstove za objavljivanje ispravljanjem tipografije.\",\"zh-ZH\":\"\\u901a\\u8fc7\\u6821\\u6b63\\u6392\\u7248\\uff0c\\u4e3a\\u53d1\\u5e03\\u60a8\\u7684\\u6587\\u672c\\u505a\\u597d\\u51c6\\u5907\\u3002\"},\"url\":\"index.html\",\"icons\":[\"resources/img/icon.png\",\"resources/img/icon@2x.png\"],\"icons2\":[{\"100%\":{\"normal\":\"resources/img/icon.png\"},\"125%\":{\"normal\":\"resources/img/icon@1.25x.png\"},\"150%\":{\"normal\":\"resources/img/icon@1.5x.png\"},\"175%\":{\"normal\":\"resources/img/icon@1.75x.png\"},\"200%\":{\"normal\":\"resources/img/icon@2x.png\"}}],\"isViewer\":false,\"EditorsSupport\":[\"word\",\"slide\",\"cell\"],\"isVisual\":true,\"isModal\":false,\"isInsideMode\":true,\"initDataType\":\"text\",\"initData\":\"\",\"isUpdateOleOnResize\":false,\"buttons\":[],\"initOnSelectionChanged\":true,\"store\":{\"background\":{\"light\":\"#00ACC7\",\"dark\":\"#00ACC7\"},\"screenshots\":[\"resources/store/screenshots/screen_1.png\"],\"icons\":{\"light\":\"resources/store/icons\",\"dark\":\"resources/store/icons\"},\"categories\":[\"specAbilities\"]}}]},{\"name\":\"Doc2md\",\"nameLocale\":{\"ru\":\"Doc2md\",\"fr\":\"Doc2md\",\"es\":\"Doc2md\",\"pt-BR\":\"Doc2md\",\"de\":\"Doc2md\",\"sq-AL\":\"Doc2md\",\"sr-Cyrl-RS\":\"Doc2md\",\"sr-Latn-RS\":\"Doc2md\",\"zh-ZH\":\"Doc2md\"},\"guid\":\"asc.{5aff455d-96c4-4bd8-9658-16e5bc143738}\",\"version\":\"1.0.3\",\"minVersion\":\"7.4.0\",\"variations\":[{\"description\":\"Convert your formatted documents to Markdown or HTML.\",\"descriptionLocale\":{\"ru\":\"\\u041f\\u0440\\u0435\\u043e\\u0431\\u0440\\u0430\\u0437\\u0443\\u0439\\u0442\\u0435 \\u043e\\u0442\\u0444\\u043e\\u0440\\u043c\\u0430\\u0442\\u0438\\u0440\\u043e\\u0432\\u0430\\u043d\\u043d\\u044b\\u0435 \\u0434\\u043e\\u043a\\u0443\\u043c\\u0435\\u043d\\u0442\\u044b \\u0432 Markdown \\u0438\\u043b\\u0438 HTML.\",\"fr\":\"Convertissez votre document mis en forme en Markdown ou HTML.\",\"es\":\"Convierta sus documentos formateados a Markdown o HTML.\",\"pt-BR\":\"Converta seus documentos formatados para Markdown ou HTML.\",\"de\":\"Konvertieren Sie formatierte Dokumente in Markdown oder HTML.\",\"cs\":\"P\\u0159eve\\u010fte sv\\u00e9 form\\u00e1tovan\\u00e9 dokumenty na Markdown nebo HTML.\",\"sq-AL\":\"Konvertoni dokumentet tuaja t\\u00eb formatizuara n\\u00eb Markdown ose HTML.\",\"sr-Cyrl-RS\":\"\\u041a\\u043e\\u043d\\u0432\\u0435\\u0440\\u0442\\u0443\\u0458\\u0442\\u0435 \\u0432\\u0430\\u0448\\u0435 \\u0444\\u043e\\u0440\\u043c\\u0430\\u0442\\u0438\\u0440\\u0430\\u043d\\u0435 \\u0434\\u043e\\u043a\\u0443\\u043c\\u0435\\u043d\\u0442\\u0435 \\u0443 Markdown \\u0438\\u043b\\u0438 HTML.\",\"sr-Latn-RS\":\"Konvertujte va\\u0161e formatirane dokumente u Markdown ili HTML.\",\"zh-ZH\":\"\\u5c06\\u6587\\u6863\\u8f6c\\u6362\\u4e3a Markdown \\u6216 HTML \\u683c\\u5f0f\\u3002\"},\"url\":\"index.html\",\"icons\":[\"resources/light/icon.png\",\"resources/light/icon@2x.png\"],\"icons2\":[{\"style\":\"light\",\"100%\":{\"normal\":\"resources/light/icon.png\",\"hover\":\"resources/light/icon.png\",\"active\":\"resources/light/icon.png\"},\"125%\":{\"normal\":\"resources/light/icon@1.25x.png\"},\"150%\":{\"normal\":\"resources/light/icon@1.5x.png\"},\"175%\":{\"normal\":\"resources/light/icon@1.75x.png\"},\"200%\":{\"normal\":\"resources/light/icon@2x.png\"}},{\"style\":\"dark\",\"100%\":{\"normal\":\"resources/dark/icon.png\"},\"125%\":{\"normal\":\"resources/dark/icon@1.25x.png\"},\"150%\":{\"normal\":\"resources/dark/icon@1.5x.png\"},\"175%\":{\"normal\":\"resources/dark/icon@1.75x.png\"},\"200%\":{\"normal\":\"resources/dark/icon@2x.png\"}}],\"isViewer\":true,\"EditorsSupport\":[\"word\"],\"isVisual\":true,\"isModal\":true,\"isInsideMode\":true,\"initDataType\":\"text\",\"isUpdateOleOnResize\":false,\"initOnSelectionChanged\":true,\"store\":{\"background\":{\"light\":\"#333333\",\"dark\":\"#444444\"},\"screenshots\":[\"resources/store/screenshots/screen_1.png\",\"resources/store/screenshots/screen_2.png\"],\"icons\":{\"light\":\"resources/store/icons\",\"dark\":\"resources/store/icons\"},\"categories\":[\"specAbilities\",\"work\"]}},{\"description\":\"About\",\"descriptionLocale\":{\"ru\":\"\\u0421\\u043f\\u0440\\u0430\\u0432\\u043a\\u0430\",\"fr\":\"\\u00c0 propos\",\"es\":\"Sobre programa\",\"de\":\"\\u00dcber\",\"sq-AL\":\"Rreth\",\"zh-ZH\":\"\\u5173\\u4e8e\"},\"url\":\"index_about.html\",\"icons\":[\"resources/img/icon.png\",\"resources/img/icon@2x.png\",\"resources/img/icon.png\",\"resources/img/icon@2x.png\"],\"isViewer\":true,\"EditorsSupport\":[\"word\"],\"isVisual\":true,\"isModal\":true,\"isInsideMode\":false,\"initDataType\":\"none\",\"isUpdateOleOnResize\":false,\"buttons\":[{\"text\":\"Ok\",\"primary\":true}],\"size\":[400,50]}]},{\"name\":\"LanguageTool\",\"nameLocale\":{\"ru\":\"LanguageTool\",\"fr\":\"LanguageTool\",\"es\":\"LanguageTool\",\"pt-BR\":\"LanguageTool\",\"de\":\"LanguageTool\",\"sq-AL\":\"LanguageTool\",\"sr-Cyrl-RS\":\"LanguageTool\",\"sr-Latn-RS\":\"LanguageTool\",\"zh-ZH\":\"LanguageTool\"},\"guid\":\"asc.{7CDB02C9-A0BF-4B56-9A1A-71C860B8498F}\",\"version\":\"1.0.5\",\"variations\":[{\"description\":\"Improve spelling, grammar, and style in your texts.\",\"descriptionLocale\":{\"ru\":\"\\u0418\\u0441\\u043f\\u0440\\u0430\\u0432\\u043b\\u044f\\u0439\\u0442\\u0435 \\u043e\\u0440\\u0444\\u043e\\u0433\\u0440\\u0430\\u0444\\u0438\\u0447\\u0435\\u0441\\u043a\\u0438\\u0435, \\u0433\\u0440\\u0430\\u043c\\u043c\\u0430\\u0442\\u0438\\u0447\\u0435\\u0441\\u043a\\u0438\\u0435 \\u0438 \\u0441\\u0442\\u0438\\u043b\\u0438\\u0441\\u0442\\u0438\\u0447\\u0435\\u0441\\u043a\\u0438\\u0435 \\u043e\\u0448\\u0438\\u0431\\u043a\\u0438 \\u0432 \\u0432\\u0430\\u0448\\u0438\\u0445 \\u0434\\u043e\\u043a\\u0443\\u043c\\u0435\\u043d\\u0442\\u0430\\u0445.\",\"fr\":\"Am\\u00e9liorez l'orthographe, la grammaire et le style de vos textes.\",\"es\":\"Mejore la ortograf\\u00eda, la gram\\u00e1tica y el estilo de sus textos.\",\"pt-BR\":\"Melhore a ortografia, a gram\\u00e1tica e o estilo dos seus textos.\",\"de\":\"Verbessern Sie Rechtschreibung, Grammatik und Stil in Ihren Texten.\",\"sq-AL\":\"P\\u00ebrmir\\u00ebsoni ortografin\\u00eb, gramatik\\u00ebn dhe stilin n\\u00eb tekstet tuaja.\",\"sr-Cyrl-RS\":\"\\u041f\\u043e\\u0431\\u043e\\u0459\\u0448\\u0430\\u0458\\u0442\\u0435 \\u043f\\u0440\\u0430\\u0432\\u043e\\u043f\\u0438\\u0441, \\u0433\\u0440\\u0430\\u043c\\u0430\\u0442\\u0438\\u043a\\u0443 \\u0438 \\u0441\\u0442\\u0438\\u043b \\u0443 \\u0441\\u0432\\u043e\\u0458\\u0438\\u043c \\u0442\\u0435\\u043a\\u0441\\u0442\\u043e\\u0432\\u0438\\u043c\\u0430.\",\"sr-Latn-RS\":\"Pobolj\\u0161ajte pravopis, gramatiku i stil u svojim tekstovima.\",\"zh-ZH\":\"\\u6539\\u8fdb\\u6587\\u672c\\u7684\\u62fc\\u5199\\u3001\\u8bed\\u6cd5\\u548c\\u6837\\u5f0f\\u3002\"},\"url\":\"index.html\",\"icons\":[\"resources/light/icon.png\",\"resources/light/icon@2x.png\"],\"icons2\":[{\"style\":\"light\",\"100%\":{\"normal\":\"resources/light/icon.png\"},\"125%\":{\"normal\":\"resources/light/icon@1.25x.png\"},\"150%\":{\"normal\":\"resources/light/icon@1.5x.png\"},\"175%\":{\"normal\":\"resources/light/icon@1.75x.png\"},\"200%\":{\"normal\":\"resources/light/icon@2x.png\"}},{\"style\":\"dark\",\"100%\":{\"normal\":\"resources/dark/icon.png\"},\"125%\":{\"normal\":\"resources/dark/icon@1.25x.png\"},\"150%\":{\"normal\":\"resources/dark/icon@1.5x.png\"},\"175%\":{\"normal\":\"resources/dark/icon@1.75x.png\"},\"200%\":{\"normal\":\"resources/dark/icon@2x.png\"}}],\"isViewer\":true,\"EditorsSupport\":[\"word\",\"cell\",\"slide\",\"pdf\"],\"isVisual\":true,\"isModal\":true,\"isInsideMode\":true,\"initDataType\":\"text\",\"initData\":\"\",\"isUpdateOleOnResize\":false,\"buttons\":[],\"initOnSelectionChanged\":true,\"store\":{\"background\":{\"light\":\"#357EE5\",\"dark\":\"#357EE5\"},\"screenshots\":[\"resources/store/screenshots/screen_1.png\",\"resources/store/screenshots/screen_2.png\",\"resources/store/screenshots/screen_3.png\"],\"icons\":{\"light\":\"resources/store/icons\",\"dark\":\"resources/store/icons\"},\"categories\":[\"specAbilities\",\"work\",\"recommended\"]}},{\"description\":\"About\",\"descriptionLocale\":{\"ru\":\"\\u0421\\u043f\\u0440\\u0430\\u0432\\u043a\\u0430\",\"fr\":\"\\u00c0 propos\",\"es\":\"Sobre programa\",\"de\":\"\\u00dcber\",\"sq-AL\":\"Rreth\",\"sr-Cyrl-RS\":\"\\u0423 \\u0432\\u0435\\u0437\\u0438\",\"sr-Latn-RS\":\"U vezi\",\"zh-ZH\":\"\\u5173\\u4e8e\"},\"url\":\"index_about.html\",\"icons\":[\"resources/img/icon.png\",\"resources/img/icon@2x.png\",\"resources/img/icon.png\",\"resources/img/icon@2x.png\"],\"isViewer\":true,\"EditorsSupport\":[\"word\",\"cell\",\"slide\"],\"isVisual\":true,\"isModal\":true,\"isInsideMode\":false,\"initDataType\":\"none\",\"initData\":\"\",\"isUpdateOleOnResize\":false,\"buttons\":[{\"text\":\"Ok\",\"primary\":true}],\"size\":[400,50]}]},{\"name\":\"AI\",\"nameLocale\":{\"ru\":\"\\u0418\\u0418\",\"fr\":\"AI\",\"es\":\"AI\",\"de\":\"AI\",\"cs\":\"AI\",\"zh\":\"AI\",\"pt-BR\":\"AI\",\"sr-Cyrl-RS\":\"AI\",\"sr-Latn-RS\":\"AI\",\"ja-JA\":\"AI\",\"sq-AL\":\"AI\",\"it\":\"IA\",\"ar-SA\":\"AI\"},\"guid\":\"asc.{9DC93CDB-B576-4F0C-B55E-FCC9C48DD007}\",\"version\":\"3.2.2\",\"minVersion\":\"8.2.0\",\"variations\":[{\"description\":\"Use the AI chatbot to perform tasks which involve understanding or generating natural language or code.\",\"descriptionLocale\":{\"ru\":\"\\u0418\\u0441\\u043f\\u043e\\u043b\\u044c\\u0437\\u0443\\u0439\\u0442\\u0435 \\u0447\\u0430\\u0442-\\u0431\\u043e\\u0442 AI \\u0434\\u043b\\u044f \\u0432\\u044b\\u043f\\u043e\\u043b\\u043d\\u0435\\u043d\\u0438\\u044f \\u0437\\u0430\\u0434\\u0430\\u0447, \\u0441\\u0432\\u044f\\u0437\\u0430\\u043d\\u043d\\u044b\\u0445 \\u0441 \\u043f\\u043e\\u043d\\u0438\\u043c\\u0430\\u043d\\u0438\\u0435\\u043c \\u0438\\u043b\\u0438 \\u0433\\u0435\\u043d\\u0435\\u0440\\u0430\\u0446\\u0438\\u0435\\u0439 \\u0435\\u0441\\u0442\\u0435\\u0441\\u0442\\u0432\\u0435\\u043d\\u043d\\u043e\\u0433\\u043e \\u044f\\u0437\\u044b\\u043a\\u0430 \\u0438\\u043b\\u0438 \\u043a\\u043e\\u0434\\u0430.\",\"fr\":\"Utilisez le chatbot AI pour effectuer des t\\u00e2ches qui impliquent la compr\\u00e9hension ou la g\\u00e9n\\u00e9ration de langage naturel ou de code.\",\"es\":\"Utilice el chatbot AI para realizar tareas que impliquen la comprensi\\u00f3n o generaci\\u00f3n de lenguaje natural o de c\\u00f3digo.\",\"pt-BR\":\"Use o chatbot AI para realizar tarefas que envolvam compreens\\u00e3o ou gera\\u00e7\\u00e3o de linguagem ou c\\u00f3digo natural.\",\"de\":\"Verwenden Sie den AI-Chatbot, um Aufgaben auszuf\\u00fchren, die das Verstehen oder Generieren von nat\\u00fcrlicher Sprache oder Code beinhalten.\",\"cs\":\"Pou\\u017eijte chatbota AI k prov\\u00e1d\\u011bn\\u00ed \\u00fakol\\u016f, kter\\u00fd zahrnuje porozum\\u011bn\\u00ed nebo generov\\u00e1n\\u00ed p\\u0159irozen\\u00e9ho jazyka nebo k\\u00f3du.\",\"zh\":\"\\u4f7f\\u7528 AI \\u804a\\u5929\\u673a\\u5668\\u4eba\\u5b8c\\u6210\\u6709\\u5173\\u7406\\u89e3\\u3001\\u751f\\u6210\\u81ea\\u7136\\u8bed\\u8a00\\u6216\\u4ee3\\u7801\\u7684\\u4efb\\u52a1\\u3002\",\"sr-Cyrl-RS\":\"\\u041a\\u043e\\u0440\\u0438\\u0441\\u0442\\u0438\\u0442\\u0435 AI \\u0447\\u0435\\u0442 \\u0440\\u043e\\u0431\\u043e\\u0442\\u0430 \\u0437\\u0430 \\u043e\\u0431\\u0430\\u0432\\u0459\\u0430\\u045a\\u0435 \\u0437\\u0430\\u0434\\u0430\\u0442\\u0430\\u043a\\u0430 \\u043a\\u043e\\u0458\\u0438 \\u0443\\u043a\\u0459\\u0443\\u0447\\u0443\\u0458\\u0443 \\u0440\\u0430\\u0437\\u0443\\u043c\\u0435\\u0432\\u0430\\u045a\\u0435 \\u0438\\u043b\\u0438 \\u0433\\u0435\\u043d\\u0435\\u0440\\u0438\\u0441\\u0430\\u045a\\u0435 \\u043f\\u0440\\u0438\\u0440\\u043e\\u0434\\u043d\\u043e\\u0433 \\u0458\\u0435\\u0437\\u0438\\u043a\\u0430 \\u0438\\u043b\\u0438 \\u043a\\u043e\\u0434\\u0430.\",\"sr-Latn-RS\":\"Koristite AI \\u010det robota za obavljanje zadataka koji uklju\\u010duju razumevanje ili generisanje prirodnog jezika ili koda.\",\"ja-JA\":\"\\u81ea\\u7136\\u8a00\\u8a9e\\u3084\\u30b3\\u30fc\\u30c9\\u306e\\u7406\\u89e3\\u307e\\u305f\\u306f\\u751f\\u6210\\u304c\\u5fc5\\u8981\\u306a\\u30bf\\u30b9\\u30af\\u3092\\u884c\\u3046\\u306b\\u306f\\u3001AI\\u30c1\\u30e3\\u30c3\\u30c8\\u30dc\\u30c3\\u30c8\\u3092\\u4f7f\\u7528\\u3067\\u304d\\u307e\\u3059\\u3002\",\"sq-AL\":\"Shtoni dhe selektoni modele AI p\\u00ebr detyra t\\u00eb ndryshme.\",\"it\":\"Utilizza il chatbot dell'IA per eseguire attivit\\u00e0 che implicano la comprensione o la generazione di codice o linguaggio naturale.\",\"ar-SA\":\"\\u0627\\u0633\\u062a\\u062e\\u062f\\u0645\\u0648\\u0627 \\u0631\\u0648\\u0628\\u0648\\u062a \\u0627\\u0644\\u0645\\u062d\\u0627\\u062f\\u062b\\u0629 \\u0627\\u0644\\u0630\\u0643\\u064a \\u0644\\u062a\\u0646\\u0641\\u064a\\u0630 \\u0627\\u0644\\u0645\\u0647\\u0627\\u0645 \\u0627\\u0644\\u062a\\u064a \\u062a\\u062a\\u0637\\u0644\\u0628 \\u0641\\u0647\\u0645\\u064b\\u0627 \\u0623\\u0648 \\u0625\\u0646\\u062a\\u0627\\u062c\\u064b\\u0627 \\u0644\\u0644\\u063a\\u0629 \\u0627\\u0644\\u0637\\u0628\\u064a\\u0639\\u064a\\u0629 \\u0623\\u0648 \\u0627\\u0644\\u0628\\u0631\\u0645\\u062c\\u0629.\"},\"url\":\"index.html\",\"icons\":\"resources/%theme-type%(light|dark)/icon%scale%(default).%extension%(png)\",\"isViewer\":false,\"EditorsSupport\":[\"word\",\"cell\",\"slide\",\"pdf\"],\"type\":\"background\",\"initDataType\":\"none\",\"buttons\":[],\"events\":[\"onAIPluginSettings\",\"onContextMenuShow\",\"onContextMenuClick\",\"onToolbarMenuClick\"],\"store\":{\"background\":{\"light\":\"linear-gradient(90deg, #F9B6FF 0%, #E370EE 102.01%)\",\"dark\":\"linear-gradient(90deg, #F9B6FF 0%, #E370EE 102.01%)\"},\"screenshots\":[\"resources/store/screenshots/screen_1.png\",\"resources/store/screenshots/screen_2.png\",\"resources/store/screenshots/screen_3.png\",\"resources/store/screenshots/screen_4.png\",\"resources/store/screenshots/screen_5.png\",\"resources/store/screenshots/screen_6.png\"],\"icons\":{\"light\":\"resources/store/icons\",\"dark\":\"resources/store/icons\"},\"categories\":[\"specAbilities\",\"work\",\"recommended\"]}}],\"onlyofficeScheme\":true},{\"name\":\"AI agent\",\"nameLocale\":{\"ru\":\"\\u0418\\u0418-\\u0430\\u0433\\u0435\\u043d\\u0442\",\"it\":\"Agente AI\",\"fr\":\"Agent IA\",\"es\":\"Agente de IA\",\"de\":\"KI-Agent\",\"pt-BR\":\"Agente de IA\",\"pl\":\"AI agent\",\"cs\":\"Agent AI\",\"sk\":\"AI agent\",\"zh\":\"AI \\u667a\\u80fd\\u4f53\",\"ca\":\"Agente de IA\",\"el\":\"\\u0395\\u03ba\\u03c0\\u03c1\\u03cc\\u03c3\\u03c9\\u03c0\\u03bf\\u03c2 AI\",\"gl\":\"Axente de IA\",\"ko\":\"AI \\uc5d0\\uc774\\uc804\\ud2b8\",\"nl\":\"AI agent\",\"uk\":\"\\u0428\\u0406-\\u0430\\u0433\\u0435\\u043d\\u0442\",\"ar-SA\":\"\\u0648\\u0643\\u064a\\u0644 \\u0627\\u0644\\u0630\\u0643\\u0627\\u0621 \\u0627\\u0644\\u0627\\u0635\\u0637\\u0646\\u0627\\u0639\\u064a\",\"he\":\"\\u05e1\\u05d5\\u05db\\u05df \\u05d1\\u05d9\\u05e0\\u05d4 \\u05de\\u05dc\\u05d0\\u05db\\u05d5\\u05ea\\u05d9\\u05ea\",\"hy\":\"\\u0531\\u0532 \\u0563\\u0578\\u0580\\u056e\\u0561\\u056f\\u0561\\u056c\",\"ja\":\"AI\\u30a8\\u30fc\\u30b8\\u30a7\\u30f3\\u30c8\",\"ro\":\"Agent de IA\",\"sq-AL\":\"Agjenti i IA-s\\u00eb\",\"sr-Latn\":\"AI agent\",\"sr-Cyrl\":\"\\u0412\\u0418 \\u0430\\u0433\\u0435\\u043d\\u0442\",\"ur\":\"\\u0627\\u06d2 \\u0622\\u0626\\u06cc \\u0627\\u06cc\\u062c\\u0646\\u0679\",\"da\":\"AI-agent\",\"no\":\"KI-agent\",\"pt-PT\":\"Agente de IA\"},\"guid\":\"asc.{9DC93CDB-B576-4F0C-B55E-FCC9C48DD777}\",\"variations\":[{\"description\":\"ai-agent\",\"url\":\"index.html\",\"initDataType\":\"desktop-external\",\"initData\":\"none\"}],\"onlyofficeScheme\":true},{\"name\":\"Plugin Manager\",\"nameLocale\":{\"ru\":\"\\u041c\\u0435\\u043d\\u0435\\u0434\\u0436\\u0435\\u0440 \\u043f\\u043b\\u0430\\u0433\\u0438\\u043d\\u043e\\u0432\",\"fr\":\"Gestionnaire de Plugins\",\"it\":\"Plugin Manager\",\"ja\":\"\\u30d7\\u30e9\\u30b0\\u30a4\\u30f3\\u30de\\u30cd\\u30fc\\u30b8\\u30e3\\u30fc\",\"es\":\"Administrador de Complementos\",\"de\":\"Plugin-Verwaltung\",\"cs\":\"Spr\\u00e1vce Plugin\\u016f\",\"si\":\"\\u0db4\\u0dda\\u0db1\\u0dd4 \\u0d9a\\u0dc5\\u0db8\\u0db1\\u0dcf\\u0d9a\\u0dbb\\u0dd4\",\"zh\":\"\\u63d2\\u4ef6\\u7ba1\\u7406\\u5668\",\"pt-BR\":\"Gerenciador de plug-ins\",\"sq-AL\":\"Menaxheri i shtojc\\u00ebs\",\"sr-Cyrl-RS\":\"\\u041c\\u0435\\u043d\\u0430\\u045f\\u0435\\u0440 \\u0434\\u043e\\u0434\\u0430\\u0442\\u0430\\u043a\\u0430\",\"sr-Latn-RS\":\"Menad\\u017eer dodataka\"},\"guid\":\"asc.{AA2EA9B6-9EC2-415F-9762-634EE8D9A95E}\",\"version\":\"1.0.5\",\"group\":{\"name\":\"Plugin Manager\",\"rank\":1},\"loader\":false,\"variations\":[{\"description\":\"Plugin Manager\",\"url\":\"index.html\",\"icons\":[\"resources/light/icon.png\",\"resources/light/icon@2x.png\"],\"icons2\":[{\"style\":\"light\",\"100%\":{\"normal\":\"resources/light/icon.png\"},\"125%\":{\"normal\":\"resources/light/icon@1.25x.png\"},\"150%\":{\"normal\":\"resources/light/icon@1.5x.png\"},\"175%\":{\"normal\":\"resources/light/icon@1.75x.png\"},\"200%\":{\"normal\":\"resources/light/icon@2x.png\"},\"*\":{\"normal\":\"resources/light/icon.svg\"}},{\"style\":\"dark\",\"100%\":{\"normal\":\"resources/dark/icon.png\"},\"125%\":{\"normal\":\"resources/dark/icon@1.25x.png\"},\"150%\":{\"normal\":\"resources/dark/icon@1.5x.png\"},\"175%\":{\"normal\":\"resources/dark/icon@1.75x.png\"},\"200%\":{\"normal\":\"resources/dark/icon@2x.png\"},\"*\":{\"normal\":\"resources/dark/icon.svg\"}}],\"isViewer\":true,\"EditorsSupport\":[\"word\",\"cell\",\"slide\",\"pdf\"],\"isVisual\":true,\"isModal\":true,\"isInsideMode\":false,\"isSystem\":false,\"initDataType\":\"none\",\"isUpdateOleOnResize\":false,\"size\":[608,600],\"buttons\":[]}]},{\"name\":\"Send\",\"nameLocale\":{\"ru\":\"\\u041e\\u0442\\u043f\\u0440\\u0430\\u0432\\u0438\\u0442\\u044c\",\"fr\":\"Envoyer\",\"es\":\"Enviar\",\"de\":\"Senden\",\"si\":\"\\u0dba\\u0dc0\\u0db1\\u0dca\\u0db1\"},\"guid\":\"asc.{B509123E-6335-40BD-B965-91EB799346E3}\",\"variations\":[{\"description\":\"Send your documents via email as attachments using the default desktop mail client.\",\"descriptionLocale\":{\"ru\":\"\\u041e\\u0442\\u043f\\u0440\\u0430\\u0432\\u043b\\u044f\\u0439\\u0442\\u0435 \\u0434\\u043e\\u043a\\u0443\\u043c\\u0435\\u043d\\u0442\\u044b \\u043f\\u043e \\u044d\\u043b\\u0435\\u043a\\u0442\\u0440\\u043e\\u043d\\u043d\\u043e\\u0439 \\u043f\\u043e\\u0447\\u0442\\u0435 \\u0432 \\u0432\\u0438\\u0434\\u0435 \\u0432\\u043b\\u043e\\u0436\\u0435\\u043d\\u0438\\u0439, \\u0438\\u0441\\u043f\\u043e\\u043b\\u044c\\u0437\\u0443\\u044f \\u0441\\u0442\\u0430\\u043d\\u0434\\u0430\\u0440\\u0442\\u043d\\u044b\\u0439 \\u043f\\u043e\\u0447\\u0442\\u043e\\u0432\\u044b\\u0439 \\u043a\\u043b\\u0438\\u0435\\u043d\\u0442.\",\"fr\":\"Envoyez vos documents par e-mail en pi\\u00e8ces jointes \\u00e0 l'aide du client de messagerie par d\\u00e9faut.\",\"es\":\"Enviar sus documentos por correo electr\\u00f3nico como archivos adjuntos utilizando el cliente de correo de escritorio predeterminado.\",\"de\":\"Senden Sie Ihre Dokumente per E-Mail als Anhang mit dem Standard-Desktop-Mail-Client.\",\"si\":\"\\u0db4\\u0dd9\\u0dbb\\u0db1\\u0dd2\\u0db8\\u0dd2 \\u0dc0\\u0dd0\\u0da9\\u0dad\\u0dbd \\u0dad\\u0dd0\\u0db4\\u0dd0\\u0dbd\\u0dca \\u0d85\\u0db1\\u0dd4\\u0d9c\\u0dca\\u200d\\u0dbb\\u0dcf\\u0dc4\\u0d9a\\u0dba \\u0db7\\u0dcf\\u0dc0\\u0dd2\\u0dad\\u0dba\\u0dd9\\u0db1\\u0dca \\u0d87\\u0db8\\u0dd4\\u0dab\\u0dd4\\u0db8\\u0dca \\u0dbd\\u0dd9\\u0dc3 \\u0d94\\u0db6\\u0d9c\\u0dda \\u0dbd\\u0dda\\u0d9b\\u0db1 \\u0dba\\u0dc0\\u0db1\\u0dca\\u0db1.\"},\"url\":\"index.html\",\"icons\":[{\"style\":\"light\",\"100%\":{\"normal\":\"resources/light/icon.png\"},\"125%\":{\"normal\":\"resources/light/icon@1.25x.png\"},\"150%\":{\"normal\":\"resources/light/icon@1.5x.png\"},\"175%\":{\"normal\":\"resources/light/icon@1.75x.png\"},\"200%\":{\"normal\":\"resources/light/icon@2x.png\"}},{\"style\":\"dark\",\"100%\":{\"normal\":\"resources/dark/icon.png\"},\"125%\":{\"normal\":\"resources/dark/icon@1.25x.png\"},\"150%\":{\"normal\":\"resources/dark/icon@1.5x.png\"},\"175%\":{\"normal\":\"resources/dark/icon@1.75x.png\"},\"200%\":{\"normal\":\"resources/dark/icon@2x.png\"}}],\"isViewer\":true,\"EditorsSupport\":[\"word\",\"cell\",\"slide\"],\"isVisual\":false,\"isModal\":false,\"isInsideMode\":false,\"initDataType\":\"none\",\"initData\":\"\",\"isUpdateOleOnResize\":false,\"buttons\":[],\"store\":{\"background\":{\"light\":\"#D9F4FF\",\"dark\":\"#9ADAF3;\"},\"screenshots\":[\"resources/store/screenshots/screen_1.png\",\"resources/store/screenshots/screen_2.png\"],\"icons\":{\"light\":\"resources/store/icons\",\"dark\":\"resources/store/icons\"},\"categories\":[\"work\",\"specAbilities\"]}}]},{\"name\":\"Thesaurus\",\"nameLocale\":{\"ru\":\"\\u0421\\u0438\\u043d\\u043e\\u043d\\u0438\\u043c\\u044b\",\"fr\":\"Th\\u00e9saurus\",\"es\":\"Tesauro\",\"pt-BR\":\"Thesaurus\",\"de\":\"Thesaurus\",\"si\":\"\\u0db4\\u0dbb\\u0dca\\u0dba\\u0dcf\\u0dba \\u0d9a\\u0ddd\\u0dc2\\u0dba\",\"it\":\"Thesaurus\",\"sq-AL\":\"Thesaurus\",\"sr-Cyrl-RS\":\"Thesaurus\",\"sr-Latn-RS\":\"Thesaurus\",\"zh-ZH\":\"\\u540c\\u4e49\\u8bcd\\u8bcd\\u5178\"},\"guid\":\"asc.{BE5CBF95-C0AD-4842-B157-AC40FEDD9840}\",\"version\":\"1.0.3\",\"variations\":[{\"description\":\"Search for synonyms and antonyms of a word and replace it with the selected one.\",\"descriptionLocale\":{\"ru\":\"\\u041d\\u0430\\u0445\\u043e\\u0434\\u0438\\u0442\\u0435 \\u0441\\u0438\\u043d\\u043e\\u043d\\u0438\\u043c\\u044b \\u0438 \\u0430\\u043d\\u0442\\u043e\\u043d\\u0438\\u043c\\u044b \\u0434\\u043b\\u044f \\u0432\\u044b\\u0431\\u0440\\u0430\\u043d\\u043d\\u043e\\u0433\\u043e \\u0441\\u043b\\u043e\\u0432\\u0430.\",\"fr\":\"Recherchez les synonymes et les antonymes d'un mot et remplacez-le par le mot s\\u00e9lectionn\\u00e9.\",\"es\":\"Busque sin\\u00f3nimos y ant\\u00f3nimos de una palabra y sustit\\u00fayala por la seleccionada.\",\"pt-BR\":\"Pesquise sin\\u00f4nimos e ant\\u00f4nimos de uma palavra e substitua pela selecionada.\",\"de\":\"Finden Sie Synonyme und Antonyme eines Wortes und ersetzen Sie es durch die ausgew\\u00e4hlte Variante.\",\"si\":\"\\u0dc0\\u0da0\\u0db1\\u0dba\\u0d9a \\u0dc3\\u0db8\\u0dcf\\u0db1 \\u0db4\\u0daf \\u0dc3\\u0dc4 \\u0dc0\\u0dd2\\u0dbb\\u0dd4\\u0daf\\u0dca\\u0db0 \\u0db4\\u0daf \\u0dc3\\u0ddc\\u0dba\\u0dcf \\u0dad\\u0ddd\\u0dbb\\u0dcf\\u0d9c\\u0dad\\u0dca \\u0dc0\\u0da0\\u0db1\\u0dba\\u0d9a\\u0da7 \\u0db4\\u0dca\\u200d\\u0dbb\\u0dad\\u0dd2\\u0dc3\\u0dca\\u0dae\\u0dcf\\u0db4\\u0db1\\u0dba \\u0d9a\\u0dbb\\u0db1\\u0dca\\u0db1.\",\"it\":\"Cerca sinonimi e contrari di una parola e sostituiscila con quella selezionata.\",\"sq-AL\":\"K\\u00ebrkoni sinonime dhe antonime t\\u00eb nj\\u00eb fjale dhe z\\u00ebv\\u00ebnd\\u00ebsojeni at\\u00eb me fjal\\u00ebn e zgjedhur.\",\"sr-Cyrl-RS\":\"\\u041f\\u0440\\u043e\\u043d\\u0430\\u0452\\u0438\\u0442\\u0435 \\u0441\\u0438\\u043d\\u043e\\u043d\\u0438\\u043c\\u0435 \\u0438 \\u0430\\u043d\\u0442\\u043e\\u043d\\u0438\\u043c\\u0435 \\u0440\\u0435\\u0447\\u0438 \\u0438 \\u0437\\u0430\\u043c\\u0435\\u043d\\u0438\\u0442\\u0435 \\u0458\\u0435 \\u0438\\u0437\\u0430\\u0431\\u0440\\u0430\\u043d\\u043e\\u043c.\",\"sr-Latn-RS\":\"Prona\\u0111ite sinonime i antonime re\\u010di i zamenite je izabranom.\",\"zh-ZH\":\"\\u641c\\u7d22\\u67d0\\u4e2a\\u5355\\u8bcd\\u7684\\u540c\\u4e49\\u8bcd\\u548c\\u53cd\\u4e49\\u8bcd\\uff0c\\u5e76\\u5c06\\u5176\\u66ff\\u6362\\u4e3a\\u6240\\u9009\\u7684\\u8bcd\\u6c47\\u3002\"},\"url\":\"index.html\",\"icons\":[\"resources/light/icon.png\",\"resources/light/icon@2x.png\"],\"icons2\":[{\"style\":\"light\",\"100%\":{\"normal\":\"resources/light/icon.png\"},\"125%\":{\"normal\":\"resources/light/icon@1.25x.png\"},\"150%\":{\"normal\":\"resources/light/icon@1.5x.png\"},\"175%\":{\"normal\":\"resources/light/icon@1.75x.png\"},\"200%\":{\"normal\":\"resources/light/icon@2x.png\"}},{\"style\":\"dark\",\"100%\":{\"normal\":\"resources/dark/icon.png\"},\"125%\":{\"normal\":\"resources/dark/icon@1.25x.png\"},\"150%\":{\"normal\":\"resources/dark/icon@1.5x.png\"},\"175%\":{\"normal\":\"resources/dark/icon@1.75x.png\"},\"200%\":{\"normal\":\"resources/dark/icon@2x.png\"}}],\"isViewer\":true,\"EditorsSupport\":[\"word\",\"cell\",\"slide\",\"pdf\"],\"isVisual\":true,\"isModal\":false,\"isInsideMode\":true,\"initDataType\":\"text\",\"isUpdateOleOnResize\":false,\"initOnSelectionChanged\":true,\"store\":{\"background\":{\"light\":\"#F44725\",\"dark\":\"#F44725\"},\"screenshots\":[\"resources/store/screenshots/screen_1.png\"],\"icons\":{\"light\":\"resources/store/icons\",\"dark\":\"resources/store/icons\"},\"categories\":[\"specAbilities\"]}}]},{\"name\":\"Zotero\",\"nameLocale\":{\"ru\":\"Zotero\",\"fr\":\"Zotero\",\"es\":\"Zotero\",\"pt-BR\":\"Zotero\",\"de\":\"Zotero\",\"sq-AL\":\"Zotero\",\"sr-Cyrl-RS\":\"Zotero\",\"sr-Latn-RS\":\"Zotero\"},\"guid\":\"asc.{BFC5D5C6-89DE-4168-9565-ABD8D1E48711}\",\"version\":\"1.0.7\",\"minVersion\":\"9.3.0\",\"variations\":[{\"description\":\"Create bibliographies in the required style using Zotero service.\",\"descriptionLocale\":{\"ru\":\"\\u041e\\u0444\\u043e\\u0440\\u043c\\u043b\\u044f\\u0439\\u0442\\u0435 \\u0431\\u0438\\u0431\\u043b\\u0438\\u043e\\u0433\\u0440\\u0430\\u0444\\u0438\\u0447\\u0435\\u0441\\u043a\\u0438\\u0435 \\u0441\\u043f\\u0438\\u0441\\u043a\\u0438 \\u0432 \\u043d\\u0443\\u0436\\u043d\\u043e\\u043c \\u0441\\u0442\\u0438\\u043b\\u0435 \\u0441 \\u043f\\u043e\\u043c\\u043e\\u0449\\u044c\\u044e Zotero.\",\"fr\":\"Cr\\u00e9ez des bibliographies dans le style n\\u00e9cessaire avec Zotero.\",\"es\":\"Cree bibliograf\\u00edas en el estilo requerido utilizando el servicio Zotero.\",\"pt-BR\":\"Crie bibliografias no estilo desejado usando o servi\\u00e7o Zotero.\",\"de\":\"Erstellen Sie Literaturverzeichnisse im gew\\u00fcnschten Stil mittels Zotero.\",\"sq-AL\":\"Krijoni bibliografi n\\u00eb stilin e k\\u00ebrkuar duke p\\u00ebrdorur sh\\u00ebrbimin Zotero.\",\"sr-Cyrl-RS\":\"\\u041a\\u0440\\u0435\\u0438\\u0440\\u0430\\u0458\\u0442\\u0435 \\u0431\\u0438\\u0431\\u043b\\u0438\\u043e\\u0433\\u0440\\u0430\\u0444\\u0438\\u0458\\u0435 \\u0443 \\u043f\\u043e\\u0442\\u0440\\u0435\\u0431\\u043d\\u043e\\u043c \\u0441\\u0442\\u0438\\u043b\\u0443 \\u043a\\u043e\\u0440\\u0438\\u0441\\u0442\\u0435\\u045b\\u0438 Zotero \\u0443\\u0441\\u043b\\u0443\\u0433\\u0443.\",\"sr-Latn-RS\":\"Kreirajte bibliografije u potrebnom stilu koriste\\u0107i Zotero uslugu.\"},\"events\":[\"onContextMenuShow\",\"onContextMenuClick\"],\"url\":\"index.html\",\"icons\":[\"resources/light/icon.png\",\"resources/light/icon@2x.png\"],\"icons2\":[{\"style\":\"light\",\"100%\":{\"normal\":\"resources/light/icon.png\"},\"125%\":{\"normal\":\"resources/light/icon@1.25x.png\"},\"150%\":{\"normal\":\"resources/light/icon@1.5x.png\"},\"175%\":{\"normal\":\"resources/light/icon@1.75x.png\"},\"200%\":{\"normal\":\"resources/light/icon@2x.png\"}},{\"style\":\"dark\",\"100%\":{\"normal\":\"resources/dark/icon.png\"},\"125%\":{\"normal\":\"resources/dark/icon@1.25x.png\"},\"150%\":{\"normal\":\"resources/dark/icon@1.5x.png\"},\"175%\":{\"normal\":\"resources/dark/icon@1.75x.png\"},\"200%\":{\"normal\":\"resources/dark/icon@2x.png\"}}],\"isViewer\":false,\"EditorsSupport\":[\"word\"],\"initDataType\":\"text\",\"initData\":\"\",\"isVisual\":true,\"isModal\":false,\"isInsideMode\":true,\"isUpdateOleOnResize\":false,\"initOnSelectionChanged\":false,\"store\":{\"background\":{\"light\":\"#2D3037\",\"dark\":\"#2D3037\"},\"screenshots\":[\"resources/store/screenshots/screen_1.png\",\"resources/store/screenshots/screen_2.png\",\"resources/store/screenshots/screen_3.png\"],\"icons\":{\"light\":\"resources/store/icons\",\"dark\":\"resources/store/icons\"},\"categories\":[\"specAbilities\"]}}]},{\"name\":\"DeepL\",\"nameLocale\":{\"ru\":\"DeepL\",\"fr\":\"DeepL\",\"es\":\"DeepL\",\"pt-BR\":\"DeepL\",\"de\":\"DeepL\",\"sq-AL\":\"DeepL\",\"sr-Cyrl-RS\":\"DeepL\",\"sr-Latn-RS\":\"DeepL\",\"zh-ZH\":\"DeepL\"},\"guid\":\"asc.{b78a062b-e349-4634-8a44-99825600d299}\",\"version\":\"1.0.5\",\"variations\":[{\"description\":\"Translate the selected text into other languages using DeepL.\",\"descriptionLocale\":{\"ru\":\"\\u041f\\u0435\\u0440\\u0435\\u0432\\u043e\\u0434\\u0438\\u0442\\u0435 \\u0442\\u0435\\u043a\\u0441\\u0442 \\u043d\\u0430 \\u0434\\u0440\\u0443\\u0433\\u0438\\u0435 \\u044f\\u0437\\u044b\\u043a\\u0438 \\u0441 \\u043f\\u043e\\u043c\\u043e\\u0449\\u044c\\u044e \\u0441\\u0435\\u0440\\u0432\\u0438\\u0441\\u0430 DeepL.\",\"fr\":\"Traduisez un texte s\\u00e9lectionn\\u00e9 dans d\\u2019autres langues avec DeepL.\",\"es\":\"Traduzca el texto seleccionado a otros idiomas utilizando DeepL.\",\"pt-BR\":\"Traduza o texto selecionado para outros idiomas usando DeepL.\",\"de\":\"\\u00dcbersetzen Sie den ausgew\\u00e4hlten Text in andere Sprachen mit DeepL.\",\"sq-AL\":\"P\\u00ebrktheni tekstin e selektuar n\\u00eb gjuh\\u00eb t\\u00eb tjera duke p\\u00ebrdorur DeepL.\",\"sr-Cyrl-RS\":\"\\u041f\\u0440\\u0435\\u0432\\u0435\\u0434\\u0438\\u0442\\u0435 \\u043e\\u0434\\u0430\\u0431\\u0440\\u0430\\u043d\\u0438 \\u0442\\u0435\\u043a\\u0441\\u0442 \\u043d\\u0430 \\u0434\\u0440\\u0443\\u0433\\u0435 \\u0458\\u0435\\u0437\\u0438\\u043a\\u0435 \\u043a\\u043e\\u0440\\u0438\\u0441\\u0442\\u0435\\u045b\\u0438 DeepL.\",\"sr-Latn-RS\":\"Prevedite odabrani tekst na druge jezike koriste\\u0107i DeepL.\",\"zh-ZH\":\"\\u4f7f\\u7528 DeepL \\u5c06\\u6240\\u9009\\u6587\\u672c\\u7ffb\\u8bd1\\u6210\\u5176\\u4ed6\\u8bed\\u8a00\\u3002\"},\"url\":\"index.html\",\"icons\":[\"resources/light/icon.png\",\"resources/light/icon@2x.png\"],\"icons2\":[{\"theme\":\"flat\",\"style\":\"light\",\"100%\":{\"normal\":\"resources/light/icon.png\"},\"125%\":{\"normal\":\"resources/light/icon@1.25x.png\"},\"150%\":{\"normal\":\"resources/light/icon@1.5x.png\"},\"175%\":{\"normal\":\"resources/light/icon@1.75x.png\"},\"200%\":{\"normal\":\"resources/light/icon@2x.png\"},\"default\":{\"normal\":\"resources/light/icon.png\"}},{\"theme\":\"flatDark\",\"style\":\"dark\",\"100%\":{\"normal\":\"resources/dark/icon.png\"},\"125%\":{\"normal\":\"resources/dark/icon@1.25x.png\"},\"150%\":{\"normal\":\"resources/dark/icon@1.5x.png\"},\"175%\":{\"normal\":\"resources/dark/icon@1.75x.png\"},\"200%\":{\"normal\":\"resources/dark/icon@2x.png\"}}],\"isViewer\":true,\"EditorsSupport\":[\"word\",\"slide\",\"cell\",\"pdf\"],\"isVisual\":true,\"isModal\":false,\"isInsideMode\":true,\"initDataType\":\"text\",\"isUpdateOleOnResize\":false,\"initOnSelectionChanged\":true,\"store\":{\"background\":{\"light\":\"#042B48\",\"dark\":\"#042B48\"},\"screenshots\":[\"resources/store/screenshots/screen_1.png\"],\"icons\":{\"light\":\"resources/store/icons\",\"dark\":\"resources/store/icons\"},\"categories\":[\"specAbilities\",\"work\"]}}],\"onlyofficeScheme\":true}]},{\"url\":\"\",\"pluginsData\":[]}]";
    },
    isSupportPlugins: function () { return true; },
    isSupportMacroses: function () { return true; },
    SetDocumentName: function () { return null; },
    IsSignaturesSupport: function () { return false; },
    // local-open delivery acks (bytes themselves flow via DesktopOfflineAppDocumentEndLoad)
    LocalFileSetSaved: function (v) { localSaved = v !== false; return null; },
    LocalFileGetSaved: function () { return localSaved; },
    LocalFileRecoverFolder: function () { return null; },
    LocalFileSetSourcePath: function (p) { localSourcePath = p; return null; },
    LocalFileGetSourcePath: function () { return localSourcePath; },
    LocalFileSetOpenChangesCount: function (n) { localOpenChangesCount = n | 0; return null; },
    // sync-truth: PH-0 documents open clean (0 unsaved changes)
    LocalFileGetOpenChangesCount: function () { return 0; },
    // local-save (T-010). LocalFileSaveChanges is sync-answered true (the engine
    // ignores the return); the deltas themselves travel to Rust via the relay
    // (FIFO-ordered in the main frame) and land in <workdir>/changes/changes0.json.
    LocalFileSaveChanges: function () { return true; },
    // save-finished ack from the engine (fires before LocalFileSave)
    OnSave: function () { return null; },
    // legacy wheel-compat hook, sync `false ===` compared by the engine (scroll paths)
    CheckNeedWheel: function () { return true; },
    // crypto-mode property surface (AscDesktopEditor.CryptoMode getter/setter) — PH-3 = off
    Property_GetCryptoMode: function () { return 0; },
    Property_SetCryptoMode: function () { return null; },
    // desktop recents (fire-and-forget: the shell pushes lists back via the
    // onupdaterecents/onupdaterecovers globals — PH-2; must EXIST because the
    // editor UI calls them unconditionally after open — loginpage panels.js)
    LocalFileRecents: function () { return null; },
    LocalFileRecovers: function () { return null; },
    LocalFileOpenRecent: function () { return null; },
    LocalFileOpenRecover: function () { return null; },
    // modified-flag sink: the engine reports dirty state; the real shell tracks
    // it for the title/save UI (PH-1). Must exist — called right after open.
    onDocumentModifiedChanged: function () { return null; },
  };

  var localSourcePath = '';
  var localSaved = true; // fresh local open = saved (desktop-sdk m_bLocalIsSaved)
  var localOpenChangesCount = 0; // changes already applied at open (sync-truth 0 in PH-0/PH-1)

  function safeArgs(args) {
    try {
      var s = JSON.stringify(args);
      return s === undefined ? '[]' : (s.length > 500 ? s.slice(0, 500) + '…' : s);
    } catch (e) {
      return '[unserializable]';
    }
  }

  function tauriInvoke() {
    var tauri = window.__TAURI__;
    return (tauri && tauri.core && tauri.core.invoke) || null;
  }

  function isMainFrame() {
    return !!tauriInvoke();
  }

  function forwardLog(level, message) {
    var invoke = tauriInvoke();
    if (invoke) {
      invoke('frontend_log', { level: level, message: message }).catch(function () {});
    } else if (window.parent && window.parent !== window) {
      try {
        window.parent.postMessage({ type: LOG_TYPE, level: level, message: message }, '*');
      } catch (e) { /* diagnostic only */ }
    }
  }

  /** Every frame: route framed messages UP toward the main frame — LOG goes
   *  to stdout (frontend_log), RELAY goes to the Rust dispatch. The DOWN lane
   *  (document delivery + save-end) lives in src/shim/delivery.js with its own
   *  message listener; this relay only triggers the chain on LocalStartOpen.
   *  RELAY args are forwarded UNTRUNCATED (LocalFileSaveChanges deltas run
   *  ~KBs; safeArgs is for logging only). */
  function installRelay() {
    // FIFO for Rust-bound invokes: the engine sends LocalFileSaveChanges
    // (deltas) before LocalFileSave (convert); both are async IPC — without
    // the queue the convert could read the changes file before the append
    // lands. Ordered postMessage + chained invokes preserve the contract.
    var relayQueue = Promise.resolve();
    window.addEventListener('message', function (event) {
      var data = event.data;
      if (!data) return;

      if (data.type === LOG_TYPE) {
        var invoke = tauriInvoke();
        if (invoke) {
          invoke('frontend_log', { level: data.level, message: data.message }).catch(function () {});
        } else if (window.parent && window.parent !== window) {
          try { window.parent.postMessage(data, '*'); } catch (e) { /* diagnostic */ }
        }
        return;
      }

      if (data.type === RELAY_TYPE) {
        if (isMainFrame()) {
          if (data.method === 'LocalStartOpen') {
            // Engine is ready — start the delivery chain (src/shim/delivery.js).
            mainFrameInvoke('LocalStartOpen', data.args).catch(function () {});
            if (window.__wasenoffice_delivery) window.__wasenoffice_delivery.deliverDocument();
          } else {
            relayQueue = relayQueue.then(function () {
              return mainFrameInvoke(data.method, data.args);
            }).catch(function () {
              /* UNIMPL already logged; save/open relays are handled Rust-side */
            });
          }
        } else if (window.parent && window.parent !== window) {
          try { window.parent.postMessage(data, '*'); } catch (e) { /* diagnostic */ }
        }
        return;
      }

      // No DOWN branch here: that lane (__wasenoffice_down) is handled by
      // src/shim/delivery.js with its own message listener.
    });
  }

  function mainFrameInvoke(method, args) {
    var invoke = tauriInvoke();
    if (!invoke) {
      console.warn('[wasenoffice-bridge] __TAURI__ unavailable; method', method);
      return Promise.resolve(null);
    }
    // T-059: the main frame owns the document — `open_document` returned its
    // fileId (main.ts keeps it in window.__wasenoffice_file_id__). Every
    // relayed engine call is stamped with it so Rust resolves exactly that
    // registry entry instead of an implicit "latest" (quick-view windows
    // register files into the same registry).
    var payload = { args: args };
    if (window.__wasenoffice_file_id__ != null) payload.fileId = window.__wasenoffice_file_id__;
    return invoke(BRIDGE_CMD, { method: method, payload: payload }).catch(function (err) {
      var msg = String(err);
      if (msg.indexOf('UNIMPL:') === 0) {
        console.warn('[wasenoffice-bridge] UNIMPL hit:', msg, 'args:', safeArgs(args));
      }
      throw err;
    });
  }

  function installErrorForwarding() {
    // stack capture (T-047): trigger evidence for the engine's pre-contentReady error-dialog mapping
    window.addEventListener('error', function (ev) {
      var target = ev.target;
      var resource = target && (target.src || target.href);
      if (resource) {
        forwardLog('error', 'resource error: ' + resource);
        return;
      }
      var stack = ev.error && ev.error.stack ? ' | stack: ' + String(ev.error.stack).slice(0, 500) : '';
      forwardLog('error', ev.message + ' @ ' + ev.filename + ':' + ev.lineno + ':' + ev.colno + stack);
    }, true);
    window.addEventListener('unhandledrejection', function (ev) {
      var reason = ev.reason;
      var stack = reason && reason.stack ? ' | stack: ' + String(reason.stack).slice(0, 500) : '';
      forwardLog('error', 'unhandled rejection: ' + String(reason) + stack);
    });
  }

  /**
   * Font-file loader adaptation for the FULL desktop builds (PH-0 word, T-048
   * cell/slide — the pdf editor reuses the word build).
   *
   * Observed engine behavior (each sdkjs build, T-048): near the END of
   * sdk-all.js the font-file loader entry (`AscFonts.<loader>.prototype.<fn>`
   * aliased as LoadFontAsync) is rerouted to an async XHR of
   * `ascdesktop://fonts/<font path>` — a scheme the CEF desktop shell
   * intercepts natively and answers with the font bytes. The three builds
   * implement the IDENTICAL contract under different minified names:
   *
   *   build  loader proto + fn      status  path  cb   idx-set  stream  registry  glyph.wasm
   *   word   sje.prototype.BSa      UB      Va    sV   wOb      G2b     Hcc       Wg.vue
   *   cell   mye.prototype.cZa      GE      Wa    TR   iXb      uac     Nmc       hj.RIe
   *   slide  K8d.prototype.VMa      Pz      Na    gY   wFb      FTb     D3b       mi.Zie
   *
   * (glyph.wasm = the build's glyph-manager global + its wasm-ready handle —
   * the readiness predicate in delivery.js consumes these fields; adding a
   * build means one new row here and nowhere else.)
   *
   * Observed real-shell runtime: fonts land in the engine stream registry
   * (the build's registry array of stream objects) with loader status
   * 2 → 0 (loaded) / 1 (failed), stream index assigned via the loader's
   * idx-setter, and the loader's callbacks fired when the bytes are in.
   *
   * A Tauri WKWebView has no `ascdesktop://` scheme handler, so every font
   * XHR would die (CORS/unknown scheme) and the document open would wait on
   * fonts forever. This adapter swaps the SAME entry point for a same-origin
   * XHR of the font URL itself — the Rust filesrv already serves every font
   * listed in `__fonts_files` under its (rewritten) URL — then feeds the
   * registry/status/callback contract described above. Behavior is
   * re-implemented from runtime observation; no upstream code is copied.
   */
  var FONT_VARIANTS = [
    { loader: 'sje', fn: 'BSa', status: 'UB', path: 'Va', cb: 'sV', index: 'wOb', stream: 'G2b', registry: 'Hcc', glyph: 'Wg', wasm: 'vue' },
    { loader: 'mye', fn: 'cZa', status: 'GE', path: 'Wa', cb: 'TR', index: 'iXb', stream: 'uac', registry: 'Nmc', glyph: 'hj', wasm: 'RIe' },
    { loader: 'K8d', fn: 'VMa', status: 'Pz', path: 'Na', cb: 'gY', index: 'wFb', stream: 'FTb', registry: 'D3b', glyph: 'mi', wasm: 'Zie' },
  ];

  /** Detect the current sdkjs build's font-loader variant (null = unknown).
   *  Published on the shared shim surface for sibling modules (delivery.js). */
  function detectFontVariant() {
    var fm = window.AscFonts;
    if (!fm) return null;
    for (var i = 0; i < FONT_VARIANTS.length; i++) {
      var v = FONT_VARIANTS[i];
      if (fm[v.loader] && fm[v.loader].prototype && fm[v.registry]) return v;
    }
    return null;
  }

  function patchDesktopFontLoader() {
    if (!window.AscFonts) {
      forwardLog('warn', 'font loader patch: sdkjs font registry not found');
      return;
    }
    var variant = detectFontVariant();
    if (!variant) {
      forwardLog('warn', 'font loader patch: no known font-loader variant in this build');
      return;
    }
    var proto = window.AscFonts[variant.loader].prototype;
    if (proto.__wasenoffice_patched) return;
    proto.__wasenoffice_patched = true;
    proto[variant.fn] = function (_baseUrl, onDone) {
      var self = this;
      if (self[variant.status] !== -1) return true; // loading (2) / loaded (0) / failed (1)
      self[variant.status] = 2;
      self[variant.cb] = onDone || null;
      var xhr = new XMLHttpRequest();
      xhr.open('GET', self[variant.path], true); // same-origin loopback font URL
      xhr.responseType = 'arraybuffer';
      xhr.onload = function () {
        if (xhr.status !== 200 || !xhr.response) {
          self[variant.status] = 1; // failed (final state — the engine treats 1 as done)
          return;
        }
        var bytes = new Uint8Array(xhr.response);
        var registry = window.AscFonts[variant.registry];
        registry.push(new window.AscFonts[variant.stream](bytes, bytes.length));
        self[variant.index](registry.length - 1);
        self[variant.status] = 0;
        if (self[variant.cb]) { var cb = self[variant.cb]; self[variant.cb] = null; cb(); }
        if (self.externalCallback) self.externalCallback();
      };
      xhr.onerror = function () {
        self[variant.status] = 1;
        forwardLog('warn', 'font fetch failed: ' + String(self[variant.path]).slice(0, 110));
      };
      xhr.send();
      return false; // async load in flight
    };
    // sdk-all.js aliases the final loader fn onto LoadFontAsync by reference —
    // repoint the alias so both entry names reach the patched implementation.
    if (typeof proto.LoadFontAsync !== 'undefined') {
      proto.LoadFontAsync = proto[variant.fn];
    }
    forwardLog('info', 'desktop font loader patched for loopback delivery ('
      + variant.loader + '.' + variant.fn + ')');
  }

  function install() {
    if (window.AscDesktopEditor) return;

    var bridge = {};
    KNOWN_METHODS.forEach(function (method) {
      bridge[method] = function () {
        var args = Array.prototype.slice.call(arguments);
        var truth = LOCAL_TRUTHS[method];
        if (truth) {
          relay(method, args);
          return truth.apply(null, args);
        }
        relay(method, args);
        if (isMainFrame()) {
          return mainFrameInvoke(method, args);
        }
        // Engine/boot frames: the call has been relayed UP (the Rust dispatch
        // logs UNIMPL for methods it does not know); async shell work answers
        // through its own channel (e.g. DesktopOfflineAppDocumentEndSave), the
        // engine never consumes this return value — resolve instead of leaving
        // a floating rejection on every fire-and-forget call.
        console.warn('[wasenoffice-bridge] async relay (iframe):', method, safeArgs(args));
        return Promise.resolve(null);
      };
    });
    // Status channel web-apps uses heavily at boot (desktopinit.js, Desktop.js);
    // outside KNOWN_METHODS in upstream terms but part of the same contract.
    bridge.execCommand = function (cmd, json) {
      relay('execCommand', [cmd, json]);
      return LOCAL_TRUTHS.execCommand(cmd, json);
    };
    /**
     * sdk-all-min.js is a BOOTSTRAP: the real engine (sdk-all.js) loads through
     * this bridge method in the desktop shell (sdk-all-min rb(): return codes
     * 1 = sync, 2 = async with local_load_add completeLoad, else web fallback).
     * Without it the runtime ends up with an inconsistent dual-build state.
     */
    bridge.LoadJS = function (url) {
      var abs;
      try {
        abs = new URL(url, window.location.href).href;
      } catch (e) {
        abs = url;
      }
      fetch(abs)
        .then(function (r) { return r.text(); })
        .then(function (text) {
          // Emscripten glue resolves the wasm payload via scriptDirectory,
          // which is empty when the script runs through LoadJS eval (no
          // currentScript). Give the glue an explicit locateFile: engine
          // payloads live in fixed sdkjs/engine directories (by basename).
          window.Module = {
            locateFile: function (path) {
              var map = {
                'fonts.wasm': '/editors/sdkjs/common/libfont/engine/',
                'zlib.wasm': '/editors/sdkjs/common/zlib/engine/',
                'engine.wasm': '/editors/sdkjs/common/hash/hash/engine/',
                'drawingfile.wasm': '/editors/sdkjs/pdf/src/engine/',
                'spell.wasm': '/editors/sdkjs/common/spell/spell/',
              };
              return (map[path] || '/editors/sdkjs/common/libfont/engine/') + path;
            },
          };
          // jshint evil:true — the engine payload itself (AGPL artifact).
          (0, eval)(text);
          if (abs.indexOf('sdk-all.js') !== -1) {
            patchDesktopFontLoader();
            installPdfFixes();
          }
          var pending = window.__wasenoffice_pending_js;
          var done = pending && pending[url];
          if (done) {
            delete pending[url];
            done();
          }
          forwardLog('info', 'LoadJS executed: ' + abs.slice(0, 100));
        })
        .catch(function (err) {
          forwardLog('error', 'LoadJS failed: ' + abs + ' — ' + err);
        });
      return 2; // async; completeLoad fires when executed
    };
    // Crypto-mode property: the engine/plugins read AscDesktopEditor.CryptoMode
    // synchronously (desktop shell exposes it as a getter → Property_GetCryptoMode).
    // PH-3 = crypto off → constant 0.
    try {
      Object.defineProperty(bridge, 'CryptoMode', {
        get: function () { return LOCAL_TRUTHS.Property_GetCryptoMode(); },
        set: function (v) { LOCAL_TRUTHS.Property_SetCryptoMode(v); },
      });
    } catch (e) { /* non-fatal: property surface only */ }
    window.AscDesktopEditor = bridge;

    // Coordination tables for LoadJS (see rb() in sdk-all-min).
    window.local_load_add = function (store, name, url) {
      window.__wasenoffice_pending_js = window.__wasenoffice_pending_js || {};
      window.__wasenoffice_pending_js[url] = store.completeLoad;
      return store;
    };
    window.local_load_remove = function (url) {
      if (window.__wasenoffice_pending_js) delete window.__wasenoffice_pending_js[url];
    };


    // Native → JS channel stubs (real handlers arrive with PH-1/PH-2 work).
    if (!window.RendererProcessVariable) window.RendererProcessVariable = {};
    if (!window.on_native_message) {
      window.on_native_message = function (cmd, param) {
        console.debug('[wasenoffice-bridge] on_native_message (stub):', cmd, param);
      };
    }

    installRelay();
    installErrorForwarding();
    installTabProbe();
    installCompactChrome();
    installKeymapProbe();
    installKeymapApply();
    installKeytipReveal();
    try {
      installPdfFixes();
    } catch (e) {
      forwardLog('warn', 'installPdfFixes caught: ' + e);
    }
    // Shared plumbing for sibling shim modules: delivery.js loads AFTER this
    // file in every frame (init-script order in src-tauri/src/lib.rs, import
    // order in src/main.ts) and consumes exactly this surface. Keep minimal.
    window.__wasenoffice_shim = {
      tauriInvoke: tauriInvoke,
      forwardLog: forwardLog,
      detectFontVariant: detectFontVariant,
    };
    forwardLog('info', 'frame ready: ' + window.location.href.slice(0, 120));
  }

  /** T-A40 compact chrome (DD U01 slice 6): engine chrome yields to our
   *  one-row header [menu | filename | account]. All parts additive and
   *  behind WASEN_OFFICE_COMPACT_CHROME (kill switch `=0` in config.rs):
   *  1. CSS hide of the tabs strip (the frame that owns `#toolbar`):
   *     `#toolbar .tabs` display:none — Backbone tab state keeps working.
   *     NO DocsAPI/customization injection: `toolbarNoTabs` makes api.js load
   *     `index_loader.html` (api.js:1175-1178) — the splash-loader variant
   *     whose assets don't exist in our trimmed engine tree → dead boot
   *     (proven 17.10: 3 GETs then silence vs full open with plain index.html).
   *  2. Tab driver (every frame): `{type:'__wasenoffice_tab', action}`
   *     postMessage relayed DOWN the frame chain; the frame holding
   *     Common.NotificationCenter executes it — 'file' maps to
   *     menu:show/menu:hide, anything else to tab:set-active. Spike-proven
   *     17.10: `common=object nc=object` in the app frame (index.html) at t=2s.
   */
  var TAB_MSG_TYPE = '__wasenoffice_tab';

  function installTabProbe() {
    if (window.__WASEN_OFFICE_TAB_PROBE__ !== true) return;
    [2000, 6000].forEach(function (ms) {
      setTimeout(function () {
        var common = typeof window.Common;
        var nc = common !== 'undefined' && window.Common
          ? typeof window.Common.NotificationCenter
          : 'undefined';
        forwardLog('info', 'tab-probe: t=' + (ms / 1000) + 's common=' + common +
          ' nc=' + nc + ' frame=' + window.location.href.slice(0, 80));
      }, ms);
    });
  }

  function installCompactChrome() {
    if (window.__WASEN_OFFICE_COMPACT_CHROME__ !== true) return;
    installTabDriver();
    hideEngineTabs();
  }

  function installTabDriver() {
    window.addEventListener('message', function (ev) {
      var d = ev.data;
      if (!d || d.type !== TAB_MSG_TYPE || !d.action) return;
      var nc = window.Common && window.Common.NotificationCenter;
      if (nc) {
        try {
          if (d.action === 'file') {
            nc.trigger(d.state === 'hide' ? 'menu:hide' : 'menu:show');
          } else {
            nc.trigger('tab:set-active', d.action);
          }
        } catch (e) { /* engine decides; the driver is best-effort */ }
      }
      for (var i = 0; i < window.frames.length; i++) {
        try { window.frames[i].postMessage(d, '*'); } catch (e) { /* nothing below */ }
      }
    });
  }

  function hideEngineTabs() {
    var tries = 0;
    (function poll() {
      if (document.querySelector('#toolbar')) {
        var st = document.createElement('style');
        st.id = '__wasenoffice_compact_chrome__';
        st.textContent = '#toolbar .tabs { display: none !important; }';
        document.head.appendChild(st);
        forwardLog('info', 'compact chrome: tabs strip hidden (app frame)');
        return;
      }
      if (++tries < 40) setTimeout(poll, 300);
    })();
  }

  /** T-A41 DD K02 S1 spike: `window.editor` (sdkjs api global, word/api.js:932)
   *  reachability + engine button id presence — opens (or vetoes) S2. */
  function installKeymapProbe() {
    if (window.__WASEN_OFFICE_KM_PROBE__ !== true) return;
    var IDS = ['bold', 'italic', 'underline', 'strikeout', 'save', 'undo', 'redo'];
    var tries = 0;
    (function poll() {
      if (window.editor) {
        // api inits before toolbar render — count buttons after a settle delay.
        setTimeout(function () {
          var found = 0;
          for (var i = 0; i < IDS.length; i++) {
            if (document.getElementById('id-toolbar-btn-' + IDS[i])) found++;
          }
          var unlocked = window.Asc && window.Asc.c_oAscUnlockedShortcutActionTypes
            ? Object.keys(window.Asc.c_oAscUnlockedShortcutActionTypes).length : -1;
          // Reveal self-test (synthetic events reach OUR listeners, DD §7.3).
          var reveal = '?';
          if (document.body && window.__WASEN_OFFICE_COMPACT_CHROME__) {
            window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Meta' }));
            var on = document.body.classList.contains('keyhint');
            window.dispatchEvent(new KeyboardEvent('keyup', { key: 'Meta' }));
            var off = !document.body.classList.contains('keyhint');
            reveal = on && off ? 'ok' : 'broken';
          }
          forwardLog('info', 'km-probe: editor=object unlocked=' + unlocked +
            ' btn-ids=' + found + '/' + IDS.length + ' reveal=' + reveal);
        }, 2000);
        return;
      }
      if (++tries < 50) setTimeout(poll, 300);
      else forwardLog('info', 'km-probe: editor=undefined after 15s');
    })();
  }

  /** T-A41 DD K02 S3/S4 — keymap apply + Keytip reveal (app frame only).
   *  Flow: poll window.editor → request the list UP the frame chain → main
   *  frame answers with entries → engine rows go through the NATIVE sdkjs
   *  shortcut API (asc_applyAscShortcuts; reassignment hides the default
   *  chord) — no key interception anywhere. Shell rows never reach us.
   *  Reveal: ⌘ held → body.keyhint; CSS shows [data-wasenoffice-hint] badges
   *  stamped from the keymap. Blur clears (keyup lost to app switch). */
  var KM_MSG_TYPE = '__wasenoffice_keymap';
  var kmEntries = null;

  function installKeymapApply() {
    if (window.__WASEN_OFFICE_NO_KEYMAP__ === true) return;
    window.addEventListener('message', function (ev) {
      var d = ev.data;
      var REQ = KM_MSG_TYPE + ':req';
      if (!d || (d.type !== KM_MSG_TYPE && d.type !== REQ)) return;
      if (d.entries) {
        kmEntries = d.entries;
        // Down-relay: main → boot → app; the app frame applies.
        for (var i = 0; i < window.frames.length; i++) {
          try { window.frames[i].postMessage(d, '*'); } catch (e) { /* leaf */ }
        }
        if (window.editor) applyKeymap();
      } else if (d.type === REQ && window.parent && window.parent !== window) {
        // Up-relay the request toward the main frame (it answers).
        try { window.parent.postMessage(d, '*'); } catch (e) { /* standalone */ }
      }
    });
    var tries = 0;
    (function poll() {
      if (window.editor) {
        if (kmEntries) applyKeymap();
        else {
          try {
            window.parent.postMessage({ type: KM_MSG_TYPE + ':req' }, '*');
          } catch (e) { /* no parent — nothing to apply */ }
        }
        return;
      }
      if (++tries < 50) setTimeout(poll, 300);
    })();
  }

  function applyKeymap() {
    if (!kmEntries || !window.editor) return;
    var applied = 0;
    for (var i = 0; i < kmEntries.length; i++) {
      var e = kmEntries[i];
      if (e.scope !== 'engine' || !e.engine_type) continue;
      if (e.chord && e.chord.key) {
        window.editor.asc_applyAscShortcuts([{
          type: e.engine_type,
          keyCode: e.chord.key.toUpperCase().charCodeAt(0),
          ctrlKey: !!e.chord.ctrl,
          shiftKey: !!e.chord.shift,
          altKey: !!e.chord.alt,
          commandKey: !!e.chord.cmd,
          isHidden: false,
        }]);
        applied++;
        var dc = e.default_chord;
        if (dc && e.chord.key.toUpperCase() !== dc.key.toUpperCase()) {
          // Reassigned: retire the default chord so only the new one fires.
          window.editor.asc_applyAscShortcuts([{
            type: e.engine_type,
            keyCode: dc.key.toUpperCase().charCodeAt(0),
            ctrlKey: !!dc.ctrl,
            shiftKey: !!dc.shift,
            altKey: !!dc.alt,
            commandKey: !!dc.cmd,
            isHidden: true,
          }]);
        }
      }
    }
    stampKeymap();
    // Late renderers: some toolbar buttons land after the first stamp pass —
    // the evidence line reports AFTER the second pass (single truthful line).
    setTimeout(function () {
      stampKeymap();
      forwardLog('info', 'km-apply: engine=' + applied + ' hinted=' + stampCount +
        (stampMissing ? ' missing=' + stampMissing : ''));
    }, 3000);
  }

  var stampCount = 0;
  var stampMissing = '';
  function stampKeymap() {
    if (!kmEntries) return;
    stampCount = 0;
    var missing = [];
    for (var i = 0; i < kmEntries.length; i++) {
      var e = kmEntries[i];
      if (e.scope !== 'engine' || !e.hint) continue;
      var btn = document.getElementById('id-toolbar-btn-' + e.id);
      if (btn && !btn.hasAttribute('data-wasenoffice-hint')) {
        btn.setAttribute('data-wasenoffice-hint', e.hint);
      }
      if (btn) stampCount++;
      else missing.push(e.id);
    }
    stampMissing = missing.join(',');
  }

  function installKeytipReveal() {
    if (window.__WASEN_OFFICE_COMPACT_CHROME__ !== true) return;
    var toggle = function (on) {
      if (document.body) document.body.classList.toggle('keyhint', on);
    };
    window.addEventListener('keydown', function (ev) {
      if (ev.key === 'Meta') toggle(true);
    });
    window.addEventListener('keyup', function (ev) {
      if (ev.key === 'Meta') toggle(false);
    });
    window.addEventListener('blur', function () { toggle(false); });
    var st = document.createElement('style');
    st.id = '__wasenoffice_keytip__';
    st.textContent =
      'body.keyhint #toolbar [data-wasenoffice-hint]::after { content: attr(data-wasenoffice-hint);' +
      ' position: absolute; right: 1px; bottom: 1px; padding: 0 3px; border-radius: 3px;' +
      ' background: #2b579a; color: #fff; font-size: 9px; line-height: 12px; font-weight: 600; }' +
      '#toolbar [data-wasenoffice-hint] { position: relative; }';
    var root = document.head || document.documentElement;
    if (root) root.appendChild(st);
  }

  /** T-A41 bisect probe: synthetic click on pdfeditor Edit Text after boot —
   *  engine console errors surface via the [web] ERROR forwarding. */
  function installPdfProbe() {
    if (window.__WASEN_OFFICE_PDF_PROBE__ !== true) return;
    // Capture the engine's own error channel — the Warning dialog fires from
    // asc_onError and does not always reach console.error.
    var tries = 0;
    (function poll() {
      if (window.editor && window.editor.asc_registerCallback) {
        try {
          var errCb = function (code, msg) {
            forwardLog('error', 'pdf-probe: asc_onError code=' + code + ' msg=' + msg);
          };
          window.editor.asc_registerCallback('asc_onError', errCb);
          window.editor.asc_registerCallback('onError', errCb);
        } catch (e) { /* callback optional */ }
      }
      var btn = document.querySelector('#id-toolbar-btn-edittext, #toolbar .btn-edit-text');
      if (btn && window.editor) {
        // Unmask: wrap every bridge method + engine api method in a
        // try/catch logger — the next throw carries a real stack.
        var wrapObj = function (obj, tag) {
          if (!obj) return;
          Object.keys(obj).forEach(function (k) {
            if (typeof obj[k] !== 'function') return;
            (function (orig, name) {
              obj[name] = function () {
                try {
                  return orig.apply(this, arguments);
                } catch (e) {
                  forwardLog('error', 'wrap-throw: ' + tag + '.' + name + ' — ' +
                    ((e && e.stack) || e));
                  throw e;
                }
              };
            })(obj[k], k);
          });
        };
        wrapObj(window.AscDesktopEditor, 'bridge');
        try { wrapObj(window.editor, 'api'); } catch (e3) { /* engine api */ }
        // Real-input automation mode (AppleScript drives the window): dump
        // SCREEN coordinates of the buttons the user flow needs, repeatedly
        // (layout settles after external window resize).
        var dumpCoords = function () {
          var pick = function (sel) {
            var el = document.querySelector(sel);
            if (!el) return null;
            var r = el.getBoundingClientRect();
            return {
              x: Math.round(window.screenX + r.left + r.width / 2),
              y: Math.round(window.screenY + r.top + r.height / 2),
            };
          };
          var cv = document.getElementById('id_viewer');
          var cvr = cv ? cv.getBoundingClientRect() : null;
          var dpr = window.devicePixelRatio || 1;
          var d = {
            dpr: dpr, outerW: window.outerWidth, outerH: window.outerHeight,
            screenX: window.screenX, screenY: window.screenY,
            editpdf: pick('#id-toolbar-btn-edittext, .btn-edit-text'),
            textbox: pick('.btn-text'),
            canvas: cvr ? {
              x: Math.round(window.screenX + cvr.left + cvr.width / 2),
              y: Math.round(window.screenY + cvr.top + cvr.height / 3),
            } : null,
          };
          forwardLog('info', 'pdf-coords: ' + JSON.stringify(d));
        };
        [3000, 8000].forEach(function (ms) {
          setTimeout(dumpCoords, ms);
        });
        // Periodic re-dump: buttons appear/disappear as modes activate and
        // the driver (AppleScript) re-reads the freshest line before clicks.
        for (var s = 15; s <= 300; s += 5) setTimeout(dumpCoords, s * 1000);
        // Repro sequence (relative to editor readiness — the 26MB scan takes
        // a while): Edit PDF → Text Box api → canvas click → end-state dump.
        window.addEventListener('error', function (ev) {
          forwardLog('error', 'pdf-probe: frame error: ' + ev.message + ' stack=' +
            ((ev.error && ev.error.stack) || '').slice(0, 400));
        });
        setTimeout(function () {
          try {
            document.querySelector('#id-toolbar-btn-edittext, .btn-edit-text').click();
            forwardLog('info', 'pdf-probe: editpdf clicked');
          } catch (e4) {
            forwardLog('error', 'pdf-probe: editpdf threw: ' + (e4 && e4.stack || e4));
          }
        }, 2000);
        setTimeout(function () {
          try {
            window.editor.AddFreeTextAnnot(0);
            forwardLog('info', 'pdf-probe: AddFreeTextAnnot(0) called');
          } catch (e5) {
            forwardLog('error', 'pdf-probe: AddFreeTextAnnot threw: ' + (e5 && e5.stack || e5));
          }
        }, 6000);
        setTimeout(function () {
          var cv = document.getElementById('id_viewer');
          if (!cv) return;
          var r = cv.getBoundingClientRect();
          var opts = {
            bubbles: true, cancelable: true, clientX: r.left + r.width / 2,
            clientY: r.top + r.height / 3, button: 0,
          };
          cv.dispatchEvent(new MouseEvent('mousedown', opts));
          cv.dispatchEvent(new MouseEvent('mouseup', opts));
          cv.dispatchEvent(new MouseEvent('click', opts));
          forwardLog('info', 'pdf-probe: canvas clicked (place text)');
        }, 10000);
        setTimeout(function () {
          var locked = document.querySelectorAll('#toolbar .disabled, #toolbar [class*="locked"]').length;
          forwardLog('info', 'pdf-probe: end-state locked=' + locked);
        }, 16000);
        return; // sequence started — poll done (no re-entry, no re-clicks)
      }
      if (++tries < 200) setTimeout(poll, 300);
      else forwardLog('info', 'pdf-probe: edit-text button not found in 60s');
    })();
  }

  /**
   * Fixes for PDF Editor & Clean-Room Runtime Adaptations:
   * 1. Force asc_isOffline = true: ensures editor behaves as an offline desktop application
   *    (prevents co-authoring disconnects, enables direct local save, corrects button icons).
   * 2. Fix asc_EditPage: provides fallback pre_Paste implementation that triggers callback
   *    so editing transactions complete and re-render the page canvas.
   * 3. UI Controllers Protection:
   *    - FormsTab.prototype.onApiFocusObject: safe guards against undefined this.view.btnTextField.
   *    - InsTab.prototype.onApiFocusObject: safe guards against undefined this.view.btnInsertChart.
   * 4. Error Suicide Prevention:
   *    - Suppresses EditingError (-25) in baseEditorsApi.sendUnhandledError and Main.onError
   *      so minor JS glitches never trigger disableEditing(true) or lock the user out.
   *    - Suppresses Warning modal dialog ("error occurred during the work with the document...").
   * 5. PDF Local-Save Wiring:
   *    - Extracts modified PDF binary from editor.getDocumentRenderer().Save() and sends
   *      to AscDesktopEditor.localSaveToDrawingFormat to write directly to disk.
   */
  function installPdfFixes() {
    try {
      function uint8ToBase64(u8) {
        if (window.AscCommon && window.AscCommon.Base64 && typeof window.AscCommon.Base64.encode === 'function') {
          return window.AscCommon.Base64.encode(u8, 0, u8.length);
        }
        var CHUNK_SIZE = 0x8000;
        var index = 0;
        var length = u8.length;
        var result = '';
        while (index < length) {
          var slice = u8.subarray(index, Math.min(index + CHUNK_SIZE, length));
          result += String.fromCharCode.apply(null, slice);
          index += CHUNK_SIZE;
        }
        return btoa(result);
      }

      function patchApi(target) {
        if (!target || target.__wasenoffice_pdf_api_patched) return;
        target.__wasenoffice_pdf_api_patched = true;

        target.asc_isOffline = function () { return true; };

        var origPrePaste = target.pre_Paste;
        target.pre_Paste = function (_fonts, _images, callback) {
          if (typeof callback === 'function') {
            try { callback(); } catch (e) { forwardLog('warn', 'pre_Paste callback error: ' + e); }
          } else if (origPrePaste) {
            origPrePaste.apply(this, arguments);
          }
        };

        var origSendUnhandled = target.sendUnhandledError;
        if (origSendUnhandled) {
          target.sendUnhandledError = function (errorMsg, url, lineNumber, column, stack) {
            forwardLog('warn', 'sendUnhandledError suppressed: ' + errorMsg + ' at ' + url + ':' + lineNumber);
          };
        }
      }

      function patchEngineApi() {
        var baseApi = window.baseEditorsApi || (window.Asc && window.Asc.baseEditorsApi);
        if (baseApi && baseApi.prototype) {
          patchApi(baseApi.prototype);
        }
        if (window.PDFEditorApi && window.PDFEditorApi.prototype) {
          patchApi(window.PDFEditorApi.prototype);
        }
        if (window.editor) {
          patchApi(window.editor);

          // PDF Save wiring
          if (!window.editor.__wasenoffice_pdf_save_wired && typeof window.editor.getDocumentRenderer === 'function') {
            window.editor.__wasenoffice_pdf_save_wired = true;
            var origAscSave = window.editor.asc_Save;
            window.editor.asc_Save = function () {
              try {
                var renderer = window.editor.getDocumentRenderer();
                if (renderer && typeof renderer.Save === 'function') {
                  var bytes = renderer.Save();
                  if (bytes && bytes.length > 0) {
                    var b64 = uint8ToBase64(bytes);
                    var docTitle = window.editor.documentTitle || 'document.pdf';
                    window.AscDesktopEditor.localSaveToDrawingFormat(
                      docTitle,
                      window.editor.DocumentUrl || '',
                      '',
                      b64,
                      (window.Asc && window.Asc.c_oAscFileType) ? window.Asc.c_oAscFileType.PDF : 513
                    );
                    forwardLog('info', 'PDF saved via localSaveToDrawingFormat: ' + bytes.length + ' bytes');
                    return true;
                  }
                }
              } catch (saveErr) {
                forwardLog('error', 'PDF custom save error: ' + (saveErr && saveErr.stack || saveErr));
              }
              if (origAscSave) return origAscSave.apply(this, arguments);
            };
          }
        }
      }

      function patchUI() {
        // 1. Safe-guard FormsTab.onApiFocusObject
        if (window.PDFE && window.PDFE.Controllers && window.PDFE.Controllers.FormsTab) {
          var formsTab = window.PDFE.Controllers.FormsTab;
          if (formsTab.prototype && !formsTab.prototype.__wasenoffice_guarded) {
            formsTab.prototype.__wasenoffice_guarded = true;
            var origFormsFocus = formsTab.prototype.onApiFocusObject;
            formsTab.prototype.onApiFocusObject = function (selectedObjects) {
              if (!this.view || !this.view.btnTextField) return;
              try {
                return origFormsFocus.apply(this, arguments);
              } catch (e) {
                forwardLog('warn', 'FormsTab.onApiFocusObject error caught: ' + e);
              }
            };
          }
        }

        // 2. Safe-guard InsTab.onApiFocusObject
        if (window.PDFE && window.PDFE.Controllers && window.PDFE.Controllers.InsTab) {
          var insTab = window.PDFE.Controllers.InsTab;
          if (insTab.prototype && !insTab.prototype.__wasenoffice_guarded) {
            insTab.prototype.__wasenoffice_guarded = true;
            var origInsFocus = insTab.prototype.onApiFocusObject;
            insTab.prototype.onApiFocusObject = function (selectedObjects) {
              if (!this.view) return;
              try {
                return origInsFocus.apply(this, arguments);
              } catch (e) {
                forwardLog('warn', 'InsTab.onApiFocusObject error caught: ' + e);
              }
            };
          }
        }

        // 3. Safe-guard Main.onError: Suppress EditingError (-25) to avoid document lock
        if (window.PDFE && window.PDFE.Controllers && window.PDFE.Controllers.Main) {
          var mainCtrl = window.PDFE.Controllers.Main;
          if (mainCtrl.prototype && !mainCtrl.prototype.__wasenoffice_guarded) {
            mainCtrl.prototype.__wasenoffice_guarded = true;
            var origOnError = mainCtrl.prototype.onError;
            mainCtrl.prototype.onError = function (id, errData) {
              if (window.Asc && window.Asc.c_oAscError && id === window.Asc.c_oAscError.ID.EditingError) {
                forwardLog('warn', 'Main.onError EditingError (-25) suppressed, document stays editable');
                return;
              }
              return origOnError.apply(this, arguments);
            };
          }
        }

        // 4. Safe-guard Common.UI.alert: prevent warning dialog from locking UI
        if (window.Common && window.Common.UI && window.Common.UI.alert && !window.Common.UI.alert.__wasenoffice_guarded) {
          var origAlert = window.Common.UI.alert;
          window.Common.UI.alert = function (config) {
            if (config && config.msg && typeof config.msg === 'string') {
              if (
                config.msg.indexOf('Download as') !== -1 ||
                config.msg.indexOf('Save as') !== -1 ||
                config.msg.indexOf('error occurred during the work with the document') !== -1
              ) {
                forwardLog('warn', 'Suppressed warning dialog: ' + config.msg);
                return { $window: { attr: function () {} } };
              }
            }
            return origAlert.apply(this, arguments);
          };
          window.Common.UI.alert.__wasenoffice_guarded = true;
        }
      }

      patchEngineApi();
      patchUI();
      var pollCount = 0;
      var timer = setInterval(function () {
        patchEngineApi();
        patchUI();
        if (++pollCount > 100) clearInterval(timer);
      }, 200);
    } catch (e) {
      forwardLog('warn', 'installPdfFixes error: ' + e);
    }
  }

  function relay(method, args) {
    // engine/boot frame → parent chain → main frame → Rust (best effort).
    // FULL args — truncation here would corrupt LocalFileSaveChanges deltas.
    try {
      if (!isMainFrame() && window.parent && window.parent !== window) {
        window.parent.postMessage({ type: RELAY_TYPE, method: method, args: args }, '*');
      }
    } catch (e) { /* relay is diagnostic only */ }
  }

  install();
})(window);
