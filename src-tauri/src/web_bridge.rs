//! Web dev mode (T-068, DD `docs/design/web-dev-mode/DD.md`): the Vite
//! frontend running in a plain browser reaches the command layer over the
//! loopback file server — `POST /invoke/<cmd>` carrying the same JSON args
//! Tauri's `invoke` would. Dev-only: `WASEN_OFFICE_WEB_BRIDGE=1` (default OFF,
//! the route 404s otherwise), origins limited to the Vite dev server.
//!
//! The one idea: a browser tab stands in for a Tauri window. Commands that
//! read `window.label()` take the label the tab sends (`X-Wasen-Window`),
//! commands that would open a native window stash as usual and return
//! `{"webOpen": {"url"}}` so the tab opens the surface page itself — the
//! label travels in that URL. Everything else calls the very same handlers.
//! Ceiling: the engine editor (sdkjs) stays Tauri-only — it needs the
//! Rust→JS lane the bridge does not have (DD W01 §5).

use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use tauri::Manager;

use crate::editor::EditorSessions;
use crate::protocol::FileRegistry;
use crate::recents::RecentsStore;
use crate::window::SurfaceHandoff;

/// Origins allowed to call the bridge — the Vite dev server, nothing else.
pub const ALLOWED_ORIGINS: [&str; 2] = ["http://localhost:1420", "http://127.0.0.1:1420"];

/// Refusal for the engine surfaces (open/create route to the sdkjs iframe,
/// whose save flow needs Rust→JS — not in web mode).
pub const ENGINE_TAURI_ONLY: &str =
    "web mode: engine editor is Tauri-only — use Quick view / Nano Light";

pub fn origin_allowed(origin: Option<&str>) -> bool {
    origin.is_some_and(|o| ALLOWED_ORIGINS.contains(&o))
}

/// One invoke argument by its (camelCase) key — absent reads as JSON null, so
/// `Option<T>` params deserialize to `None` exactly like Tauri's IPC.
fn arg<T: DeserializeOwned>(args: &Value, key: &str) -> Result<T, String> {
    serde_json::from_value(args.get(key).cloned().unwrap_or(Value::Null))
        .map_err(|e| format!("web bridge: bad arg `{key}`: {e}"))
}

fn ok<T: serde::Serialize>(v: T) -> Result<Value, String> {
    serde_json::to_value(v).map_err(|e| format!("web bridge: serialize: {e}"))
}

fn web_open(url: String) -> Result<Value, String> {
    Ok(json!({ "webOpen": { "url": url } }))
}

fn state<T: Send + Sync + 'static>(app: &tauri::AppHandle) -> Result<tauri::State<'_, T>, String> {
    app.try_state::<T>()
        .ok_or_else(|| format!("web bridge: {} not managed", std::any::type_name::<T>()))
}

