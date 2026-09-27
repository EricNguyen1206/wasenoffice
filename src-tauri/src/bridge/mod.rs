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
    /// Implemented call failed at runtime. (First real runtime errors arrive
    /// with PH-1 file lifecycle handlers.)
    #[allow(dead_code)]
    Runtime(String),
}

pub struct BridgeCmd;

impl BridgeCmd {
    /// Dispatch a bridge method by its exact upstream name.
    ///
    /// PH-0 first wave (open/render flow, evidence in docs/reference.map.md):
    /// - `CreateEditorApi` — engine hands over its api object at init
    ///   (upstream/sdkjs/common/apiBase.js:266); we only acknowledge.
    /// - `IsLocalFile` — sync boolean gates in apiBase.js (e.g. :2394, :3222);
    ///   PH-0 is local-only, so always `true`.
    /// - `execCommand` — status channel from web-apps (desktopinit.js sends
    ///   `webapps:entry` on boot); acknowledge and log the command.
    /// - `GetEncryptedHeader` — called synchronously at sdk-all load
    ///   (upstream/sdkjs/common/editorscommon.js:12380); "ENCRYPTED;" is the
    ///   engine's own fallback value when the method is absent.
    /// - Local-open delivery acks (`LocalFileSetSaved`, `LocalFileRecoverFolder`,
    ///   `LocalFileSetSourcePath`, `LocalFileSetOpenChangesCount`) — the CEF shell
    ///   fires them around `DesktopOfflineAppDocumentEndLoad`; the bytes arrive
    ///   through that separate engine-global (see protocol delivery chain).
    /// - `GetInstallPlugins` — parsed as JSON by the engine; "[]" = no plugins.
    pub fn dispatch(method: &str, payload: Value) -> Result<Value, BridgeError> {
        match method {
            "CreateEditorApi" => Ok(Value::Null),
            "IsLocalFile" => Ok(Value::Bool(true)),
            "getEngineVersion" => Ok(Value::String(env!("CARGO_PKG_VERSION").into())),
            "GetEncryptedHeader" => Ok(Value::String("ENCRYPTED;".into())),
            "CheckUserId" => Ok(Value::String("nanooffice-ph0".into())),
            "GetInstallPlugins" => Ok(Value::String(
                r#"[{"url":"","pluginsData":[]},{"url":"","pluginsData":[]}]"#.into(),
            )),
            "isSupportPlugins" | "isSupportMacroses" => Ok(Value::Bool(true)),
            "IsSignaturesSupport" => Ok(Value::Bool(false)),
            "SetDocumentName" => {
                if let Some(name) = payload
                    .get("args")
                    .and_then(|a| a.get(0))
                    .and_then(Value::as_str)
                {
                    log::info!("document name: {name}");
                }
                Ok(Value::Null)
            }
            "LocalFileSetSaved"
            | "LocalFileRecoverFolder"
            | "LocalFileSetSourcePath"
            | "LocalFileGetSourcePath"
            | "LocalFileSetOpenChangesCount" => Ok(Value::Null),
            // sync-truth mirror: fresh local open = saved (shim answers true).
            "LocalFileGetSaved" => Ok(Value::Bool(true)),
            // sync-truth mirror: PH-0 documents open clean (shim answers 0).
            "LocalFileGetOpenChangesCount" => Ok(serde_json::json!(0)),
            // modified-flag sink: the real shell tracks dirty state for the
            // title/save UI (PH-1); acknowledging keeps the engine flowing.
            "onDocumentModifiedChanged" => Ok(Value::Null),
            // Recents surface: the editor UI calls these unconditionally after
            // open (fire-and-forget); the shell would push lists back via the
            // onupdaterecents/onupdaterecovers globals (PH-2).
            "LocalFileRecents"
            | "LocalFileRecovers"
            | "LocalFileOpenRecent"
            | "LocalFileOpenRecover" => Ok(Value::Null),
            // Acknowledged — the actual delivery chain (x2t convert →
            // DesktopOfflineAppDocumentEndLoad) is driven from lib.rs/shim.
            "LocalStartOpen" => Ok(Value::Null),
            "execCommand" => {
                if let Some(cmd) = payload.get("cmd").and_then(Value::as_str) {
                    log::info!("bridge execCommand: {cmd}");
                }
                Ok(Value::Null)
            }
            _ => Err(BridgeError::Unsupported(method.to_string())),
        }
    }
}
