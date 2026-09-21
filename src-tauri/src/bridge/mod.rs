//! Bridge shim — Rust side of `window.AscDesktopEditor`.
//!
//! Contract (see harness/skills/bridge-shim/SKILL.md):
//! - Method names are kept EXACTLY as upstream (case-sensitive).
//! - Not-yet-implemented methods return `BridgeError::Unsupported` and the JS shim
//!   surfaces an `UNIMPL:<method>` error — engine call-sites are logged, never silently dropped.
//! - Mapping state per method lives in docs/reference.map.md (STUB | PARTIAL | DONE).

use serde_json::Value;

#[derive(Debug)]
pub enum BridgeError {
    /// Method known but not implemented yet.
    Unsupported(String),
    /// Implemented call failed at runtime.
    Runtime(String),
}

pub struct BridgeCmd;

impl BridgeCmd {
    /// Dispatch a bridge method by its exact upstream name.
    pub fn dispatch(method: &str, _payload: Value) -> Result<Value, BridgeError> {
        // TODO(ph0): implement first wave of methods the editor needs at open-time:
        //   IsLocalFile, CreateEditorApi (registration ack), LoadFontBase, GetImageBase,
        //   OpenFilenameDialog, LocalFileGetImageUrl, LocalFileSave...
        // Every other known method must fall through as Unsupported (logged as UNIMPL).
        Err(BridgeError::Unsupported(method.to_string()))
    }
}