/// Route one invoke. `label` is the caller tab's window label (see module doc).
pub fn dispatch(
    app: &tauri::AppHandle,
    label: &str,
    cmd: &str,
    args: Value,
) -> Result<Value, String> {
    log::debug!("web bridge ← {label} {cmd}");
    let registry = state::<Arc<FileRegistry>>(app)?;
    let handoff = state::<Arc<SurfaceHandoff>>(app)?;
    let sessions = state::<Arc<EditorSessions>>(app)?;
    let recents = state::<Arc<RecentsStore>>(app)?;
    match cmd {
        // ── window-free commands: the real handlers, state via the app handle
        "frontend_log" => {
            crate::frontend_log(arg(&args, "level")?, arg(&args, "message")?);
            ok(())
        }
        "get_engine_base" => ok(crate::get_engine_base()),
        "recents_list" => ok(crate::recents::recents_list(app.state())),
        "recents_remove" => ok(crate::recents::recents_remove(
            app.state(),
            arg(&args, "path")?,
        )),
        "keymap_load" => ok(crate::keymap::keymap_load(app.clone())),
        "keymap_save" => ok(crate::keymap::keymap_save(
            app.clone(),
            arg(&args, "overrides")?,
        )?),
        "get_document_for_delivery" => ok(crate::get_document_for_delivery(
            app.state(),
            arg(&args, "fileId")?,
        )?),
        "asc_desktop_editor" => crate::asc_desktop_editor(
            app.clone(),
            arg(&args, "method")?,
            arg(&args, "payload")?,
            app.state(),
        ),
        "touchbar_update" => ok(crate::touchbar::touchbar_update(arg(&args, "payload")?)?),
        "mcp_get_status" => ok(crate::mcp::mcp_get_status()?),
        "mcp_get_config_snippet" => ok(crate::mcp::mcp_get_config_snippet()?),
        "mcp_toggle_client" => ok(crate::mcp::mcp_toggle_client(
            arg(&args, "clientId")?,
            arg(&args, "enabled")?,
        )?),
        // ── window-keyed commands: the tab's label stands in for window.label()
        "viewer_open" => ok(crate::viewer::viewer_open_impl(&registry, &handoff, label)?),
        "wasen_light_open" => ok(crate::editor::wasen_light_open_impl(
            &registry,
            &handoff,
            &sessions,
            label,
            arg(&args, "tabId")?,
            arg(&args, "docPath")?,
        )?),
        "wasen_light_edit" => ok(crate::editor::wasen_light_edit_impl(
            &sessions,
            label,
            arg(&args, "cmd")?,
            arg(&args, "tabId")?,
        )?),
        "wasen_light_save" => ok(crate::editor::wasen_light_save_impl(
            &sessions,
            label,
            arg(&args, "tabId")?,
        )?),
        "wasen_light_close_tab" => ok(crate::editor::wasen_light_close_tab_impl(
            &sessions,
            label,
            arg(&args, "tabId")?,
        )?),
        // ── window-opening commands: stash, then let the tab open the surface
        "viewer_view" => {
            let path: String = arg(&args, "path")?;
            let l = crate::viewer::prepare_quick_view(&handoff, &recents, &path)?;
            web_open(format!("/viewer.html?win={l}"))
        }
        "wasen_light_view" => {
            let path: String = arg(&args, "path")?;
            let l = crate::editor::prepare_view(&handoff, &recents, &path)?;
            web_open(format!("/editor.html?win={l}"))
        }
        "wasen_light_reopen" => {
            let (l, _) = crate::editor::prepare_reopen(&handoff, &recents, &sessions, label)?;
            web_open(format!("/editor.html?win={l}"))
        }
        // ── start-page extras: a browser tab has no startup document; settings
        // is an in-page view since T-A46 (nothing to open server-side)
        "get_startup_document" => Ok(Value::Null),
        "plugin:app|version" => ok(app.package_info().version.to_string()),
        // ── ceiling
        "open_document" | "create_document" => Err(ENGINE_TAURI_ONLY.into()),
        other => Err(format!("web bridge: unknown command `{other}`")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_allowlist_is_exact() {
        assert!(origin_allowed(Some("http://127.0.0.1:1420")));
        assert!(origin_allowed(Some("http://localhost:1420")));
        assert!(!origin_allowed(None));
        assert!(!origin_allowed(Some("http://127.0.0.1:1421")));
        assert!(!origin_allowed(Some("http://evil.test")));
        assert!(!origin_allowed(Some("http://127.0.0.1:1420/")));
    }

    /// Absent keys read as null (Option → None, required → error) and a wrong
    /// type names the key — the Tauri IPC shape the frontend already speaks.
    #[test]
    fn arg_extraction_mirrors_tauri_ipc() {
        let args = json!({ "tabId": 3, "path": "/x.docx" });
        assert_eq!(arg::<Option<u32>>(&args, "tabId").unwrap(), Some(3));
        assert_eq!(arg::<Option<u32>>(&args, "missing").unwrap(), None);
        assert_eq!(arg::<String>(&args, "path").unwrap(), "/x.docx");
        let err = arg::<String>(&args, "missing").unwrap_err();
        assert!(err.contains("`missing`"), "{err}");
        let err = arg::<u32>(&args, "path").unwrap_err();
        assert!(err.contains("`path`"), "{err}");
    }
}
