//! NanoOffice — Rust shell around the ONLYOFFICE editing engine.
//!
//! Modules:
//! - `bridge`   : shim of `window.AscDesktopEditor` (~130 methods, see docs/reference.map.md)
//! - `protocol` : `nanooffice-engine://` assets + `nanooffice-file://` user documents
//! - `sidecar`  : x2t converter seam — binary resolution + `convert()` (T-A14)
//! - `formats`  : format registry + per-document workdir layout (T-052)
//!
//! Shim JS injected into every frame (in this order, see `run`): probe flag →
//! `BRIDGE_JS` (desktopEditor.js — bridge + relay) → `DELIVERY_JS`
//! (delivery.js — open-delivery chain); both also bundled by vite via src/main.ts.

mod bridge;
mod formats;
mod protocol;
mod sidecar;
mod viewer;

use serde::Serialize;
use std::sync::Arc;
use tauri::{Manager, State, WebviewUrl, WebviewWindowBuilder};

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
/// reaches JS). See protocol::filesrv for the rationale.
#[tauri::command]
fn open_document(
    path: String,
    registry: State<'_, Arc<FileRegistry>>,
) -> Result<OpenedDocument, String> {
    log::info!("open_document: {path}");
    let id = registry.register(std::path::Path::new(&path))?;
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
    })
}

fn registry_path(registry: &State<Arc<FileRegistry>>, id: u32) -> std::path::PathBuf {
    registry
        .inner()
        .get(id)
        .unwrap_or_else(|| std::path::PathBuf::from("unknown"))
}

#[derive(Serialize)]
pub struct OpenedDocument {
    url: String,
    name: String,
}

/// Startup document (env `NANO_OFFICE_OPEN`), consumed once by the start page.
#[tauri::command]
fn get_startup_document(
    registry: State<'_, Arc<FileRegistry>>,
) -> Result<Option<OpenedDocument>, String> {
    match std::env::var("NANO_OFFICE_OPEN") {
        Ok(path) if !path.is_empty() => {
            std::env::remove_var("NANO_OFFICE_OPEN");
            open_document(path, registry).map(Some)
        }
        _ => Ok(None),
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

/// Deliver the most recently registered document to the engine frame via the
/// desktop open contract (LocalStartOpen → convert →
/// DesktopOfflineAppDocumentEndLoad). The engine's offline loader consumes the
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
) -> Result<DeliverableDocument, String> {
    use base64::Engine as _;
    let (id, disk) = registry
        .inner()
        .latest()
        .ok_or_else(|| "no document registered yet".to_string())?;

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

    tauri::Builder::default()
        .manage(Arc::new(FileRegistry::default()))
        .register_uri_scheme_protocol(protocol::ENGINE_SCHEME, protocol::handle_engine)
        .register_uri_scheme_protocol(protocol::FILE_SCHEME, protocol::handle_file)
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
            viewer::viewer_open
        ])
        .setup(|app| {
            // Engine-facing document URLs must be loopback HTTP (see filesrv docs).
            let registry = Arc::clone(app.state::<Arc<FileRegistry>>().inner());
            if let Err(e) = protocol::filesrv::spawn(registry) {
                log::error!("{e} — document open will not work");
            }
            // Probe gate for the shim's diagnostics probes (src/shim/delivery.js
            // `probesEnabled()`): default ON in debug builds (dev evidence
            // depends on them), OFF in release; NANO_OFFICE_PROBE=0/1 forces
            // OFF/ON. Published as a one-line init script so every frame can
            // read the flag synchronously.
            let probe_enabled = match std::env::var("NANO_OFFICE_PROBE").as_deref() {
                Ok("0") => false,
                Ok("1") => true,
                _ => cfg!(debug_assertions),
            };
            let probe_flag_js = format!("window.__NANO_OFFICE_PROBE__ = {probe_enabled};");
            // Save-proof automation (T-010, probe-gated in the shim): env holds
            // the text the shim inserts via asc_AddText + asc_Save() after
            // contentReady — no keystrokes (diagnostics 27.09 T-003).
            let save_probe_js = match std::env::var("NANO_OFFICE_SAVE_PROBE") {
                Ok(text) if !text.is_empty() => {
                    format!(
                        "window.__NANO_OFFICE_SAVE_PROBE_TEXT__ = {};",
                        serde_json::json!(text)
                    )
                }
                _ => String::new(),
            };
            // V01 quick-view slice (DD docs/design/viewer-a3/DD.md §3.1): env
            // trigger opens the dedicated viewer entry — no shim, no engine
            // iframe. The normal boot path below is untouched.
            if std::env::var("NANO_OFFICE_QUICK_VIEW").is_ok_and(|v| !v.is_empty()) {
                let win =
                    WebviewWindowBuilder::new(app, "main", WebviewUrl::App("viewer.html".into()))
                        .title("NanoOffice — Quick View")
                        .inner_size(1000.0, 700.0)
                        .build()?;
                let _ = win;
                return Ok(());
            }
            let win = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
                .title("NanoOffice")
                .inner_size(1280.0, 800.0)
                .min_inner_size(940.0, 560.0)
                .initialization_script_for_all_frames(&probe_flag_js);
            let win = if save_probe_js.is_empty() {
                win.initialization_script_for_all_frames(BRIDGE_JS)
            } else {
                win.initialization_script_for_all_frames(&save_probe_js)
                    .initialization_script_for_all_frames(BRIDGE_JS)
            };
            let win = win
                .initialization_script_for_all_frames(DELIVERY_JS)
                .build()?;
            // Debug builds only: engine console is the primary PH-0 evidence source.
            // Disable with NANO_OFFICE_DEVTOOLS=0 — the docked panel squeezes the
            // webview viewport and skews layout/render verification.
            #[cfg(debug_assertions)]
            if std::env::var("NANO_OFFICE_DEVTOOLS").as_deref() != Ok("0") {
                win.open_devtools();
            }
            let _ = win;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
