//! NanoOffice — Rust shell around the ONLYOFFICE editing engine.
//!
//! Modules:
//! - `bridge`   : shim of `window.AscDesktopEditor` (~130 methods, see docs/reference.map.md)
//! - `protocol` : `nanooffice-engine://` assets + `nanooffice-file://` user documents
//! - `sidecar`  : x2t converter seam — binary resolution + `convert()` (T-A14)
//! - `formats`  : format registry + per-document workdir layout (T-052)
//! - `viewer`   : quick-view command layer (V01 — parse-only, no engine)
//! - `editor`   : Nano Light editor command layer (L01 — sessions, mutations,
//!   save straight through the A2 crate; no Editor.bin, no x2t on that path)
//!
//! Shim JS injected into every frame (in this order, see `run`): probe flag →
//! `BRIDGE_JS` (desktopEditor.js — bridge + relay) → `DELIVERY_JS`
//! (delivery.js — open-delivery chain); both also bundled by vite via src/main.ts.

mod bridge;
mod config;
mod editor;
mod formats;
mod protocol;
mod sidecar;
pub(crate) mod storage;
mod viewer;
mod window;

use config::AppConfig;
use serde::Serialize;
use std::sync::Arc;
use tauri::{Manager, State};

use protocol::FileRegistry;

/// Bridge shim JS — installed in ALL frames (the engine runs inside a child
/// iframe created by DocsAPI.DocEditor). Same file is bundled with the start
/// page by vite; keep it framework-free.
const BRIDGE_JS: &str = include_str!("../../src/shim/desktopEditor.js");

/// Open-delivery chain JS (T-045) — MUST be injected AFTER BRIDGE_JS in every
/// frame (it consumes `window.__nanooffice_shim`, published by the bridge) and
/// imported after it in src/main.ts. Same vite double-duty as BRIDGE_JS.
const DELIVERY_JS: &str = include_str!("../../src/shim/delivery.js");

/// Open a local file for editing: registers it in the FileRegistry and returns
/// the URL the engine should load — loopback HTTP with an opaque fileId
/// (the engine rejects custom-scheme document URLs; the disk path never
/// reaches JS). See protocol::filesrv for the rationale. T-059: the entry is
/// owned by the calling window's label; every later engine call for this
/// document (delivery/save) must carry the returned `fileId` explicitly.
#[tauri::command]
fn open_document(
    window: tauri::WebviewWindow,
    path: String,
    registry: State<'_, Arc<FileRegistry>>,
) -> Result<OpenedDocument, String> {
    log::info!("open_document: {path}");
    let id = registry.register(window.label(), std::path::Path::new(&path))?;
    let disk = registry_path(&registry, id);
    let name = disk
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "document".into());
    Ok(OpenedDocument {
        url: format!(
            "http://127.0.0.1:{}/file/{id}/{}",
            protocol::filesrv::bound_port(),
            name
        ),
        name,
        file_id: id,
    })
}

