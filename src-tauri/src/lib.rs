//! NanoOffice — Rust shell around the ONLYOFFICE editing engine.
//!
//! Modules:
//! - `bridge`   : shim of `window.AscDesktopEditor` (~130 methods, see docs/reference.map.md)
//! - `protocol` : `nanooffice-file://` custom protocol serving files to the engine
//! - `sidecar`  : x2t converter process management
//! - `window`   : window / tab state management

mod bridge;
mod protocol;
mod sidecar;
mod window;

use bridge::BridgeCmd;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! Welcome to NanoOffice.", name)
}

/// Bridge entry: JS shim calls this for every `window.AscDesktopEditor.*` method.
/// Methods not yet implemented return `Err` with an `UNIMPL` marker.
#[tauri::command]
fn asc_desktop_editor(method: String, payload: serde_json::Value) -> Result<serde_json::Value, String> {
    match BridgeCmd::dispatch(&method, payload) {
        Ok(v) => Ok(v),
        Err(bridge::BridgeError::Unsupported(m)) => {
            log::warn!("bridge UNIMPL: {}", m);
            Err(format!("UNIMPL:{}", m))
        }
        Err(bridge::BridgeError::Runtime(e)) => Err(format!("ERR:{}", e)),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, asc_desktop_editor])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
