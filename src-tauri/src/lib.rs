//! NanoOffice — Rust shell around the ONLYOFFICE editing engine.
//!
//! Modules:
//! - `bridge`   : shim of `window.AscDesktopEditor` (~130 methods, see docs/reference.map.md)
//! - `protocol` : `nanooffice-engine://` assets + `nanooffice-file://` user documents
//! - `sidecar`  : x2t converter process management (PH-1)
//! - `window`   : window / tab state management (PH-2)
//!
//! Shim JS injected into every frame (in this order, see `run`): probe flag →
//! `BRIDGE_JS` (desktopEditor.js — bridge + relay) → `DELIVERY_JS`
//! (delivery.js — open-delivery chain); both also bundled by vite via src/main.ts.

mod bridge;
mod protocol;
mod sidecar;
mod window;

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
/// desktop open contract (LocalStartOpen → x2t convert →
/// DesktopOfflineAppDocumentEndLoad). The engine's offline loader consumes the
/// x2t-converted `Editor.bin`, not the source OOXML (observed: sdk-all `brb`
/// has no plain-docx branch; the real shell always converts first).
/// PH-0: synchronous one-shot x2t call; PH-1 replaces this with the sidecar.
#[tauri::command]
fn get_document_for_delivery(
    registry: State<'_, Arc<FileRegistry>>,
) -> Result<DeliverableDocument, String> {
    use base64::Engine as _;
    let (id, disk) = registry
        .inner()
        .latest()
        .ok_or_else(|| "no document registered yet".to_string())?;

    let work = bridge::save::workdir_for(id);
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).map_err(|e| format!("mkdir {}: {e}", work.display()))?;
    let out = work.join("Editor.bin");

    let x2t = bridge::save::x2t_path();
    let started = std::time::Instant::now();
    let status = std::process::Command::new(&x2t)
        .arg(&disk)
        .arg(&out)
        .output()
        .map_err(|e| format!("spawn x2t: {e}"))?;
    if !out.is_file() {
        return Err(format!(
            "x2t produced no Editor.bin (exit {:?}, stderr: {})",
            status.status.code(),
            String::from_utf8_lossy(&status.stderr)
                .chars()
                .take(200)
                .collect::<String>()
        ));
    }
    let bytes = std::fs::read(&out).map_err(|e| format!("read Editor.bin: {e}"))?;
    log::info!(
        "x2t converted {} bytes → Editor.bin ({} bytes) in {:?}",
        disk.display(),
        bytes.len(),
        started.elapsed()
    );

    Ok(DeliverableDocument {
        folder: work.to_string_lossy().into_owned(),
        name: "Editor.bin".into(),
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
        .plugin(tauri_plugin_opener::init())
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
            frontend_log
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