fn registry_path(registry: &State<Arc<FileRegistry>>, id: u32) -> std::path::PathBuf {
    registry
        .inner()
        .get(id)
        .unwrap_or_else(|| std::path::PathBuf::from("unknown"))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenedDocument {
    url: String,
    name: String,
    /// The registry key of this document (T-059): the JS side stamps it on
    /// every later engine call (delivery/save) so Rust resolves exactly this
    /// entry — never an implicit "latest".
    file_id: u32,
}

/// Startup document (env `NANO_OFFICE_OPEN`), consumed once by the start page.
#[tauri::command]
fn get_startup_document(
    window: tauri::WebviewWindow,
    registry: State<'_, Arc<FileRegistry>>,
    config: State<'_, Arc<AppConfig>>,
) -> Result<Option<OpenedDocument>, String> {
    match config.take_startup_document() {
        Some(path) => open_document(window, path, registry).map(Some),
        None => Ok(None),
    }
}

/// Bridge entry: JS shim calls this for every `window.AscDesktopEditor.*` method.
/// Methods not yet implemented return `Err` with an `UNIMPL` marker.
#[tauri::command]
fn asc_desktop_editor(
    app: tauri::AppHandle,
    method: String,
    payload: serde_json::Value,
    registry: State<'_, Arc<FileRegistry>>,
) -> Result<serde_json::Value, String> {
    log::info!("bridge → {method}({})", payload_to_short(&payload));
    match bridge::BridgeCmd::dispatch(&app, registry.inner(), &method, payload) {
        Ok(v) => Ok(v),
        Err(bridge::BridgeError::Unsupported(m)) => {
            log::warn!("bridge UNIMPL: {}", m);
            Err(format!("UNIMPL:{}", m))
        }
        Err(bridge::BridgeError::Runtime(e)) => Err(format!("ERR:{}", e)),
    }
}

/// Compact single-line payload summary for logs (never full dumps).
fn payload_to_short(payload: &serde_json::Value) -> String {
    let s = payload.to_string();
    if s.len() > 120 {
        format!("{}… ({} bytes)", &s[..120], s.len())
    } else {
        s
    }
}

/// Deliver a REGISTERED document to the engine frame via the
/// desktop open contract (LocalStartOpen → convert →
/// DesktopOfflineAppDocumentEndLoad). T-059: the caller states WHICH document
/// (`fileId`, stamped by the shim from `open_document`) — resolving by
/// "most recent registration" raced quick-view windows into wrong opens.
/// The engine's offline loader consumes the
/// x2t-converted `Editor.bin` for OOXML sources; PDF is a cross-platform
/// format — the real shell does NOT convert it, it copies the ORIGINAL file
/// into the recovery folder and delivers those bytes (observed: recovery dir
/// contains `<name>.pdf`, no Editor.bin; client_renderer_wrapper.cpp
/// "onlocaldocument_loadend" falls back to `folder + "/" + file_name(src)`
/// when Editor.bin is missing — the pdf editor's own loader parses raw PDF).
/// Format facts live in the registry (`formats::FORMATS` — the Rust half),
/// mirrored by `src/main.ts` FILE_FORMATS for the boot page (cross-checked
/// by test; xps/djvu are CROSSPLATFORM too — same copy-original rule when
/// they join the table).
/// Conversions run through the sidecar seam (`sidecar::convert`, T-A14).
#[tauri::command]
fn get_document_for_delivery(
    registry: State<'_, Arc<FileRegistry>>,
    file_id: Option<u32>,
) -> Result<DeliverableDocument, String> {
    use base64::Engine as _;
    let id = file_id.ok_or_else(|| {
        "delivery: no fileId (the shim must stamp the open_document id)".to_string()
    })?;
    let disk = registry
        .inner()
        .get(id)
        .ok_or_else(|| format!("no document registered for fileId {id}"))?;

    let work = formats::DocWorkdir::new(id);
    let _ = std::fs::remove_dir_all(work.root());
    std::fs::create_dir_all(work.root())
        .map_err(|e| format!("mkdir {}: {e}", work.root().display()))?;

    let ext = disk
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    // Copy-original flavor is per-format (pdf today). Unknown extensions
    // keep the convert branch — the TS boot gate (FILE_FORMATS) rejects them
    // long before delivery ever runs, so this is unreachable in practice.
    let flavor = formats::by_ext(&ext)
        .map(|f| f.open)
        .unwrap_or(formats::OpenFlavor::Convert);
    let name = work.delivered_name(flavor, &disk)?;

    let delivered_path = match flavor {
        formats::OpenFlavor::CopyOriginal => {
            let target = work.root().join(&name);
            sidecar::convert(&sidecar::ConvertRequest::Passthrough {
                src: &disk,
                dst: &target,
            })
            .map_err(|e| e.to_string())?;
            target
        }
        formats::OpenFlavor::Convert => {
            let out = work.editor_bin();
            let conv = sidecar::convert(&sidecar::ConvertRequest::OpenBinary {
                src: &disk,
                dst: &out,
            })
            .map_err(|e| e.to_string())?;
            log::info!(
                "x2t converted {} bytes → Editor.bin ({} bytes) in {:?}",
                disk.display(),
                std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0),
                conv.elapsed
            );
            out
        }
    };

    let bytes = std::fs::read(&delivered_path)
        .map_err(|e| format!("read {}: {e}", delivered_path.display()))?;
    log::info!("delivering {} ({} bytes) to engine", name, bytes.len());

    Ok(DeliverableDocument {
        folder: work.root().to_string_lossy().into_owned(),
        name,
        // the engine's save-end contract derives the document name from the
        // source path — it must be the ORIGINAL file, not the workdir artifact
        src: disk.to_string_lossy().into_owned(),
        b64: base64::engine::general_purpose::STANDARD.encode(&bytes),
    })
}

#[derive(Serialize)]
pub struct DeliverableDocument {
    folder: String,
    name: String,
    src: String,
    b64: String,
}

/// Loopback origin serving the engine + documents (iframe src base).
#[tauri::command]
fn get_engine_base() -> String {
    format!("http://127.0.0.1:{}", protocol::filesrv::bound_port())
}

/// Frontend diagnostics: DOM-side errors/logs forwarded to stdout (PH-0
/// observability — the engine console is only visible in devtools otherwise).
#[tauri::command]
fn frontend_log(level: String, message: String) {
    match level.as_str() {
        "error" => log::error!("[web] {message}"),
        "warn" => log::warn!("[web] {message}"),
        _ => log::info!("[web] {message}"),
    }
}
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // `log` has no backend by default; PH-0 evidence (bridge UNIMPL hits,
    // protocol traffic) flows to stdout via env_logger.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let config = Arc::new(AppConfig::from_env());

    let mut app = tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(Arc::clone(&config))
        .manage(Arc::new(FileRegistry::default()))
        .manage(Arc::new(window::SurfaceHandoff::default()))
        .manage(Arc::new(editor::EditorSessions::default()))
        .register_uri_scheme_protocol(protocol::ENGINE_SCHEME, protocol::handle_engine)
        .register_uri_scheme_protocol(protocol::FILE_SCHEME, protocol::handle_file)
        // T-059 cleanup: the registry used to only ever grow. A destroyed
        // window drops the entries it registered (quick-view / Nano Light
        // windows come and go in one app run; the main window's drop
        // coincides with exit). L01: editor sessions drop on the same hook.
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                let registry = window.app_handle().state::<Arc<FileRegistry>>();
                let removed = registry.unregister_owner(window.label());
                if removed > 0 {
                    log::info!(
                        "registry: window {} closed — unregistered {removed} file(s)",
                        window.label()
                    );
                }
                let sessions = window.app_handle().state::<Arc<editor::EditorSessions>>();
                let dropped = sessions.remove_owner(window.label());
                if dropped > 0 {
                    log::info!(
                        "nano light: window {} closed — dropped session",
                        window.label()
                    );
                }
            }
        })
        .on_page_load(|_webview, payload| {
            log::info!("page_load [{:?}] {}", payload.event(), payload.url());
        })
        .invoke_handler(tauri::generate_handler![
            asc_desktop_editor,
            open_document,
            get_startup_document,
            get_engine_base,
            get_document_for_delivery,
            frontend_log,
            viewer::viewer_open,
            viewer::viewer_view,
            editor::nano_light_view,
            editor::nano_light_reopen,
            editor::nano_light_open,
            editor::nano_light_edit,
            editor::nano_light_save,
            editor::nano_light_close_tab
        ])
        .setup(|app| {
            // Engine-facing document URLs must be loopback HTTP (see filesrv docs).
            let registry = Arc::clone(app.state::<Arc<FileRegistry>>().inner());
            if let Err(e) = protocol::filesrv::spawn(registry) {
                log::error!("{e} — document open will not work");
            }

            // Window factory seam (T-046): creates initial boot window matching config
            let config = Arc::clone(app.state::<Arc<AppConfig>>().inner());
            window::create_boot_window(app.handle(), &config, BRIDGE_JS, DELIVERY_JS)?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    // Rule 6b (AGENTS.md): agent/dev boots must not steal the user's screen.
    // MUST run BEFORE app.run(): tauri's setup hook fires after macOS
    // applicationDidFinishLaunching already activated the app (verified 04.10).
    #[cfg(target_os = "macos")]
    if config.background {
        app.set_activation_policy(tauri::ActivationPolicy::Accessory);
        log::info!("background: accessory activation policy (NANO_OFFICE_BACKGROUND)");
    }

    app.run(|_app_handle, _event| {});
}
